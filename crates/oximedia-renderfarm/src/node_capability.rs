#![allow(dead_code)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::return_self_not_must_use)]
//! Node capability modelling for render farm workers.
//!
//! Tracks what rendering features each worker node supports and
//! validates that a job's requirements can be satisfied before dispatch.

use std::collections::HashSet;

/// A discrete capability that a render node may possess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderCapability {
    /// NVIDIA CUDA acceleration.
    CudaGpu,
    /// AMD `ROCm` acceleration.
    RocmGpu,
    /// Apple Metal acceleration.
    MetalGpu,
    /// CPU-only software rendering.
    CpuRender,
    /// High-memory node (≥ 128 GiB RAM).
    HighMemory,
    /// NVMe-backed fast local scratch storage.
    FastLocalStorage,
    /// Access to shared network-attached storage.
    NetworkStorage,
    /// Ability to run Blender render engine.
    BlenderEngine,
    /// Ability to run Arnold render engine.
    ArnoldEngine,
    /// Ability to run V-Ray render engine.
    VRayEngine,
    /// Ability to run `RenderMan` render engine.
    RenderManEngine,
    /// Hardware-accelerated video encoding (NVENC/VCE/etc.).
    HardwareVideoEncode,
    /// Support for 3D LUT colour management.
    LutColorManagement,
    /// Support for distributed tile rendering.
    TileRendering,
}

impl RenderCapability {
    /// Returns a human-readable name for the capability.
    pub fn capability_name(self) -> &'static str {
        match self {
            Self::CudaGpu => "CUDA GPU",
            Self::RocmGpu => "ROCm GPU",
            Self::MetalGpu => "Metal GPU",
            Self::CpuRender => "CPU Render",
            Self::HighMemory => "High Memory",
            Self::FastLocalStorage => "Fast Local Storage",
            Self::NetworkStorage => "Network Storage",
            Self::BlenderEngine => "Blender Engine",
            Self::ArnoldEngine => "Arnold Engine",
            Self::VRayEngine => "V-Ray Engine",
            Self::RenderManEngine => "RenderMan Engine",
            Self::HardwareVideoEncode => "Hardware Video Encode",
            Self::LutColorManagement => "LUT Color Management",
            Self::TileRendering => "Tile Rendering",
        }
    }

    /// Returns `true` if this capability relates to GPU acceleration.
    pub fn is_gpu(self) -> bool {
        matches!(self, Self::CudaGpu | Self::RocmGpu | Self::MetalGpu)
    }

    /// Returns `true` if this capability relates to a specific render engine.
    pub fn is_render_engine(self) -> bool {
        matches!(
            self,
            Self::BlenderEngine | Self::ArnoldEngine | Self::VRayEngine | Self::RenderManEngine
        )
    }
}

// ── NodeCapabilitySet ──────────────────────────────────────────────────────

/// The complete set of capabilities advertised by one render node.
#[derive(Debug, Clone, Default)]
pub struct NodeCapabilitySet {
    caps: HashSet<RenderCapability>,
    node_id: String,
}

impl NodeCapabilitySet {
    /// Creates an empty capability set for the given node.
    pub fn new(node_id: impl Into<String>) -> Self {
        Self {
            caps: HashSet::new(),
            node_id: node_id.into(),
        }
    }

    /// Returns the node identifier.
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Adds a capability to this node's set.
    pub fn add(&mut self, cap: RenderCapability) {
        self.caps.insert(cap);
    }

    /// Returns `true` if the node has the requested capability.
    pub fn has(&self, cap: RenderCapability) -> bool {
        self.caps.contains(&cap)
    }

    /// Returns the number of capabilities the node advertises.
    pub fn count(&self) -> usize {
        self.caps.len()
    }

    /// Returns `true` if this node satisfies all capabilities in `requirements`.
    pub fn meets_requirements(&self, requirements: &[RenderCapability]) -> bool {
        requirements.iter().all(|r| self.has(*r))
    }

    /// Returns an iterator over the capabilities.
    pub fn iter(&self) -> impl Iterator<Item = &RenderCapability> {
        self.caps.iter()
    }
}

// ── CapabilityRequirement ──────────────────────────────────────────────────

/// A named set of capability requirements that a job demands.
#[derive(Debug, Clone, Default)]
pub struct CapabilityRequirement {
    required: Vec<RenderCapability>,
    /// Optional human-readable label for diagnostics.
    pub label: String,
}

