use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod types;
pub mod error;
pub mod deterministic;
pub mod canonical;

pub use types::*;
pub use error::*;
pub use deterministic::*;
pub use canonical::*;

pub const DOMAIN_SEPARATOR: &[u8] = b"LITE-VISION";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");