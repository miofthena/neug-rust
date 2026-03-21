use crate::connection::Connection;
use crate::error::{Error, Result};
use neug_sys::{neug_db_close, neug_db_connect, neug_db_open, neug_db_options_t, neug_get_last_error, neug_init};
use std::ffi::{CStr, CString};
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
    // Pointer to the C++ NeugDB object via our C wrapper
    inner: neug_sys::neug_db_t,
    options: DatabaseOptions,
}

// Ensure Database can be sent across threads as per NeugDB C++ thread-safety semantics
unsafe impl Send for Database {}
unsafe impl Sync for Database {}

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
        // Set environment variables for the underlying C++ glog library
        // to prevent it from spamming stdout/stderr with INFO messages
        // (like "Closing connection" on every benchmark iteration).
        unsafe { std::env::set_var("GLOG_minloglevel", "2") }; // 0=INFO, 1=WARNING, 2=ERROR
        
        unsafe { neug_init() };

        let c_path = CString::new(options.db_path.clone()).unwrap();
        let c_mode = CString::new(options.mode.as_str()).unwrap();

        let c_options = neug_db_options_t {
            db_path: c_path.as_ptr(),
            mode: c_mode.as_ptr(),
            max_thread_num: options.max_thread_num,
            checkpoint_on_close: options.checkpoint_on_close,
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
            return Err(Error::InitializationFailed(error_msg));
        }

        Ok(Self {
            inner: db_ptr,
            options,
        })
    }

    /// Get the mode of the database.
    pub fn mode(&self) -> Mode {
        self.options.mode
    }

    /// Creates a new connection to the database.
    pub fn connect(&self) -> Result<Connection> {
        if self.inner.is_null() {
            return Err(Error::DatabaseClosed);
        }

        let conn_ptr = unsafe { neug_db_connect(self.inner) };
        
        if conn_ptr.is_null() {
            let error_msg = unsafe {
                let err_ptr = neug_get_last_error();
                if err_ptr.is_null() {
                    "Failed to create connection".to_string()
                } else {
                    CStr::from_ptr(err_ptr).to_string_lossy().into_owned()
                }
            };
            return Err(Error::InitializationFailed(error_msg));
        }

        Ok(Connection::new(conn_ptr))
    }

    /// Close the database.
    pub fn close(&mut self) {
        if !self.inner.is_null() {
            unsafe { neug_db_close(self.inner) };
            self.inner = std::ptr::null_mut();
        }
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        self.close();
    }
}
