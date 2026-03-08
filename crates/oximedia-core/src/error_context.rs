//! Structured error context and error chaining utilities.
//!
//! Provides [`ErrorContext`], [`ErrorChain`], and [`ErrorContextBuilder`] for
//! attaching structured metadata (component, operation, and arbitrary key/value
//! pairs) to errors propagated through the media pipeline.
//!
//! # Examples
//!
//! ```
//! use oximedia_core::error_context::{ErrorContext, ErrorChain};
//!
//! let ctx = ErrorContext::new("demuxer", "read_packet", "unexpected EOF");
//! assert_eq!(ctx.component(), "demuxer");
//!
//! let chain = ErrorChain::root(ctx);
//! assert_eq!(chain.depth(), 1);
//! ```

#![allow(dead_code)]
#![allow(clippy::module_name_repetitions)]

use std::collections::HashMap;

/// Structured context attached to a single error occurrence.
///
/// Records where an error happened (`component`, `operation`) and a
/// human-readable `message`.  Optional key/value pairs may carry additional
/// diagnostic information.
///
/// # Examples
///
/// ```
/// use oximedia_core::error_context::ErrorContext;
///
/// let ctx = ErrorContext::new("muxer", "write_header", "disk full");
/// assert_eq!(ctx.component(), "muxer");
/// assert_eq!(ctx.operation(), "write_header");
/// assert_eq!(ctx.message(), "disk full");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorContext {
    component: String,
    operation: String,
    message: String,
    fields: HashMap<String, String>,
}

impl ErrorContext {
    /// Creates a new context with the given component, operation, and message.
    #[must_use]
    pub fn new(component: &str, operation: &str, message: &str) -> Self {
        Self {
            component: component.to_owned(),
            operation: operation.to_owned(),
            message: message.to_owned(),
            fields: HashMap::new(),
        }
    }

    /// Returns the name of the component that raised the error.
    #[inline]
    #[must_use]
    pub fn component(&self) -> &str {
        &self.component
    }

    /// Returns the name of the operation that was in progress.
    #[inline]
    #[must_use]
    pub fn operation(&self) -> &str {
        &self.operation
    }

    /// Returns the human-readable error message.
    #[inline]
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Attaches an additional key/value diagnostic field.
    ///
    /// Calling this multiple times with the same key overwrites the previous
    /// value.
    pub fn with_field(&mut self, key: &str, value: &str) -> &mut Self {
        self.fields.insert(key.to_owned(), value.to_owned());
        self
    }

    /// Returns the value of a diagnostic field, or `None` if absent.
    #[must_use]
    pub fn field(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }

    /// Returns an iterator over all attached diagnostic fields.
    pub fn fields(&self) -> impl Iterator<Item = (&str, &str)> {
        self.fields.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

impl std::fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}::{}] {}",
            self.component, self.operation, self.message
        )
    }
}

// ---------------------------------------------------------------------------

/// A chain of [`ErrorContext`] records representing an error's call stack.
///
/// Errors are pushed from innermost (root cause) to outermost (top-level
/// context).  [`depth`](Self::depth) returns the number of frames.
///
/// # Examples
///
/// ```
/// use oximedia_core::error_context::{ErrorContext, ErrorChain};
///
/// let root = ErrorContext::new("io", "read", "timeout");
/// let mut chain = ErrorChain::root(root);
/// chain.push(ErrorContext::new("demuxer", "read_packet", "I/O error"));
/// assert_eq!(chain.depth(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct ErrorChain {
    frames: Vec<ErrorContext>,
}

impl ErrorChain {
    /// Creates a chain containing a single root (innermost) context.
    #[must_use]
    pub fn root(ctx: ErrorContext) -> Self {
        Self { frames: vec![ctx] }
    }

    /// Creates an empty chain.
    #[must_use]
    pub fn empty() -> Self {
        Self { frames: Vec::new() }
    }

    /// Pushes an outer context onto the chain.
    pub fn push(&mut self, ctx: ErrorContext) {
        self.frames.push(ctx);
    }

    /// Returns the number of context frames in the chain.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    /// Returns `true` if the chain contains no frames.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Returns the root (innermost / first-cause) context, or `None` if empty.
    #[must_use]
    pub fn root_cause(&self) -> Option<&ErrorContext> {
        self.frames.first()
    }

    /// Returns the outermost context (most recently pushed), or `None` if empty.
    #[must_use]
    pub fn outermost(&self) -> Option<&ErrorContext> {
        self.frames.last()
    }

    /// Returns an iterator over all frames from root to outermost.
    pub fn iter(&self) -> impl Iterator<Item = &ErrorContext> {
        self.frames.iter()
    }

    /// Returns `true` if any frame's component matches `component`.
    #[must_use]
    pub fn involves(&self, component: &str) -> bool {
        self.frames.iter().any(|f| f.component() == component)
    }
}

impl std::fmt::Display for ErrorChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, frame) in self.frames.iter().enumerate() {
            if i > 0 {
                write!(f, " -> ")?;
            }
            write!(f, "{frame}")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------

/// A builder that constructs an [`ErrorContext`] with a fluent API.
///
/// # Examples
///
/// ```
/// use oximedia_core::error_context::ErrorContextBuilder;
///
/// let ctx = ErrorContextBuilder::new("codec", "decode_frame")
///     .message("bitstream error")
///     .field("pts", "12345")
///     .build();
///
/// assert_eq!(ctx.component(), "codec");
/// assert_eq!(ctx.field("pts"), Some("12345"));
/// ```
#[derive(Debug, Default)]
pub struct ErrorContextBuilder {
    component: String,
    operation: String,
    message: String,
    fields: HashMap<String, String>,
}

impl ErrorContextBuilder {
    /// Starts a new builder with the given `component` and `operation`.
    #[must_use]
    pub fn new(component: &str, operation: &str) -> Self {
        Self {
            component: component.to_owned(),
            operation: operation.to_owned(),
            message: String::new(),
            fields: HashMap::new(),
        }
    }

