//! Crosspoint routing matrix implementation for any-to-any audio routing.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a crosspoint in the routing matrix
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CrosspointId {
    /// Input channel index
    pub input: usize,
    /// Output channel index
    pub output: usize,
}

impl CrosspointId {
    /// Create a new crosspoint identifier
    #[must_use]
    pub const fn new(input: usize, output: usize) -> Self {
        Self { input, output }
    }
}

/// State of a crosspoint connection
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum CrosspointState {
    /// Crosspoint is disconnected
    #[default]
    Disconnected,
    /// Crosspoint is connected with optional gain (in dB)
    Connected { gain_db: f32 },
}

/// Crosspoint routing matrix for full any-to-any routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrosspointMatrix {
    /// Number of input channels
    inputs: usize,
    /// Number of output channels
    outputs: usize,
    /// Crosspoint states
    crosspoints: HashMap<CrosspointId, CrosspointState>,
    /// Input labels
    input_labels: Vec<String>,
    /// Output labels
    output_labels: Vec<String>,
}

impl CrosspointMatrix {
    /// Create a new crosspoint matrix
    #[must_use]
    pub fn new(inputs: usize, outputs: usize) -> Self {
        Self {
            inputs,
            outputs,
            crosspoints: HashMap::new(),
            input_labels: (0..inputs).map(|i| format!("Input {}", i + 1)).collect(),
            output_labels: (0..outputs).map(|i| format!("Output {}", i + 1)).collect(),
        }
    }

    /// Get the number of inputs
    #[must_use]
    pub const fn input_count(&self) -> usize {
        self.inputs
    }

    /// Get the number of outputs
    #[must_use]
    pub const fn output_count(&self) -> usize {
        self.outputs
    }

    /// Connect an input to an output with optional gain
    pub fn connect(
        &mut self,
        input: usize,
        output: usize,
        gain_db: Option<f32>,
    ) -> Result<(), MatrixError> {
        if input >= self.inputs {
            return Err(MatrixError::InvalidInput(input));
        }
        if output >= self.outputs {
            return Err(MatrixError::InvalidOutput(output));
        }

        let crosspoint = CrosspointId::new(input, output);
        self.crosspoints.insert(
            crosspoint,
            CrosspointState::Connected {
                gain_db: gain_db.unwrap_or(0.0),
            },
        );
        Ok(())
    }

    /// Disconnect an input from an output
    pub fn disconnect(&mut self, input: usize, output: usize) -> Result<(), MatrixError> {
        if input >= self.inputs {
            return Err(MatrixError::InvalidInput(input));
        }
        if output >= self.outputs {
            return Err(MatrixError::InvalidOutput(output));
        }

        let crosspoint = CrosspointId::new(input, output);
        self.crosspoints
            .insert(crosspoint, CrosspointState::Disconnected);
        Ok(())
    }

    /// Check if an input is connected to an output
    #[must_use]
    pub fn is_connected(&self, input: usize, output: usize) -> bool {
        let crosspoint = CrosspointId::new(input, output);
        matches!(
            self.crosspoints.get(&crosspoint),
            Some(CrosspointState::Connected { .. })
        )
    }

    /// Get the state of a crosspoint
    #[must_use]
    pub fn get_state(&self, input: usize, output: usize) -> CrosspointState {
        let crosspoint = CrosspointId::new(input, output);
        self.crosspoints
            .get(&crosspoint)
            .copied()
            .unwrap_or(CrosspointState::Disconnected)
    }

    /// Set gain for a crosspoint (must be connected)
    pub fn set_gain(
        &mut self,
        input: usize,
        output: usize,
        gain_db: f32,
    ) -> Result<(), MatrixError> {
        if input >= self.inputs {
            return Err(MatrixError::InvalidInput(input));
        }
        if output >= self.outputs {
            return Err(MatrixError::InvalidOutput(output));
        }

        let crosspoint = CrosspointId::new(input, output);
        if let Some(state) = self.crosspoints.get_mut(&crosspoint) {
            if let CrosspointState::Connected { gain_db: gain } = state {
                *gain = gain_db;
                return Ok(());
            }
        }
        Err(MatrixError::NotConnected(input, output))
    }

    /// Get all inputs connected to an output
    #[must_use]
    pub fn get_inputs_for_output(&self, output: usize) -> Vec<usize> {
        (0..self.inputs)
            .filter(|&input| self.is_connected(input, output))
            .collect()
    }

    /// Get all outputs connected to an input
    #[must_use]
    pub fn get_outputs_for_input(&self, input: usize) -> Vec<usize> {
        (0..self.outputs)
            .filter(|&output| self.is_connected(input, output))
            .collect()
    }

