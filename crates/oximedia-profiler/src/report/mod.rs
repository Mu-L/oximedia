//! Reporting modules.

pub mod generate;
pub mod html;
pub mod json;

pub use generate::{Report, ReportGenerator};
pub use html::HtmlReporter;
pub use json::JsonReporter;
