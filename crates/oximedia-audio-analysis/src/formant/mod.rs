//! Formant analysis module using Linear Predictive Coding (LPC).

pub mod analyze;
pub mod track;
pub mod vowel;

pub use analyze::{FormantAnalyzer, FormantResult};
pub use track::FormantTracker;
pub use vowel::{detect_vowel, Vowel};
