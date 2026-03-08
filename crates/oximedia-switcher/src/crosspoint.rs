#![allow(dead_code)]
//! Crosspoint matrix routing for video switcher signal paths.
//!
//! This module implements a crosspoint matrix that maps input signals to
//! output destinations. It supports multiple signal layers (video, key,
//! audio), locking individual crosspoints, and salvos (preset routing states).

use std::collections::HashMap;
use std::fmt;

/// Signal layer within the crosspoint matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignalLayer {
    /// Video signal.
    Video,
    /// Key / alpha signal.
    Key,
    /// Audio signal.
    Audio,
    /// Metadata / ancillary data.
    Metadata,
}

impl fmt::Display for SignalLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Video => write!(f, "Video"),
            Self::Key => write!(f, "Key"),
            Self::Audio => write!(f, "Audio"),
            Self::Metadata => write!(f, "Metadata"),
        }
    }
}

/// A single crosspoint (input-to-output mapping).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crosspoint {
    /// Input index (source).
    pub input: usize,
    /// Output index (destination).
    pub output: usize,
    /// Signal layer.
    pub layer: SignalLayer,
    /// Whether this crosspoint is locked.
    pub locked: bool,
}

impl Crosspoint {
    /// Create a new crosspoint.
    pub fn new(input: usize, output: usize, layer: SignalLayer) -> Self {
        Self {
            input,
            output,
            layer,
            locked: false,
        }
    }
}

impl fmt::Display for Crosspoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{} -> {} {}",
            self.layer,
            self.input,
            self.output,
            if self.locked { "[LOCKED]" } else { "" }
        )
    }
}

/// Error type for crosspoint operations.
#[derive(Debug, Clone)]
pub enum CrosspointError {
    /// Input index out of range.
    InputOutOfRange(usize),
    /// Output index out of range.
    OutputOutOfRange(usize),
    /// Crosspoint is locked and cannot be changed.
    Locked {
        /// Output index.
        output: usize,
        /// Signal layer.
        layer: SignalLayer,
    },
    /// Salvo not found.
    SalvoNotFound(String),
}

impl fmt::Display for CrosspointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputOutOfRange(i) => write!(f, "Input {i} out of range"),
            Self::OutputOutOfRange(o) => write!(f, "Output {o} out of range"),
            Self::Locked { output, layer } => {
                write!(f, "Output {output} is locked on layer {layer}")
            }
            Self::SalvoNotFound(name) => write!(f, "Salvo '{name}' not found"),
        }
    }
}

/// A salvo is a named preset of crosspoint routes.
#[derive(Debug, Clone)]
pub struct Salvo {
    /// Name of the salvo.
    pub name: String,
    /// Routes: (layer, output) -> input.
    pub routes: HashMap<(SignalLayer, usize), usize>,
}

impl Salvo {
    /// Create a new empty salvo.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            routes: HashMap::new(),
        }
    }

    /// Add a route to the salvo.
    pub fn add_route(&mut self, layer: SignalLayer, output: usize, input: usize) {
        self.routes.insert((layer, output), input);
    }

    /// Get the number of routes in this salvo.
    pub fn route_count(&self) -> usize {
        self.routes.len()
    }
}

/// Crosspoint matrix configuration.
#[derive(Debug, Clone)]
pub struct CrosspointMatrixConfig {
    /// Number of inputs.
    pub num_inputs: usize,
    /// Number of outputs.
    pub num_outputs: usize,
    /// Enabled signal layers.
    pub layers: Vec<SignalLayer>,
}

impl CrosspointMatrixConfig {
    /// Create a new configuration.
    pub fn new(num_inputs: usize, num_outputs: usize) -> Self {
        Self {
            num_inputs,
            num_outputs,
            layers: vec![SignalLayer::Video],
        }
    }

    /// Create with all standard layers.
    pub fn with_all_layers(num_inputs: usize, num_outputs: usize) -> Self {
        Self {
            num_inputs,
            num_outputs,
            layers: vec![
                SignalLayer::Video,
                SignalLayer::Key,
                SignalLayer::Audio,
                SignalLayer::Metadata,
            ],
        }
    }
}

/// Crosspoint matrix router.
#[derive(Debug)]
pub struct CrosspointMatrix {
    /// Configuration.
    config: CrosspointMatrixConfig,
    /// Current routes: (layer, output) -> crosspoint.
    routes: HashMap<(SignalLayer, usize), Crosspoint>,
    /// Saved salvos.
    salvos: HashMap<String, Salvo>,
}

impl CrosspointMatrix {
    /// Create a new crosspoint matrix.
    pub fn new(config: CrosspointMatrixConfig) -> Self {
        let mut routes = HashMap::new();
        // Initialize all outputs to input 0
        for &layer in &config.layers {
            for out in 0..config.num_outputs {
                routes.insert((layer, out), Crosspoint::new(0, out, layer));
            }
        }

        Self {
            config,
            routes,
            salvos: HashMap::new(),
        }
    }

