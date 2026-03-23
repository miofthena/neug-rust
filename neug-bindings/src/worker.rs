use crate::error::{Error, Result};
use bincode::{deserialize_from, serialize_into};
use neug_protocol::{Request, RequestPayload, Response, ResponsePayload};
use std::collections::HashMap;
use std::io::{BufReader, BufWriter, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

type ResponseSender = std::sync::mpsc::Sender<ResponsePayload>;

struct SharedState {
    child: Option<Child>,
    stdin: Option<BufWriter<ChildStdin>>,
    pending_requests: HashMap<u64, ResponseSender>,
    error: Option<String>,
}

pub(crate) struct WorkerClient {
    state: Arc<Mutex<SharedState>>,
    next_req_id: AtomicU64,
}

impl WorkerClient {
    pub fn spawn() -> Result<Self> {
        let mut command = Command::new("neug-worker");

        if let Ok(mut exe_path) = std::env::current_exe() {
            exe_path.pop(); // remove current executable name

            let mut candidate = exe_path.join("neug-worker");
            if candidate.exists() {
                command = Command::new(candidate);
            } else {
                exe_path.pop();
                candidate = exe_path.join("neug-worker");
                if candidate.exists() {
                    command = Command::new(candidate);
                }
            }
        }

        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| {
                Error::InitializationFailed(format!(
                    "Failed to spawn neug-worker. Make sure it is compiled and in PATH: {}",
                    e
                ))
            })?;

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let state = Arc::new(Mutex::new(SharedState {
            child: Some(child),
            stdin: Some(BufWriter::new(stdin)),
            pending_requests: HashMap::new(),
            error: None,
        }));

        let state_clone = Arc::clone(&state);

        // Spawn a dedicated reader thread to handle out-of-order responses
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                match deserialize_from::<_, Response>(&mut reader) {
                    Ok(res) => {
                        let mut st = state_clone.lock().unwrap();
                        if let Some(sender) = st.pending_requests.remove(&res.req_id) {
                            let _ = sender.send(res.payload);
                        }
                    }
                    Err(e) => {
                        let mut st = state_clone.lock().unwrap();
                        // Connection lost or worker died
                        st.error = Some(format!("IPC reader thread failed: {}", e));
                        // Notify all pending requests
                        for (_, sender) in st.pending_requests.drain() {
                            let _ =
                                sender.send(ResponsePayload::Error("Worker disconnected".into()));
                        }
                        break;
                    }
                }
            }
        });

        Ok(Self {
            state,
            next_req_id: AtomicU64::new(1),
        })
    }

    pub fn send_request(&self, payload: RequestPayload) -> Result<ResponsePayload> {
        let req_id = self.next_req_id.fetch_add(1, Ordering::SeqCst);
        let req = Request { req_id, payload };

        let (tx, rx) = std::sync::mpsc::channel();

        {
            let mut st = self.state.lock().unwrap();

            if let Some(err) = &st.error {
                return Err(Error::ExecutionFailed(format!("Worker error: {}", err)));
            }

            st.pending_requests.insert(req_id, tx);

            if let Some(stdin) = st.stdin.as_mut() {
                if let Err(e) = serialize_into(&mut *stdin, &req) {
                    return Err(Error::ExecutionFailed(format!("IPC write error: {}", e)));
                }
                if let Err(e) = stdin.flush() {
                    return Err(Error::ExecutionFailed(format!("IPC flush error: {}", e)));
                }
            } else {
                return Err(Error::ExecutionFailed("Worker stdin is closed".into()));
            }
        }

        // Wait for the specific response to our request ID
        match rx.recv() {
            Ok(payload) => Ok(payload),
            Err(_) => {
                let st = self.state.lock().unwrap();
                let err_msg = st
                    .error
                    .clone()
                    .unwrap_or_else(|| "Unknown IPC error".into());
                Err(Error::ExecutionFailed(err_msg))
            }
        }
    }
}

impl Drop for WorkerClient {
    fn drop(&mut self) {
        let mut st = self.state.lock().unwrap();
        // Drop stdin to close the pipe, which will cause the worker to exit cleanly
        st.stdin.take();
        if let Some(mut child) = st.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
