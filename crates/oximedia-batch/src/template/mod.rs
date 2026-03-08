//! Template system for dynamic file naming and configuration

pub mod engine;
pub mod functions;
pub mod variables;

use crate::error::Result;
use engine::TemplateEngine;
use std::collections::HashMap;
use std::path::Path;

/// Template context with variables
#[derive(Debug, Clone)]
pub struct TemplateContext {
    variables: HashMap<String, String>,
}

impl TemplateContext {
    /// Create a new template context
    #[must_use]
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    /// Set a variable
    pub fn set(&mut self, key: String, value: String) {
        self.variables.insert(key, value);
    }

    /// Get a variable
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&String> {
        self.variables.get(key)
    }

    /// Load variables from a file
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file
    ///
    /// # Errors
    ///
    /// Returns an error if loading fails
    pub fn from_file(&mut self, path: &Path) -> Result<()> {
        // Extract file properties
        if let Some(filename) = path.file_name() {
            self.set(
                "filename".to_string(),
                filename.to_string_lossy().to_string(),
            );
        }

        if let Some(stem) = path.file_stem() {
            self.set("stem".to_string(), stem.to_string_lossy().to_string());
        }

        if let Some(extension) = path.extension() {
            self.set(
                "extension".to_string(),
                extension.to_string_lossy().to_string(),
            );
        }

        if let Some(parent) = path.parent() {
            self.set(
                "directory".to_string(),
                parent.to_string_lossy().to_string(),
            );
        }

        // File metadata
        if let Ok(metadata) = std::fs::metadata(path) {
            self.set("size".to_string(), metadata.len().to_string());

            if let Ok(modified) = metadata.modified() {
                if let Ok(datetime) = modified.duration_since(std::time::UNIX_EPOCH) {
                    self.set("modified".to_string(), datetime.as_secs().to_string());
                }
            }
        }

        Ok(())
    }

    /// Load media properties
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the media file
    ///
    /// # Errors
    ///
    /// Returns an error if loading fails
    pub fn from_media(&mut self, _path: &Path) -> Result<()> {
        // TODO: Integration with oximedia-metadata
        // For now, set dummy values
        self.set("width".to_string(), "1920".to_string());
        self.set("height".to_string(), "1080".to_string());
        self.set("duration".to_string(), "120".to_string());
        self.set("codec".to_string(), "h264".to_string());
        self.set("bitrate".to_string(), "5000000".to_string());
        self.set("framerate".to_string(), "30".to_string());

        Ok(())
    }
}

impl Default for TemplateContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Template processor
pub struct TemplateProcessor {
    engine: TemplateEngine,
}

impl TemplateProcessor {
    /// Create a new template processor
    #[must_use]
    pub fn new() -> Self {
        Self {
            engine: TemplateEngine::new(),
        }
    }

    /// Process a template with context
    ///
    /// # Arguments
    ///
    /// * `template` - Template string
    /// * `context` - Template context
    ///
    /// # Errors
    ///
    /// Returns an error if processing fails
    pub fn process(&self, template: &str, context: &TemplateContext) -> Result<String> {
        self.engine.render(template, context)
    }

    /// Process a file path template
    ///
    /// # Arguments
    ///
    /// * `template` - Template string
    /// * `input_path` - Input file path
    ///
    /// # Errors
    ///
    /// Returns an error if processing fails
    pub fn process_file_path(&self, template: &str, input_path: &Path) -> Result<String> {
        let mut context = TemplateContext::new();
        context.from_file(input_path)?;
        self.process(template, &context)
    }
}

impl Default for TemplateProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_template_context_creation() {
        let context = TemplateContext::new();
        assert!(context.variables.is_empty());
    }

    #[test]
    fn test_set_and_get_variable() {
        let mut context = TemplateContext::new();
        context.set("key".to_string(), "value".to_string());

        assert_eq!(context.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_context_from_file() {
        let mut context = TemplateContext::new();
        let path = PathBuf::from("/tmp/test.mp4");

        context.from_file(&path).ok();

        assert_eq!(context.get("filename"), Some(&"test.mp4".to_string()));
        assert_eq!(context.get("stem"), Some(&"test".to_string()));
        assert_eq!(context.get("extension"), Some(&"mp4".to_string()));
    }

    #[test]
    fn test_context_from_media() {
        let mut context = TemplateContext::new();
        let path = PathBuf::from("/tmp/test.mp4");

        context.from_media(&path).ok();

        assert!(context.get("width").is_some());
        assert!(context.get("height").is_some());
        assert!(context.get("codec").is_some());
    }

    #[test]
    fn test_template_processor_creation() {
        let processor = TemplateProcessor::new();
        let _ = processor; // processor created successfully
    }
}