    /// Set a crosspoint route.
    pub fn route(
        &mut self,
        layer: SignalLayer,
        input: usize,
        output: usize,
    ) -> Result<(), CrosspointError> {
        if input >= self.config.num_inputs {
            return Err(CrosspointError::InputOutOfRange(input));
        }
        if output >= self.config.num_outputs {
            return Err(CrosspointError::OutputOutOfRange(output));
        }

        let key = (layer, output);
        if let Some(cp) = self.routes.get(&key) {
            if cp.locked {
                return Err(CrosspointError::Locked { output, layer });
            }
        }

        self.routes
            .insert(key, Crosspoint::new(input, output, layer));
        Ok(())
    }

    /// Get the current input routed to an output on a given layer.
    pub fn get_route(&self, layer: SignalLayer, output: usize) -> Option<usize> {
        self.routes.get(&(layer, output)).map(|cp| cp.input)
    }

    /// Get a crosspoint reference.
    pub fn get_crosspoint(&self, layer: SignalLayer, output: usize) -> Option<&Crosspoint> {
        self.routes.get(&(layer, output))
    }

    /// Lock a crosspoint so it cannot be changed.
    pub fn lock(&mut self, layer: SignalLayer, output: usize) -> Result<(), CrosspointError> {
        if output >= self.config.num_outputs {
            return Err(CrosspointError::OutputOutOfRange(output));
        }
        if let Some(cp) = self.routes.get_mut(&(layer, output)) {
            cp.locked = true;
            Ok(())
        } else {
            Err(CrosspointError::OutputOutOfRange(output))
        }
    }

    /// Unlock a crosspoint.
    pub fn unlock(&mut self, layer: SignalLayer, output: usize) -> Result<(), CrosspointError> {
        if output >= self.config.num_outputs {
            return Err(CrosspointError::OutputOutOfRange(output));
        }
        if let Some(cp) = self.routes.get_mut(&(layer, output)) {
            cp.locked = false;
            Ok(())
        } else {
            Err(CrosspointError::OutputOutOfRange(output))
        }
    }

    /// Save the current state as a salvo.
    pub fn save_salvo(&mut self, name: &str) {
        let mut salvo = Salvo::new(name);
        for (&(layer, output), cp) in &self.routes {
            salvo.add_route(layer, output, cp.input);
        }
        self.salvos.insert(name.to_string(), salvo);
    }

    /// Recall a salvo (apply saved routes, skipping locked crosspoints).
    pub fn recall_salvo(&mut self, name: &str) -> Result<usize, CrosspointError> {
        let salvo = self
            .salvos
            .get(name)
            .cloned()
            .ok_or_else(|| CrosspointError::SalvoNotFound(name.to_string()))?;

        let mut applied = 0;
        for (&(layer, output), &input) in &salvo.routes {
            if let Some(cp) = self.routes.get(&(layer, output)) {
                if cp.locked {
                    continue;
                }
            }
            if input < self.config.num_inputs && output < self.config.num_outputs {
                self.routes
                    .insert((layer, output), Crosspoint::new(input, output, layer));
                applied += 1;
            }
        }

        Ok(applied)
    }

    /// Get the number of inputs.
    pub fn num_inputs(&self) -> usize {
        self.config.num_inputs
    }

    /// Get the number of outputs.
    pub fn num_outputs(&self) -> usize {
        self.config.num_outputs
    }

