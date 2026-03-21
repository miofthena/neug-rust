use crate::error::{Error, Result};
use neug_sys::{
    neug_conn_close, neug_conn_execute, neug_result_free, neug_result_get_error, neug_result_is_ok,
};
use std::collections::HashMap;
use std::ffi::{CStr, CString};

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

/// Represents the result of a query.
#[derive(Debug)]
pub struct QueryResult {
    // Pointer to the C++ QueryResult wrapper
    inner: neug_sys::neug_result_t,
}

impl QueryResult {
    // Methods to iterate over the result records would go here.
}

impl Drop for QueryResult {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            unsafe { neug_result_free(self.inner) };
            self.inner = std::ptr::null_mut();
        }
    }
}

/// Represents a connection to the NeuG database.
pub struct Connection {
    // Pointer to the C++ Connection object via our C wrapper
    inner: neug_sys::neug_conn_t,
}

// Connections in neug can be sent across threads but are not sync
unsafe impl Send for Connection {}

impl Connection {
    pub(crate) fn new(ptr: neug_sys::neug_conn_t) -> Self {
        Self { inner: ptr }
    }

    /// Checks if the connection is currently open.
    pub fn is_open(&self) -> bool {
        !self.inner.is_null()
    }

    /// Executes a Cypher query on the database.
    pub fn execute(&self, query: &str) -> Result<QueryResult> {
        self.execute_with_options(query, None, None)
    }

    /// Executes a Cypher query with a specific access mode and parameters.
    pub fn execute_with_options(
        &self,
        query: &str,
        access_mode: Option<AccessMode>,
        _parameters: Option<&HashMap<String, String>>, // Future: implement parameter mapping
    ) -> Result<QueryResult> {
        if !self.is_open() {
            return Err(Error::ConnectionClosed);
        }

        let c_query = CString::new(query)
            .map_err(|_| Error::InvalidArgument("Query contains null byte".into()))?;

        let c_mode = access_mode.map(|m| CString::new(m.as_str()).unwrap());
        let c_mode_ptr = c_mode.as_ref().map_or(std::ptr::null(), |m| m.as_ptr());

        let res_ptr = unsafe { neug_conn_execute(self.inner, c_query.as_ptr(), c_mode_ptr) };

        if res_ptr.is_null() {
            return Err(Error::ExecutionFailed(
                "Failed to invoke execute on engine".to_string(),
            ));
        }

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
            return Err(Error::ExecutionFailed(error_msg));
        }

        Ok(QueryResult { inner: res_ptr })
    }

    /// Closes the connection.
    pub fn close(&mut self) {
        if !self.inner.is_null() {
            unsafe { neug_conn_close(self.inner) };
            self.inner = std::ptr::null_mut();
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.close();
    }
}
