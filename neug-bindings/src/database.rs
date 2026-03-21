use crate::connection::Connection;
use crate::error::{Error, Result};
use std::path::Path;

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
    // In a real implementation, this would hold the FFI pointer/handle to the C++ NeugDB object.
    // e.g., inner: cxx::UniquePtr<ffi::NeugDB>
    options: DatabaseOptions,
    is_closed: bool,
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
        // Here you would initialize the C++ NeugDB object.
        // let inner = ffi::new_neug_db(options.db_path.clone(), options.mode.as_str(), options.max_thread_num, options.checkpoint_on_close)?;

        Ok(Self {
            options,
            is_closed: false,
        })
    }

    /// Get the mode of the database.
    pub fn mode(&self) -> Mode {
        self.options.mode
    }

    /// Creates a new connection to the database.
    pub fn connect(&self) -> Result<Connection> {
        if self.is_closed {
            return Err(Error::DatabaseClosed);
        }

        // let conn_inner = ffi::db_connect(self.inner.as_ref())?;

        Ok(Connection::new())
    }

    /// Close the database.
    pub fn close(&mut self) {
        if !self.is_closed {
            // ffi::db_close(self.inner.pin_mut());
            self.is_closed = true;
        }
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        self.close();
    }
}
