#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

// Include the generated bindings
// NOTE: Commented out because `neug/neug.h` relies on heavy C++20 templates (std::vector,
// google::protobuf, arrow) which bindgen struggles to parse natively. In a production scenario,
// a dedicated C wrapper API (extern "C") should be created for bindgen.
// include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