    /// Set label for an input
    pub fn set_input_label(&mut self, input: usize, label: String) -> Result<(), MatrixError> {
        if input >= self.inputs {
            return Err(MatrixError::InvalidInput(input));
        }
        self.input_labels[input] = label;
        Ok(())
    }

    /// Set label for an output
    pub fn set_output_label(&mut self, output: usize, label: String) -> Result<(), MatrixError> {
        if output >= self.outputs {
            return Err(MatrixError::InvalidOutput(output));
        }
        self.output_labels[output] = label;
        Ok(())
    }

    /// Get label for an input
    #[must_use]
    pub fn get_input_label(&self, input: usize) -> Option<&str> {
        self.input_labels.get(input).map(String::as_str)
    }

    /// Get label for an output
    #[must_use]
    pub fn get_output_label(&self, output: usize) -> Option<&str> {
        self.output_labels.get(output).map(String::as_str)
    }

    /// Clear all connections
    pub fn clear_all(&mut self) {
        self.crosspoints.clear();
    }

    /// Get all active crosspoints
    #[must_use]
    pub fn get_active_crosspoints(&self) -> Vec<(CrosspointId, f32)> {
        self.crosspoints
            .iter()
            .filter_map(|(id, state)| {
                if let CrosspointState::Connected { gain_db } = state {
                    Some((*id, *gain_db))
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Errors that can occur in matrix operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum MatrixError {
    /// Invalid input index
    #[error("Invalid input index: {0}")]
    InvalidInput(usize),
    /// Invalid output index
    #[error("Invalid output index: {0}")]
    InvalidOutput(usize),
    /// Crosspoint not connected
    #[error("Crosspoint not connected: input {0} to output {1}")]
    NotConnected(usize, usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crosspoint_id() {
        let cp = CrosspointId::new(5, 10);
        assert_eq!(cp.input, 5);
        assert_eq!(cp.output, 10);
    }

    #[test]
    fn test_matrix_creation() {
        let matrix = CrosspointMatrix::new(16, 8);
        assert_eq!(matrix.input_count(), 16);
        assert_eq!(matrix.output_count(), 8);
    }

    #[test]
    fn test_connect_disconnect() {
        let mut matrix = CrosspointMatrix::new(4, 4);

        assert!(!matrix.is_connected(0, 0));
        matrix
            .connect(0, 0, Some(0.0))
            .expect("should succeed in test");
        assert!(matrix.is_connected(0, 0));

        matrix.disconnect(0, 0).expect("should succeed in test");
        assert!(!matrix.is_connected(0, 0));
    }

    #[test]
    fn test_gain_control() {
        let mut matrix = CrosspointMatrix::new(4, 4);

        matrix
            .connect(0, 0, Some(-6.0))
            .expect("should succeed in test");
        if let CrosspointState::Connected { gain_db } = matrix.get_state(0, 0) {
            assert!((gain_db - (-6.0)).abs() < f32::EPSILON);
        } else {
            panic!("Expected connected state");
        }

        matrix.set_gain(0, 0, -3.0).expect("should succeed in test");
        if let CrosspointState::Connected { gain_db } = matrix.get_state(0, 0) {
            assert!((gain_db - (-3.0)).abs() < f32::EPSILON);
        } else {
            panic!("Expected connected state");
        }
    }

    #[test]
    fn test_labels() {
        let mut matrix = CrosspointMatrix::new(2, 2);

        matrix
            .set_input_label(0, "Mic 1".to_string())
            .expect("should succeed in test");
        matrix
            .set_output_label(1, "Speaker R".to_string())
            .expect("should succeed in test");

        assert_eq!(matrix.get_input_label(0), Some("Mic 1"));
        assert_eq!(matrix.get_output_label(1), Some("Speaker R"));
    }

    #[test]
    fn test_routing_queries() {
        let mut matrix = CrosspointMatrix::new(4, 4);

        matrix.connect(0, 0, None).expect("should succeed in test");
        matrix.connect(0, 1, None).expect("should succeed in test");
        matrix.connect(1, 0, None).expect("should succeed in test");

        let outputs = matrix.get_outputs_for_input(0);
        assert_eq!(outputs.len(), 2);
        assert!(outputs.contains(&0));
        assert!(outputs.contains(&1));

        let inputs = matrix.get_inputs_for_output(0);
        assert_eq!(inputs.len(), 2);
        assert!(inputs.contains(&0));
        assert!(inputs.contains(&1));
    }

    #[test]
    fn test_invalid_indices() {
        let mut matrix = CrosspointMatrix::new(4, 4);

        assert!(matches!(
            matrix.connect(10, 0, None),
            Err(MatrixError::InvalidInput(10))
        ));
        assert!(matches!(
            matrix.connect(0, 10, None),
            Err(MatrixError::InvalidOutput(10))
        ));
    }
}
