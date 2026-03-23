use crate::connection::Connection;
use crate::error::{Error, Result};
use crate::worker::WorkerClient;
use neug_protocol::{RequestPayload, ResponsePayload};
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    ReadOnly,
    ReadWrite,
}

impl Mode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Mode::ReadOnly => "read-only",
            Mode::ReadWrite => "read-write",
        }
    }
}

pub struct DatabaseOptions {
    pub db_path: String,
    pub mode: Mode,
    pub max_thread_num: usize,
    pub checkpoint_on_close: bool,
}

impl Default for DatabaseOptions {
    fn default() -> Self {
        Self {
            db_path: String::new(),
            mode: Mode::ReadWrite,
            max_thread_num: 0,
            checkpoint_on_close: true,
        }
    }
}

pub struct Database {
    db_id: u64,
    worker: Arc<WorkerClient>,
    options: DatabaseOptions,
}

impl Database {
    /// Opens a database at the specified path.
    /// If `db_path` is empty or ":memory:", the database is opened in memory.
    pub fn open<P: AsRef<Path>>(db_path: P, mode: Mode) -> Result<Self> {
        let options = DatabaseOptions {
            db_path: db_path.as_ref().to_string_lossy().into_owned(),
            mode,
            ..Default::default()
        };
        Self::with_options(options)
    }

    /// Opens a database with full options.
    pub fn with_options(options: DatabaseOptions) -> Result<Self> {
        let worker = Arc::new(WorkerClient::spawn()?);

        let res = worker.send_request(RequestPayload::OpenDb {
            path: options.db_path.clone(),
            mode: options.mode.as_str().to_string(),
            max_thread_num: options.max_thread_num,
            checkpoint_on_close: options.checkpoint_on_close,
        })?;

        match res {
            ResponsePayload::OkDb { db_id } => Ok(Self {
                db_id,
                worker,
                options,
            }),
            ResponsePayload::Error(msg) => Err(Error::InitializationFailed(msg)),
            _ => Err(Error::InitializationFailed("Unexpected response".into())),
        }
    }

    /// Get the mode of the database.
    pub fn mode(&self) -> Mode {
        self.options.mode
    }

    /// Creates a new connection to the database.
    pub fn connect(&self) -> Result<Connection> {
        let res = self
            .worker
            .send_request(RequestPayload::Connect { db_id: self.db_id })?;

        match res {
            ResponsePayload::OkConn { conn_id } => {
                Ok(Connection::new(conn_id, self.worker.clone()))
            }
            ResponsePayload::Error(msg) => Err(Error::InitializationFailed(msg)),
            _ => Err(Error::InitializationFailed("Unexpected response".into())),
        }
    }

    /// Close the database.
    pub fn close(&mut self) {
        if self.db_id != 0 {
            let _ = self
                .worker
                .send_request(RequestPayload::CloseDb { db_id: self.db_id });
            self.db_id = 0;
        }
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        self.close();
    }
}
