use crate::error::{Error, Result};
use bincode::{deserialize_from, serialize_into};
use neug_protocol::{Request, RequestPayload, Response, ResponsePayload};
use std::io::{BufReader, BufWriter, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

struct WorkerState {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

pub(crate) struct WorkerClient {
    state: Mutex<WorkerState>,
    next_req_id: AtomicU64,
}

impl WorkerClient {
    pub fn spawn() -> Result<Self> {
        let mut child = Command::new("neug-worker")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| {
                Error::InitializationFailed(format!(
                    "Failed to spawn neug-worker. Make sure it is installed and in PATH: {}",
                    e
                ))
            })?;

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        Ok(Self {
            state: Mutex::new(WorkerState {
                child,
                stdin: BufWriter::new(stdin),
                stdout: BufReader::new(stdout),
            }),
            next_req_id: AtomicU64::new(1),
        })
    }

    pub fn send_request(&self, payload: RequestPayload) -> Result<ResponsePayload> {
        let req_id = self.next_req_id.fetch_add(1, Ordering::SeqCst);
        let req = Request { req_id, payload };

        // Lock the entire worker state to ensure request/response cycle is atomic
        let mut state = self.state.lock().unwrap();

        serialize_into(&mut state.stdin, &req)
            .map_err(|e| Error::ExecutionFailed(format!("IPC write error: {}", e)))?;
        state
            .stdin
            .flush()
            .map_err(|e| Error::ExecutionFailed(format!("IPC flush error: {}", e)))?;

        let res: Response = deserialize_from(&mut state.stdout)
            .map_err(|e| Error::ExecutionFailed(format!("IPC read error: {}", e)))?;

        if res.req_id != req_id {
            return Err(Error::ExecutionFailed(format!(
                "IPC protocol error: expected response for {}, got {}",
                req_id, res.req_id
            )));
        }

        Ok(res.payload)
    }
}

impl Drop for WorkerClient {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            let _ = state.child.kill();
            let _ = state.child.wait();
        }
    }
}
