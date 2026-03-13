use super::checkpoint::CheckpointStore;
use super::processor::Processor;

/// Unique identifier for a node in the pipeline tree.
pub type NodeId = usize;

/// A node in the processor tree. Each node wraps a single processor
/// and tracks its checkpoints and children (for multi-output processors).
pub struct PipelineNode {
    pub id: NodeId,
    pub processor: Box<dyn Processor>,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    /// Which output branch of the parent this node reads from (0 for single-output).
    #[allow(dead_code)]
    pub parent_output_branch: usize,
    pub checkpoints: CheckpointStore,
}

impl PipelineNode {
    pub fn new(id: NodeId, processor: Box<dyn Processor>) -> Self {
        let interval = processor.checkpoint_interval();
        Self {
            id,
            processor,
            parent: None,
            children: Vec::new(),
            parent_output_branch: 0,
            checkpoints: CheckpointStore::new(interval),
        }
    }

    /// Invalidate cached state (checkpoints) when config changes.
    pub fn invalidate(&mut self) {
        self.checkpoints.clear();
        self.processor.reset();
    }
}
