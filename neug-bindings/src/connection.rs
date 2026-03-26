use crate::error::{Error, Result};
use crate::worker::WorkerClient;
use neug_protocol::{RequestPayload, ResponsePayload};
use std::collections::HashMap;
use std::sync::Arc;

/// Represents the access mode for a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    Read,
    Insert,
    Update,
    Schema,
}

impl AccessMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            AccessMode::Read => "r",
            AccessMode::Insert => "i",
            AccessMode::Update => "u",
            AccessMode::Schema => "s",
        }
    }
}

use std::fmt;

/// Represents the result of a query.
#[derive(Debug)]
pub struct QueryResult {
    result_string: String,
}

impl fmt::Display for QueryResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.result_string)
    }
}

/// Represents a connection to the NeuG database.
pub struct Connection {
    conn_id: u64,
    worker: Arc<WorkerClient>,
}

impl Connection {
    pub(crate) fn new(conn_id: u64, worker: Arc<WorkerClient>) -> Self {
        Self { conn_id, worker }
    }

    /// Checks if the connection is currently open.
    pub fn is_open(&self) -> bool {
        self.conn_id != 0
    }

    /// Executes a Cypher query on the database.
    pub fn execute(&self, query: &str) -> Result<QueryResult> {
        self.execute_with_options(query, None, None)
    }

    /// Executes a Cypher query with a specific access mode and parameters.
    pub fn execute_with_options(
        &self,
        query: &str,
        _access_mode: Option<AccessMode>,
        _parameters: Option<&HashMap<String, String>>, // Future: implement parameter mapping
    ) -> Result<QueryResult> {
        if !self.is_open() {
            return Err(Error::ConnectionClosed);
        }

        let res = self.worker.send_request(RequestPayload::Execute {
            conn_id: self.conn_id,
            query: query.to_string(),
        })?;

        match res {
            ResponsePayload::OkResult { result_string } => Ok(QueryResult { result_string }),
            ResponsePayload::Error(msg) => Err(Error::ExecutionFailed(msg)),
            _ => Err(Error::ExecutionFailed("Unexpected response".into())),
        }
    }

    /// Closes the connection.
    pub fn close(&mut self) {
        if self.conn_id != 0 {
            let _ = self.worker.send_request(RequestPayload::CloseConn {
                conn_id: self.conn_id,
            });
            self.conn_id = 0;
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.close();
    }
}