    /// Sets the error message.
    #[must_use]
    pub fn message(mut self, msg: &str) -> Self {
        msg.clone_into(&mut self.message);
        self
    }

    /// Attaches a key/value diagnostic field.
    #[must_use]
    pub fn field(mut self, key: &str, value: &str) -> Self {
        self.fields.insert(key.to_owned(), value.to_owned());
        self
    }

    /// Consumes the builder and returns the constructed [`ErrorContext`].
    #[must_use]
    pub fn build(self) -> ErrorContext {
        ErrorContext {
            component: self.component,
            operation: self.operation,
            message: self.message,
            fields: self.fields,
        }
    }
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_context_accessors() {
        let ctx = ErrorContext::new("demuxer", "read_packet", "EOF");
        assert_eq!(ctx.component(), "demuxer");
        assert_eq!(ctx.operation(), "read_packet");
        assert_eq!(ctx.message(), "EOF");
    }

    #[test]
    fn error_context_with_field() {
        let mut ctx = ErrorContext::new("codec", "decode", "error");
        ctx.with_field("pts", "1000");
        assert_eq!(ctx.field("pts"), Some("1000"));
    }

    #[test]
    fn error_context_missing_field_is_none() {
        let ctx = ErrorContext::new("x", "y", "z");
        assert!(ctx.field("nonexistent").is_none());
    }

    #[test]
    fn error_context_field_overwrite() {
        let mut ctx = ErrorContext::new("a", "b", "c");
        ctx.with_field("k", "v1");
        ctx.with_field("k", "v2");
        assert_eq!(ctx.field("k"), Some("v2"));
    }

    #[test]
    fn error_context_display() {
        let ctx = ErrorContext::new("muxer", "write", "disk full");
        let s = ctx.to_string();
        assert!(s.contains("muxer"));
        assert!(s.contains("write"));
        assert!(s.contains("disk full"));
    }

    #[test]
    fn error_chain_root_depth_one() {
        let ctx = ErrorContext::new("io", "read", "timeout");
        let chain = ErrorChain::root(ctx);
        assert_eq!(chain.depth(), 1);
    }

    #[test]
    fn error_chain_push_increases_depth() {
        let mut chain = ErrorChain::root(ErrorContext::new("a", "op", "msg"));
        chain.push(ErrorContext::new("b", "op2", "msg2"));
        assert_eq!(chain.depth(), 2);
    }

    #[test]
    fn error_chain_root_cause() {
        let ctx = ErrorContext::new("inner", "op", "root cause");
        let chain = ErrorChain::root(ctx.clone());
        assert_eq!(chain.root_cause(), Some(&ctx));
    }

    #[test]
    fn error_chain_outermost() {
        let mut chain = ErrorChain::root(ErrorContext::new("inner", "op", "cause"));
        let outer = ErrorContext::new("outer", "handle", "context");
        chain.push(outer.clone());
        assert_eq!(chain.outermost(), Some(&outer));
    }

    #[test]
    fn error_chain_involves() {
        let mut chain = ErrorChain::root(ErrorContext::new("io", "read", "err"));
        chain.push(ErrorContext::new("demuxer", "parse", "err2"));
        assert!(chain.involves("io"));
        assert!(chain.involves("demuxer"));
        assert!(!chain.involves("encoder"));
    }

    #[test]
    fn error_chain_empty() {
        let chain = ErrorChain::empty();
        assert!(chain.is_empty());
        assert_eq!(chain.depth(), 0);
        assert!(chain.root_cause().is_none());
        assert!(chain.outermost().is_none());
    }

    #[test]
    fn error_chain_display_multi_frame() {
        let mut chain = ErrorChain::root(ErrorContext::new("a", "op", "first"));
        chain.push(ErrorContext::new("b", "op2", "second"));
        let s = chain.to_string();
        assert!(s.contains("first"));
        assert!(s.contains("second"));
        assert!(s.contains("->"));
    }

    #[test]
    fn builder_creates_correct_context() {
        let ctx = ErrorContextBuilder::new("codec", "decode_frame")
            .message("bitstream error")
            .field("pts", "12345")
            .build();
        assert_eq!(ctx.component(), "codec");
        assert_eq!(ctx.operation(), "decode_frame");
        assert_eq!(ctx.message(), "bitstream error");
        assert_eq!(ctx.field("pts"), Some("12345"));
    }

    #[test]
    fn builder_default_message_is_empty() {
        let ctx = ErrorContextBuilder::new("c", "op").build();
        assert_eq!(ctx.message(), "");
    }

    #[test]
    fn error_chain_iter_count_matches_depth() {
        let mut chain = ErrorChain::root(ErrorContext::new("a", "op", "e1"));
        chain.push(ErrorContext::new("b", "op2", "e2"));
        chain.push(ErrorContext::new("c", "op3", "e3"));
        assert_eq!(chain.iter().count(), chain.depth());
    }
}
