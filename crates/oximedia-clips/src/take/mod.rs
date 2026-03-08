//! Take management for multi-take shots.

pub mod manager;
pub mod selector;

pub use manager::TakeManager;
pub use selector::{Take, TakeId, TakeSelector};
