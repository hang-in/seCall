// secall-core library entrypoint
pub mod error;
pub mod hooks;
pub mod ingest;
pub mod mcp;
pub mod search;
pub mod store;
pub mod vault;

pub use error::{Result, SecallError};
