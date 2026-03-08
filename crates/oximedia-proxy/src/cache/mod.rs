//! Cache management module.

pub mod cleanup;
pub mod manager;
pub mod strategy;

pub use cleanup::{CacheCleanup, CacheStats, CleanupPolicy, CleanupResult};
pub use manager::CacheManager;
pub use strategy::CacheStrategy;
