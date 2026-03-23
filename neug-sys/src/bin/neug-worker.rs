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

fn main() {
    unsafe { neug_init() };

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());

    let mut dbs: HashMap<u64, neug_db_t> = HashMap::new();
    let mut conns: HashMap<u64, neug_conn_t> = HashMap::new();
    let mut next_db_id: u64 = 1;
    let mut next_conn_id: u64 = 1;

    // We keep a mapping from conn_id to its parent db_id to close it properly.
    let mut conn_to_db: HashMap<u64, u64> = HashMap::new();

    loop {
        // Read the next request
        let req: Request = match deserialize_from(&mut reader) {
            Ok(req) => req,
            Err(e) => {
                // If it's EOF, just exit cleanly
                if let bincode::ErrorKind::Io(io_err) = e.as_ref() {
                    if io_err.kind() == io::ErrorKind::UnexpectedEof {
                        break;
                    }
                }
                eprintln!("neug-worker: Failed to read request: {:?}", e);
                break;
            }
        };

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
                    let db_id = next_db_id;
                    next_db_id += 1;
                    dbs.insert(db_id, db_ptr);
                    ResponsePayload::OkDb { db_id }
                }
            }
            RequestPayload::Connect { db_id } => {
                if let Some(&db_ptr) = dbs.get(&db_id) {
                    let conn_ptr = unsafe { neug_db_connect(db_ptr) };
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
                        let conn_id = next_conn_id;
                        next_conn_id += 1;
                        conns.insert(conn_id, conn_ptr);
                        conn_to_db.insert(conn_id, db_id);
                        ResponsePayload::OkConn { conn_id }
                    }
                } else {
                    ResponsePayload::Error("Invalid db_id".to_string())
                }
            }
            RequestPayload::Execute { conn_id, query } => {
                if let Some(&conn_ptr) = conns.get(&conn_id) {
                    if let Ok(c_query) = CString::new(query) {
                        let res_ptr = unsafe {
                            neug_conn_execute(conn_ptr, c_query.as_ptr(), std::ptr::null())
                        };
                        if res_ptr.is_null() {
                            ResponsePayload::Error("Failed to invoke execute on engine".to_string())
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
                if let Some(conn_ptr) = conns.remove(&conn_id) {
                    if let Some(db_id) = conn_to_db.remove(&conn_id) {
                        if let Some(&db_ptr) = dbs.get(&db_id) {
                            unsafe { neug_conn_close(db_ptr, conn_ptr) };
                        }
                    }
                }
                // Return success even if it didn't exist to allow idempotent closes
                ResponsePayload::OkConn { conn_id }
            }
            RequestPayload::CloseDb { db_id } => {
                if let Some(db_ptr) = dbs.remove(&db_id) {
                    unsafe { neug_db_close(db_ptr) };
                }
                ResponsePayload::OkDb { db_id }
            }
        };

        let response = Response { req_id, payload };
        if serialize_into(&mut writer, &response).is_err() {
            break;
        }
        let _ = writer.flush();
    }
}