    /// Get the list of salvos.
    pub fn salvo_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.salvos.keys().cloned().collect();
        names.sort();
        names
    }

    /// Get total number of active routes.
    pub fn active_route_count(&self) -> usize {
        self.routes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_layer_display() {
        assert_eq!(format!("{}", SignalLayer::Video), "Video");
        assert_eq!(format!("{}", SignalLayer::Audio), "Audio");
    }

    #[test]
    fn test_crosspoint_creation() {
        let cp = Crosspoint::new(3, 1, SignalLayer::Video);
        assert_eq!(cp.input, 3);
        assert_eq!(cp.output, 1);
        assert!(!cp.locked);
    }

    #[test]
    fn test_crosspoint_display() {
        let cp = Crosspoint::new(2, 0, SignalLayer::Video);
        let s = format!("{cp}");
        assert!(s.contains("Video"));
        assert!(s.contains("2"));
    }

    #[test]
    fn test_matrix_creation() {
        let config = CrosspointMatrixConfig::new(8, 4);
        let matrix = CrosspointMatrix::new(config);
        assert_eq!(matrix.num_inputs(), 8);
        assert_eq!(matrix.num_outputs(), 4);
    }

    #[test]
    fn test_matrix_default_routing() {
        let config = CrosspointMatrixConfig::new(8, 4);
        let matrix = CrosspointMatrix::new(config);
        // All outputs default to input 0
        for out in 0..4 {
            assert_eq!(matrix.get_route(SignalLayer::Video, out), Some(0));
        }
    }

    #[test]
    fn test_matrix_route_change() {
        let config = CrosspointMatrixConfig::new(8, 4);
        let mut matrix = CrosspointMatrix::new(config);
        matrix
            .route(SignalLayer::Video, 5, 2)
            .expect("should succeed in test");
        assert_eq!(matrix.get_route(SignalLayer::Video, 2), Some(5));
    }

    #[test]
    fn test_matrix_route_input_out_of_range() {
        let config = CrosspointMatrixConfig::new(4, 2);
        let mut matrix = CrosspointMatrix::new(config);
        let result = matrix.route(SignalLayer::Video, 10, 0);
        assert!(matches!(result, Err(CrosspointError::InputOutOfRange(10))));
    }

    #[test]
    fn test_matrix_route_output_out_of_range() {
        let config = CrosspointMatrixConfig::new(4, 2);
        let mut matrix = CrosspointMatrix::new(config);
        let result = matrix.route(SignalLayer::Video, 0, 10);
        assert!(matches!(result, Err(CrosspointError::OutputOutOfRange(10))));
    }

    #[test]
    fn test_matrix_lock_unlock() {
        let config = CrosspointMatrixConfig::new(8, 4);
        let mut matrix = CrosspointMatrix::new(config);
        matrix
            .route(SignalLayer::Video, 3, 0)
            .expect("should succeed in test");
        matrix
            .lock(SignalLayer::Video, 0)
            .expect("should succeed in test");

        // Routing to a locked output should fail
        let result = matrix.route(SignalLayer::Video, 5, 0);
        assert!(matches!(result, Err(CrosspointError::Locked { .. })));

        // Unlock and try again
        matrix
            .unlock(SignalLayer::Video, 0)
            .expect("should succeed in test");
        assert!(matrix.route(SignalLayer::Video, 5, 0).is_ok());
    }

    #[test]
    fn test_salvo_creation() {
        let mut salvo = Salvo::new("show_a");
        salvo.add_route(SignalLayer::Video, 0, 3);
        salvo.add_route(SignalLayer::Video, 1, 5);
        assert_eq!(salvo.route_count(), 2);
        assert_eq!(salvo.name, "show_a");
    }

    #[test]
    fn test_matrix_save_recall_salvo() {
        let config = CrosspointMatrixConfig::new(8, 4);
        let mut matrix = CrosspointMatrix::new(config);

        // Set up a state
        matrix
            .route(SignalLayer::Video, 3, 0)
            .expect("should succeed in test");
        matrix
            .route(SignalLayer::Video, 5, 1)
            .expect("should succeed in test");
        matrix.save_salvo("preset_1");

        // Change routes
        matrix
            .route(SignalLayer::Video, 0, 0)
            .expect("should succeed in test");
        matrix
            .route(SignalLayer::Video, 0, 1)
            .expect("should succeed in test");

        // Recall salvo
        let applied = matrix
            .recall_salvo("preset_1")
            .expect("should succeed in test");
        assert!(applied > 0);
        assert_eq!(matrix.get_route(SignalLayer::Video, 0), Some(3));
        assert_eq!(matrix.get_route(SignalLayer::Video, 1), Some(5));
    }

    #[test]
    fn test_matrix_recall_salvo_not_found() {
        let config = CrosspointMatrixConfig::new(4, 2);
        let mut matrix = CrosspointMatrix::new(config);
        let result = matrix.recall_salvo("nonexistent");
        assert!(matches!(result, Err(CrosspointError::SalvoNotFound(_))));
    }

    #[test]
    fn test_matrix_recall_salvo_respects_locks() {
        let config = CrosspointMatrixConfig::new(8, 4);
        let mut matrix = CrosspointMatrix::new(config);

        matrix
            .route(SignalLayer::Video, 7, 0)
            .expect("should succeed in test");
        matrix.save_salvo("locked_test");

        matrix
            .route(SignalLayer::Video, 0, 0)
            .expect("should succeed in test");
        matrix
            .lock(SignalLayer::Video, 0)
            .expect("should succeed in test");

        matrix
            .recall_salvo("locked_test")
            .expect("should succeed in test");
        // Output 0 should still be 0 because it's locked
        assert_eq!(matrix.get_route(SignalLayer::Video, 0), Some(0));
    }

    #[test]
    fn test_matrix_salvo_names() {
        let config = CrosspointMatrixConfig::new(4, 2);
        let mut matrix = CrosspointMatrix::new(config);
        matrix.save_salvo("b_salvo");
        matrix.save_salvo("a_salvo");

        let names = matrix.salvo_names();
        assert_eq!(names, vec!["a_salvo", "b_salvo"]);
    }

    #[test]
    fn test_matrix_all_layers() {
        let config = CrosspointMatrixConfig::with_all_layers(4, 2);
        let matrix = CrosspointMatrix::new(config);
        // Should have routes for all 4 layers x 2 outputs = 8 routes
        assert_eq!(matrix.active_route_count(), 8);
    }
}
