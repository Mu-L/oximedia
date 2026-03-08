//! GPU compute pass management — pass types, buffer bindings, and pass queues.

#![allow(dead_code)]
#![allow(clippy::cast_precision_loss)]

/// Category of work that a compute pass performs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PassType {
    /// Video processing (real-time).
    Video,
    /// Audio processing (real-time).
    Audio,
    /// Still-image processing.
    Image,
    /// Post-processing effects.
    PostProcess,
}

impl PassType {
    /// Returns `true` for pass types that operate in real-time context.
    #[must_use]
    pub fn is_real_time(&self) -> bool {
        matches!(self, Self::Video | Self::Audio)
    }
}

/// A binding between a GPU buffer slot and a logical buffer.
#[derive(Debug, Clone)]
pub struct BufferBinding {
    /// The shader binding slot index.
    pub slot: u8,
    /// Size of the buffer in bytes.
    pub size_bytes: u32,
    /// Whether the binding is read-only (i.e. an input buffer).
    pub read_only: bool,
}

impl BufferBinding {
    /// Creates a new `BufferBinding`.
    #[must_use]
    pub fn new(slot: u8, size_bytes: u32, read_only: bool) -> Self {
        Self {
            slot,
            size_bytes,
            read_only,
        }
    }

    /// Returns `true` if this binding is an input (read-only) binding.
    #[must_use]
    pub fn is_input(&self) -> bool {
        self.read_only
    }

    /// Returns `true` if this binding is an output (writable) binding.
    #[must_use]
    pub fn is_output(&self) -> bool {
        !self.read_only
    }
}

/// A single compute pass with a name, type, buffer bindings, and dispatch dimensions.
#[derive(Debug)]
pub struct ComputePass {
    /// Human-readable name for debugging.
    pub name: String,
    /// The category of this pass.
    pub pass_type: PassType,
    /// Buffer bindings used by this pass.
    pub bindings: Vec<BufferBinding>,
    /// Workgroup dispatch dimensions (x, y, z).
    pub workgroups: (u32, u32, u32),
}

impl ComputePass {
    /// Creates a new `ComputePass` with no bindings and a default dispatch of (1, 1, 1).
    #[must_use]
    pub fn new(name: impl Into<String>, pt: PassType) -> Self {
        Self {
            name: name.into(),
            pass_type: pt,
            bindings: Vec::new(),
            workgroups: (1, 1, 1),
        }
    }

    /// Adds a read-only (input) buffer binding on the given slot.
    pub fn add_input_binding(&mut self, slot: u8, size: u32) {
        self.bindings.push(BufferBinding::new(slot, size, true));
    }

    /// Adds a writable (output) buffer binding on the given slot.
    pub fn add_output_binding(&mut self, slot: u8, size: u32) {
        self.bindings.push(BufferBinding::new(slot, size, false));
    }

    /// Total work items = workgroups.x × workgroups.y × workgroups.z.
    #[must_use]
    pub fn total_work_items(&self) -> u64 {
        u64::from(self.workgroups.0) * u64::from(self.workgroups.1) * u64::from(self.workgroups.2)
    }

    /// Returns the number of bindings attached to this pass.
    #[must_use]
    pub fn binding_count(&self) -> usize {
        self.bindings.len()
    }
}

/// An ordered queue of [`ComputePass`] entries.
#[derive(Debug, Default)]
pub struct PassQueue {
    passes: Vec<ComputePass>,
}

impl PassQueue {
    /// Creates an empty `PassQueue`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a pass to the queue.
    pub fn add(&mut self, pass: ComputePass) {
        self.passes.push(pass);
    }

    /// Returns references to all passes whose type matches `pt`.
    #[must_use]
    pub fn passes_of_type(&self, pt: &PassType) -> Vec<&ComputePass> {
        self.passes.iter().filter(|p| &p.pass_type == pt).collect()
    }

    /// Total number of bindings across all passes.
    #[must_use]
    pub fn total_bindings(&self) -> usize {
        self.passes.iter().map(ComputePass::binding_count).sum()
    }

