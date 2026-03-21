pub mod connection;
pub mod database;

pub use connection::{AccessMode, Connection, QueryResult};
pub use database::{Database, DatabaseOptions, Mode};

// Include the generated bindings if building the FFI components
// #![allow(non_upper_case_globals)]
// #![allow(non_camel_case_types)]
// #![allow(non_snake_case)]
// include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
