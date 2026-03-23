use bincode::{deserialize_from, serialize_into};
use neug_protocol::{Request, RequestPayload, Response, ResponsePayload};
use neug_sys::{
    neug_conn_close, neug_conn_execute, neug_conn_t, neug_db_close, neug_db_connect, neug_db_open,
    neug_db_options_t, neug_db_t, neug_get_last_error, neug_init, neug_result_free,
    neug_result_get_error, neug_result_is_ok, neug_result_to_string,
};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::io::{self, BufReader, BufWriter, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

#[derive(Clone, Copy)]
struct SyncDb(neug_db_t);
unsafe impl Send for SyncDb {}
unsafe impl Sync for SyncDb {}

#[derive(Clone, Copy)]
struct SyncConn(neug_conn_t);
unsafe impl Send for SyncConn {}
unsafe impl Sync for SyncConn {}

fn main() {
    unsafe { neug_init() };

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());

    let dbs: Arc<RwLock<HashMap<u64, SyncDb>>> = Arc::new(RwLock::new(HashMap::new()));
    let conns: Arc<RwLock<HashMap<u64, SyncConn>>> = Arc::new(RwLock::new(HashMap::new()));
    let conn_to_db: Arc<RwLock<HashMap<u64, u64>>> = Arc::new(RwLock::new(HashMap::new()));

    let next_db_id = Arc::new(AtomicU64::new(1));
    let next_conn_id = Arc::new(AtomicU64::new(1));

    let (tx, rx) = std::sync::mpsc::channel::<Response>();

    // Writer thread
    std::thread::spawn(move || {
        let mut writer = BufWriter::new(stdout.lock());
        for res in rx {
            if serialize_into(&mut writer, &res).is_err() {
                break;
            }
            let _ = writer.flush();
        }
    });

    loop {
        let req: Request = match deserialize_from(&mut reader) {
            Ok(req) => req,
            Err(e) => {
                if let bincode::ErrorKind::Io(io_err) = e.as_ref() {
                    if io_err.kind() == io::ErrorKind::UnexpectedEof {
                        break;
                    }
                }
                eprintln!("neug-worker: Failed to read request: {:?}", e);
                break;
            }
        };

        let tx_clone = tx.clone();
        let dbs_clone = dbs.clone();
        let conns_clone = conns.clone();
        let conn_to_db_clone = conn_to_db.clone();
        let next_db_id_clone = next_db_id.clone();
        let next_conn_id_clone = next_conn_id.clone();

        rayon::spawn(move || {
            let req_id = req.req_id;
            let payload = match req.payload {
                RequestPayload::OpenDb {
                    path,
                    mode,
                    max_thread_num,
                    checkpoint_on_close,
                } => {
                    let c_path = CString::new(path).unwrap();
                    let c_mode = CString::new(mode).unwrap();
                    let c_options = neug_db_options_t {
                        db_path: c_path.as_ptr(),
                        mode: c_mode.as_ptr(),
                        max_thread_num,
                        checkpoint_on_close,
                    };
                    let db_ptr = unsafe { neug_db_open(&c_options) };
                    if db_ptr.is_null() {
                        let error_msg = unsafe {
                            let err_ptr = neug_get_last_error();
                            if err_ptr.is_null() {
                                "Unknown error".to_string()
                            } else {
                                CStr::from_ptr(err_ptr).to_string_lossy().into_owned()
                            }
                        };
                        ResponsePayload::Error(error_msg)
                    } else {
                        let db_id = next_db_id_clone.fetch_add(1, Ordering::SeqCst);
                        dbs_clone.write().unwrap().insert(db_id, SyncDb(db_ptr));
                        ResponsePayload::OkDb { db_id }
                    }
                }
                RequestPayload::Connect { db_id } => {
                    let db_ptr = dbs_clone.read().unwrap().get(&db_id).copied();
                    if let Some(SyncDb(ptr)) = db_ptr {
                        let conn_ptr = unsafe { neug_db_connect(ptr) };
                        if conn_ptr.is_null() {
                            let error_msg = unsafe {
                                let err_ptr = neug_get_last_error();
                                if err_ptr.is_null() {
                                    "Failed to create connection".to_string()
                                } else {
                                    CStr::from_ptr(err_ptr).to_string_lossy().into_owned()
                                }
                            };
                            ResponsePayload::Error(error_msg)
                        } else {
                            let conn_id = next_conn_id_clone.fetch_add(1, Ordering::SeqCst);
                            conns_clone
                                .write()
                                .unwrap()
                                .insert(conn_id, SyncConn(conn_ptr));
                            conn_to_db_clone.write().unwrap().insert(conn_id, db_id);
                            ResponsePayload::OkConn { conn_id }
                        }
                    } else {
                        ResponsePayload::Error("Invalid db_id".to_string())
                    }
                }
                RequestPayload::Execute { conn_id, query } => {
                    let conn_ptr = conns_clone.read().unwrap().get(&conn_id).copied();
                    if let Some(SyncConn(ptr)) = conn_ptr {
                        if let Ok(c_query) = CString::new(query) {
                            let res_ptr = unsafe {
                                neug_conn_execute(ptr, c_query.as_ptr(), std::ptr::null())
                            };
                            if res_ptr.is_null() {
                                ResponsePayload::Error(
                                    "Failed to invoke execute on engine".to_string(),
                                )
                            } else {
                                let is_ok = unsafe { neug_result_is_ok(res_ptr) };
                                if !is_ok {
                                    let error_msg = unsafe {
                                        let err_ptr = neug_result_get_error(res_ptr);
                                        if err_ptr.is_null() {
                                            "Unknown execution error".to_string()
                                        } else {
                                            CStr::from_ptr(err_ptr).to_string_lossy().into_owned()
                                        }
                                    };
                                    unsafe { neug_result_free(res_ptr) };
                                    ResponsePayload::Error(error_msg)
                                } else {
                                    let result_string = unsafe {
                                        let ptr = neug_result_to_string(res_ptr);
                                        if ptr.is_null() {
                                            String::new()
                                        } else {
                                            CStr::from_ptr(ptr).to_string_lossy().into_owned()
                                        }
                                    };
                                    unsafe { neug_result_free(res_ptr) };
                                    ResponsePayload::OkResult { result_string }
                                }
                            }
                        } else {
                            ResponsePayload::Error("Query contains null byte".to_string())
                        }
                    } else {
                        ResponsePayload::Error("Invalid conn_id".to_string())
                    }
                }
                RequestPayload::CloseConn { conn_id } => {
                    let conn_ptr = conns_clone.write().unwrap().remove(&conn_id);
                    if let Some(SyncConn(c_ptr)) = conn_ptr {
                        let db_id = conn_to_db_clone.write().unwrap().remove(&conn_id);
                        if let Some(d_id) = db_id {
                            let db_ptr = dbs_clone.read().unwrap().get(&d_id).copied();
                            if let Some(SyncDb(d_ptr)) = db_ptr {
                                unsafe { neug_conn_close(d_ptr, c_ptr) };
                            }
                        }
                    }
                    ResponsePayload::OkConn { conn_id }
                }
                RequestPayload::CloseDb { db_id } => {
                    let db_ptr = dbs_clone.write().unwrap().remove(&db_id);
                    if let Some(SyncDb(ptr)) = db_ptr {
                        unsafe { neug_db_close(ptr) };
                    }
                    ResponsePayload::OkDb { db_id }
                }
            };

            let response = Response { req_id, payload };
            let _ = tx_clone.send(response);
        });
    }
}
