use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum RequestPayload {
    OpenDb {
        path: String,
        mode: String,
        max_thread_num: usize,
        checkpoint_on_close: bool,
    },
    Connect {
        db_id: u64,
    },
    Execute {
        conn_id: u64,
        query: String,
        access_mode: Option<String>,
    },
    CloseConn {
        conn_id: u64,
    },
    CloseDb {
        db_id: u64,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Request {
    pub req_id: u64,
    pub payload: RequestPayload,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ResponsePayload {
    OkDb { db_id: u64 },
    OkConn { conn_id: u64 },
    OkResult { result_string: String },
    Error(String),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Response {
    pub req_id: u64,
    pub payload: ResponsePayload,
}
