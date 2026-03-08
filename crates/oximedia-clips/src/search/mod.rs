//! Search and filtering system.

pub mod engine;
pub mod filter;

pub use engine::SearchEngine;
pub use filter::{ClipFilter, FilterCriteria};
