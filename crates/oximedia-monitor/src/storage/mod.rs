//! Time series storage for metrics.

pub mod aggregation;
#[cfg(not(target_arch = "wasm32"))]
pub mod query;
#[cfg(not(target_arch = "wasm32"))]
pub mod retention;
pub mod ringbuffer;
#[cfg(not(target_arch = "wasm32"))]
pub mod sqlite;

pub use aggregation::{AggregateFunction, AggregatedMetric, Aggregator};
#[cfg(not(target_arch = "wasm32"))]
pub use query::{QueryEngine, TimeRange, TimeSeriesQuery, TimeSeriesResult};
#[cfg(not(target_arch = "wasm32"))]
pub use retention::RetentionManager;
pub use ringbuffer::RingBuffer;
#[cfg(not(target_arch = "wasm32"))]
pub use sqlite::{SqliteStorage, TimeSeriesPoint};
