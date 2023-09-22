use std::sync::OnceLock;

pub mod error;
pub mod module;
pub mod storage;

pub static DOMAIN: OnceLock<String> = OnceLock::new();
