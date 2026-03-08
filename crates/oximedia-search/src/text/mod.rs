//! Full-text search implementation.

pub mod fuzzy;
pub mod search;
pub mod stemmer;
pub mod tokenizer;

pub use fuzzy::FuzzyMatcher;
pub use search::TextSearchIndex;
pub use stemmer::Stemmer;
pub use tokenizer::Tokenizer;
