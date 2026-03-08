//! Shot list export functionality.

use crate::error::{ShotError, ShotResult};
use crate::types::{Scene, Shot};
use std::io::Write;

/// Shot list exporter.
pub struct ShotListExporter;

impl ShotListExporter {
    /// Create a new shot list exporter.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Export shots to CSV format.
    ///
    /// # Errors
    ///
    /// Returns error if writing fails.
    pub fn export_csv<W: Write>(&self, shots: &[Shot], writer: &mut W) -> ShotResult<()> {
        // Write header
        writeln!(
            writer,
            "Shot,Start,End,Duration,Type,Angle,Coverage,Transition,Confidence"
        )
        .map_err(|e| ShotError::ExportFailed(e.to_string()))?;

        // Write each shot
        for shot in shots {
            writeln!(
                writer,
                "{},{},{},{:.2},{},{},{},{},{:.2}",
                shot.id,
                shot.start.pts,
                shot.end.pts,
                shot.duration_seconds(),
                shot.shot_type.abbreviation(),
                shot.angle.name(),
                shot.coverage.name(),
                shot.transition.name(),
                shot.confidence
            )
            .map_err(|e| ShotError::ExportFailed(e.to_string()))?;
        }

        Ok(())
    }

    /// Export shots to JSON format.
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails.
    pub fn export_json(&self, shots: &[Shot]) -> ShotResult<String> {
        serde_json::to_string_pretty(&shots)
            .map_err(|e| ShotError::SerializationError(e.to_string()))
    }

    /// Export scenes to JSON format.
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails.
    pub fn export_scenes_json(&self, scenes: &[Scene]) -> ShotResult<String> {
        serde_json::to_string_pretty(&scenes)
            .map_err(|e| ShotError::SerializationError(e.to_string()))
    }

    /// Export comprehensive shot list with all metadata.
    ///
    /// # Errors
    ///
    /// Returns error if writing fails.
    pub fn export_detailed<W: Write>(&self, shots: &[Shot], writer: &mut W) -> ShotResult<()> {
        writeln!(writer, "SHOT LIST").map_err(|e| ShotError::ExportFailed(e.to_string()))?;
        writeln!(writer, "=========\n").map_err(|e| ShotError::ExportFailed(e.to_string()))?;

        for shot in shots {
            writeln!(writer, "Shot #{}", shot.id)
                .map_err(|e| ShotError::ExportFailed(e.to_string()))?;
            writeln!(
                writer,
                "  Time: {:.2}s - {:.2}s (Duration: {:.2}s)",
                shot.start.to_seconds(),
                shot.end.to_seconds(),
                shot.duration_seconds()
            )
            .map_err(|e| ShotError::ExportFailed(e.to_string()))?;
            writeln!(writer, "  Type: {}", shot.shot_type.name())
                .map_err(|e| ShotError::ExportFailed(e.to_string()))?;
            writeln!(writer, "  Angle: {}", shot.angle.name())
                .map_err(|e| ShotError::ExportFailed(e.to_string()))?;
            writeln!(writer, "  Coverage: {}", shot.coverage.name())
                .map_err(|e| ShotError::ExportFailed(e.to_string()))?;
            writeln!(writer, "  Transition: {}", shot.transition.name())
                .map_err(|e| ShotError::ExportFailed(e.to_string()))?;

            if !shot.movements.is_empty() {
                writeln!(writer, "  Movements:")
                    .map_err(|e| ShotError::ExportFailed(e.to_string()))?;
                for movement in &shot.movements {
                    writeln!(
                        writer,
                        "    - {} ({:.2}s - {:.2}s)",
                        movement.movement_type.name(),
                        movement.start,
                        movement.end
                    )
                    .map_err(|e| ShotError::ExportFailed(e.to_string()))?;
                }
            }

            writeln!(
                writer,
                "  Composition: Rule of Thirds: {:.2}, Symmetry: {:.2}, Balance: {:.2}",
                shot.composition.rule_of_thirds,
                shot.composition.symmetry,
                shot.composition.balance
            )
            .map_err(|e| ShotError::ExportFailed(e.to_string()))?;
            writeln!(writer, "  Confidence: {:.2}\n", shot.confidence)
                .map_err(|e| ShotError::ExportFailed(e.to_string()))?;
        }

        Ok(())
    }
}

impl Default for ShotListExporter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CameraAngle, CompositionAnalysis, CoverageType, ShotType, TransitionType};
    use oximedia_core::types::{Rational, Timestamp};

    #[test]
    fn test_exporter_creation() {
        let _exporter = ShotListExporter::new();
    }

    #[test]
    fn test_export_csv() {
        let exporter = ShotListExporter::new();
        let shot = Shot {
            id: 1,
            start: Timestamp::new(0, Rational::new(1, 30)),
            end: Timestamp::new(60, Rational::new(1, 30)),
            shot_type: ShotType::MediumShot,
            angle: CameraAngle::EyeLevel,
            movements: Vec::new(),
            composition: CompositionAnalysis {
                rule_of_thirds: 0.5,
                symmetry: 0.5,
                balance: 0.5,
                leading_lines: 0.5,
                depth: 0.5,
            },
            coverage: CoverageType::Master,
            confidence: 0.8,
            transition: TransitionType::Cut,
        };

        let mut output = Vec::new();
        let result = exporter.export_csv(&[shot], &mut output);
        assert!(result.is_ok());
        assert!(!output.is_empty());
    }

    #[test]
    fn test_export_json() {
        let exporter = ShotListExporter::new();
        let shot = Shot {
            id: 1,
            start: Timestamp::new(0, Rational::new(1, 30)),
            end: Timestamp::new(60, Rational::new(1, 30)),
            shot_type: ShotType::MediumShot,
            angle: CameraAngle::EyeLevel,
            movements: Vec::new(),
            composition: CompositionAnalysis {
                rule_of_thirds: 0.5,
                symmetry: 0.5,
                balance: 0.5,
                leading_lines: 0.5,
                depth: 0.5,
            },
            coverage: CoverageType::Master,
            confidence: 0.8,
            transition: TransitionType::Cut,
        };

        let result = exporter.export_json(&[shot]);
        assert!(result.is_ok());
    }
}