impl CapabilityRequirement {
    /// Creates a new, empty requirement set with a label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            required: Vec::new(),
            label: label.into(),
        }
    }

    /// Adds a required capability.
    pub fn require(&mut self, cap: RenderCapability) {
        if !self.required.contains(&cap) {
            self.required.push(cap);
        }
    }

    /// Returns `true` if the given node satisfies **all** requirements.
    pub fn all_met(&self, node: &NodeCapabilitySet) -> bool {
        node.meets_requirements(&self.required)
    }

    /// Returns the list of capabilities that are **not** met by `node`.
    pub fn unmet_by(&self, node: &NodeCapabilitySet) -> Vec<RenderCapability> {
        self.required
            .iter()
            .filter(|&&c| !node.has(c))
            .copied()
            .collect()
    }

    /// Returns the number of required capabilities.
    pub fn len(&self) -> usize {
        self.required.len()
    }

    /// Returns `true` if no capabilities are required.
    pub fn is_empty(&self) -> bool {
        self.required.is_empty()
    }
}

// ─────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn full_node() -> NodeCapabilitySet {
        let mut n = NodeCapabilitySet::new("node-01");
        n.add(RenderCapability::CudaGpu);
        n.add(RenderCapability::BlenderEngine);
        n.add(RenderCapability::HighMemory);
        n.add(RenderCapability::TileRendering);
        n.add(RenderCapability::NetworkStorage);
        n
    }

    #[test]
    fn test_capability_name_not_empty() {
        let caps = [
            RenderCapability::CudaGpu,
            RenderCapability::RocmGpu,
            RenderCapability::CpuRender,
            RenderCapability::BlenderEngine,
        ];
        for cap in caps {
            assert!(!cap.capability_name().is_empty());
        }
    }

    #[test]
    fn test_is_gpu_flags() {
        assert!(RenderCapability::CudaGpu.is_gpu());
        assert!(RenderCapability::MetalGpu.is_gpu());
        assert!(!RenderCapability::CpuRender.is_gpu());
    }

    #[test]
    fn test_is_render_engine_flags() {
        assert!(RenderCapability::BlenderEngine.is_render_engine());
        assert!(RenderCapability::ArnoldEngine.is_render_engine());
        assert!(!RenderCapability::CudaGpu.is_render_engine());
    }

    #[test]
    fn test_node_has_added_capability() {
        let n = full_node();
        assert!(n.has(RenderCapability::CudaGpu));
        assert!(n.has(RenderCapability::BlenderEngine));
    }

    #[test]
    fn test_node_missing_capability() {
        let n = full_node();
        assert!(!n.has(RenderCapability::VRayEngine));
    }

    #[test]
    fn test_node_count() {
        let n = full_node();
        assert_eq!(n.count(), 5);
    }

    #[test]
    fn test_node_id() {
        let n = NodeCapabilitySet::new("render-box-42");
        assert_eq!(n.node_id(), "render-box-42");
    }

    #[test]
    fn test_meets_requirements_all_present() {
        let n = full_node();
        assert!(n.meets_requirements(&[RenderCapability::CudaGpu, RenderCapability::BlenderEngine]));
    }

    #[test]
    fn test_meets_requirements_one_missing() {
        let n = full_node();
        assert!(!n.meets_requirements(&[
            RenderCapability::CudaGpu,
            RenderCapability::VRayEngine // not present
        ]));
    }

    #[test]
    fn test_capability_requirement_all_met() {
        let n = full_node();
        let mut req = CapabilityRequirement::new("blender-cuda");
        req.require(RenderCapability::CudaGpu);
        req.require(RenderCapability::BlenderEngine);
        assert!(req.all_met(&n));
    }

    #[test]
    fn test_capability_requirement_unmet_by() {
        let n = full_node();
        let mut req = CapabilityRequirement::new("vray-job");
        req.require(RenderCapability::VRayEngine);
        req.require(RenderCapability::CudaGpu);
        let unmet = req.unmet_by(&n);
        assert_eq!(unmet, vec![RenderCapability::VRayEngine]);
    }

    #[test]
    fn test_capability_requirement_empty() {
        let req = CapabilityRequirement::new("any");
        assert!(req.is_empty());
        let n = NodeCapabilitySet::new("x");
        assert!(req.all_met(&n)); // vacuously true
    }

    #[test]
    fn test_add_deduplicates() {
        let mut n = NodeCapabilitySet::new("n");
        n.add(RenderCapability::CpuRender);
        n.add(RenderCapability::CpuRender);
        assert_eq!(n.count(), 1);
    }

    #[test]
    fn test_require_deduplicates() {
        let mut req = CapabilityRequirement::new("dup");
        req.require(RenderCapability::HighMemory);
        req.require(RenderCapability::HighMemory);
        assert_eq!(req.len(), 1);
    }
}