    /// Returns the number of passes in the queue.
    #[must_use]
    pub fn pass_count(&self) -> usize {
        self.passes.len()
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pass_type_video_is_real_time() {
        assert!(PassType::Video.is_real_time());
    }

    #[test]
    fn test_pass_type_audio_is_real_time() {
        assert!(PassType::Audio.is_real_time());
    }

    #[test]
    fn test_pass_type_image_not_real_time() {
        assert!(!PassType::Image.is_real_time());
    }

    #[test]
    fn test_pass_type_post_process_not_real_time() {
        assert!(!PassType::PostProcess.is_real_time());
    }

    #[test]
    fn test_buffer_binding_input() {
        let b = BufferBinding::new(0, 1024, true);
        assert!(b.is_input());
        assert!(!b.is_output());
    }

    #[test]
    fn test_buffer_binding_output() {
        let b = BufferBinding::new(1, 2048, false);
        assert!(b.is_output());
        assert!(!b.is_input());
    }

    #[test]
    fn test_compute_pass_new_defaults() {
        let pass = ComputePass::new("test", PassType::Image);
        assert_eq!(pass.name, "test");
        assert_eq!(pass.workgroups, (1, 1, 1));
        assert_eq!(pass.binding_count(), 0);
    }

    #[test]
    fn test_compute_pass_add_input_binding() {
        let mut pass = ComputePass::new("p", PassType::Video);
        pass.add_input_binding(0, 512);
        assert_eq!(pass.binding_count(), 1);
        assert!(pass.bindings[0].is_input());
    }

    #[test]
    fn test_compute_pass_add_output_binding() {
        let mut pass = ComputePass::new("p", PassType::Video);
        pass.add_output_binding(1, 512);
        assert_eq!(pass.binding_count(), 1);
        assert!(pass.bindings[0].is_output());
    }

    #[test]
    fn test_total_work_items_1x1x1() {
        let pass = ComputePass::new("p", PassType::Audio);
        assert_eq!(pass.total_work_items(), 1);
    }

    #[test]
    fn test_total_work_items_custom() {
        let mut pass = ComputePass::new("p", PassType::Image);
        pass.workgroups = (4, 8, 2);
        assert_eq!(pass.total_work_items(), 64);
    }

    #[test]
    fn test_pass_queue_add_and_count() {
        let mut q = PassQueue::new();
        q.add(ComputePass::new("a", PassType::Video));
        q.add(ComputePass::new("b", PassType::Image));
        assert_eq!(q.pass_count(), 2);
    }

    #[test]
    fn test_pass_queue_passes_of_type() {
        let mut q = PassQueue::new();
        q.add(ComputePass::new("v1", PassType::Video));
        q.add(ComputePass::new("i1", PassType::Image));
        q.add(ComputePass::new("v2", PassType::Video));
        let videos = q.passes_of_type(&PassType::Video);
        assert_eq!(videos.len(), 2);
    }

    #[test]
    fn test_pass_queue_passes_of_type_empty_result() {
        let mut q = PassQueue::new();
        q.add(ComputePass::new("a", PassType::Audio));
        let results = q.passes_of_type(&PassType::PostProcess);
        assert!(results.is_empty());
    }

    #[test]
    fn test_pass_queue_total_bindings() {
        let mut q = PassQueue::new();
        let mut p1 = ComputePass::new("p1", PassType::Video);
        p1.add_input_binding(0, 256);
        p1.add_output_binding(1, 256);
        let mut p2 = ComputePass::new("p2", PassType::Image);
        p2.add_input_binding(0, 128);
        q.add(p1);
        q.add(p2);
        assert_eq!(q.total_bindings(), 3);
    }

    #[test]
    fn test_pass_queue_empty() {
        let q = PassQueue::new();
        assert_eq!(q.pass_count(), 0);
        assert_eq!(q.total_bindings(), 0);
    }
}
