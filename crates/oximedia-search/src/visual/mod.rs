//! Visual similarity search.

pub mod features;
pub mod index;
pub mod search;

pub use features::FeatureExtractor;
pub use index::VisualIndex;
pub use search::VisualSearch;
