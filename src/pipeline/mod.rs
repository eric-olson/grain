pub mod cache;
pub mod checkpoint;
pub mod node;
pub mod offset_map;
pub mod processor;
pub mod processors;

use cache::OutputCache;
use node::{NodeId, PipelineNode};
use offset_map::OffsetTranslator;
use processor::Processor;

use crate::file_handler::MappedFile;

/// The processing pipeline. Holds a tree of processor nodes and manages
/// caching/checkpointing for efficient windowed data access.
pub struct Pipeline {
    nodes: Vec<PipelineNode>,
    next_id: NodeId,
    active_leaf: Option<NodeId>,
    cache: OutputCache,
    translator: OffsetTranslator,
}

impl Pipeline {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            next_id: 0,
            active_leaf: None,
            cache: OutputCache::new(),
            translator: OffsetTranslator::identity(),
        }
    }

    /// Whether the pipeline has any processors.
    pub fn is_active(&self) -> bool {
        !self.nodes.is_empty()
    }

    /// Add a processor to the end of the linear chain.
    /// Returns the new node's ID.
    pub fn push(&mut self, processor: Box<dyn Processor>) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;

        let mut node = PipelineNode::new(id, processor);

        // Link to previous tail
        if let Some(tail_id) = self.active_leaf {
            node.parent = Some(tail_id);
            if let Some(parent) = self.node_mut(tail_id) {
                parent.children.push(id);
            }
        }

        self.nodes.push(node);
        self.active_leaf = Some(id);
        self.rebuild_translator();
        self.cache.clear();
        id
    }

    /// Remove a processor by ID. Only supports removing the tail for now.
    pub fn remove(&mut self, id: NodeId) -> bool {
        let Some(idx) = self.nodes.iter().position(|n| n.id == id) else {
            return false;
        };

        let parent = self.nodes[idx].parent;

        // Unlink from parent
        if let Some(pid) = parent {
            if let Some(p) = self.node_mut(pid) {
                p.children.retain(|&c| c != id);
            }
        }

        // Reparent children to parent (for linear chain, just unlink)
        let children: Vec<NodeId> = self.nodes[idx].children.clone();
        for &child_id in &children {
            if let Some(child) = self.node_mut(child_id) {
                child.parent = parent;
            }
            if let Some(pid) = parent {
                if let Some(p) = self.node_mut(pid) {
                    if !p.children.contains(&child_id) {
                        p.children.push(child_id);
                    }
                }
            }
        }

        self.nodes.remove(idx);

        // Update active leaf
        if self.active_leaf == Some(id) {
            self.active_leaf = if self.nodes.is_empty() {
                None
            } else {
                // Find the node with no children
                self.nodes
                    .iter()
                    .find(|n| n.children.is_empty())
                    .map(|n| n.id)
            };
        }

        self.rebuild_translator();
        self.cache.clear();
        true
    }

    /// Move a processor up in the chain (swap with previous).
    pub fn move_up(&mut self, idx: usize) -> bool {
        if idx == 0 || idx >= self.nodes.len() {
            return false;
        }
        self.nodes.swap(idx, idx - 1);
        self.relink_chain();
        self.invalidate();
        true
    }

    /// Move a processor down in the chain (swap with next).
    pub fn move_down(&mut self, idx: usize) -> bool {
        if idx + 1 >= self.nodes.len() {
            return false;
        }
        self.nodes.swap(idx, idx + 1);
        self.relink_chain();
        self.invalidate();
        true
    }

    /// Rebuild parent/child links for a simple linear chain after reordering.
    fn relink_chain(&mut self) {
        for i in 0..self.nodes.len() {
            self.nodes[i].parent = if i > 0 {
                Some(self.nodes[i - 1].id)
            } else {
                None
            };
            self.nodes[i].children.clear();
            if i + 1 < self.nodes.len() {
                let next_id = self.nodes[i + 1].id;
                self.nodes[i].children.push(next_id);
            }
        }
        self.active_leaf = self.nodes.last().map(|n| n.id);
    }

    /// Get the active processor path from root to leaf.
    fn active_path(&self) -> Vec<usize> {
        // For linear chain, it's just all nodes in order
        (0..self.nodes.len()).collect()
    }

    /// Rebuild the offset translator from the active path's ratios.
    fn rebuild_translator(&mut self) {
        let ratios: Vec<(usize, usize)> = self
            .active_path()
            .iter()
            .map(|&idx| self.nodes[idx].processor.ratio())
            .collect();
        self.translator = OffsetTranslator::from_ratios(&ratios);
    }

    /// Get processed data for the given output range.
    pub fn get_range(&mut self, file: &MappedFile, output_offset: usize, len: usize) -> Vec<u8> {
        if self.nodes.is_empty() {
            return file.get_range(output_offset, len).to_vec();
        }

        // Check cache first
        if let Some(cached) = self.cache.get(output_offset, len) {
            return cached;
        }

        // Process through the chain
        let path = self.active_path();
        let result = self.process_chain(file, &path, output_offset, len);

        // Cache the result in aligned chunks
        let chunk_size = OutputCache::chunk_size();
        let start_chunk = output_offset / chunk_size;
        let end_chunk = (output_offset + len).saturating_sub(1) / chunk_size;

        for chunk_idx in start_chunk..=end_chunk {
            let chunk_start = chunk_idx * chunk_size;
            // Skip chunks that start before our output range — we don't have
            // complete data for them and would store bytes at wrong offsets.
            if chunk_start < output_offset && chunk_idx == start_chunk {
                continue;
            }
            let local_start = chunk_start - output_offset;
            let local_end = ((chunk_idx + 1) * chunk_size - output_offset).min(result.len());
            if local_start < local_end {
                let chunk_data = result[local_start..local_end].to_vec();
                self.cache.put(chunk_start, chunk_data);
            }
        }

        self.cache.evict(output_offset + len / 2);

        result
    }

    /// Process data through the chain of processors.
    fn process_chain(
        &mut self,
        file: &MappedFile,
        path: &[usize],
        output_offset: usize,
        len: usize,
    ) -> Vec<u8> {
        if path.is_empty() {
            return file.get_range(output_offset, len).to_vec();
        }

        // Compute how much input data we need by walking the ratio chain backwards
        let mut needed_output_offset = output_offset;
        let mut needed_output_len = len;
        let mut input_ranges: Vec<(usize, usize)> = Vec::new();

        for &node_idx in path.iter().rev() {
            let (ri, ro) = self.nodes[node_idx].processor.ratio();
            let input_offset = (needed_output_offset / ro) * ri;
            let input_len = (needed_output_len * ri).div_ceil(ro);
            input_ranges.push((input_offset, input_len));
            needed_output_offset = input_offset;
            needed_output_len = input_len;
        }
        input_ranges.reverse();

        // Get raw data from file
        let file_offset = input_ranges[0].0;
        let file_len = input_ranges[0].1;
        let mut data = file.get_range(file_offset, file_len).to_vec();

        // Process through each node
        for (i, &node_idx) in path.iter().enumerate() {
            let node = &mut self.nodes[node_idx];

            // For stateful processors, check for checkpoint
            if !node.processor.is_stateless() {
                let target_output_offset = if i + 1 < input_ranges.len() {
                    input_ranges[i + 1].0
                } else {
                    output_offset
                };

                if let Some((cp_offset, state)) =
                    node.checkpoints.nearest_before(target_output_offset)
                {
                    node.processor.restore_state(state);
                    // Skip already-processed data
                    let skip = cp_offset.saturating_sub(input_ranges[i].0);
                    if skip > 0 && skip < data.len() {
                        data = data[skip..].to_vec();
                    }
                } else {
                    // No checkpoint — process from the start of the file to
                    // build correct internal state before the requested window.
                    node.processor.reset();
                    let warmup_end = input_ranges[i].0;
                    if warmup_end > 0 {
                        let warmup_input = if i == 0 {
                            file.get_range(0, warmup_end).to_vec()
                        } else {
                            // For mid-chain nodes, we'd need the prior node's
                            // output from 0..warmup_end. For now, use raw file
                            // data — this is correct for single-node pipelines.
                            file.get_range(0, warmup_end).to_vec()
                        };
                        let mut discard = Vec::new();
                        node.processor.process(&warmup_input, &mut discard);
                        // Save a checkpoint so future seeks are fast
                        let state = node.processor.save_state();
                        node.checkpoints.force_save(warmup_end, state.as_ref());
                    }
                }
            }

            let mut output = Vec::with_capacity(data.len());
            node.processor.process(&data, &mut output);

            // Save checkpoints for stateful processors
            if !node.processor.is_stateless() {
                let interval = node.checkpoints.interval();
                if interval != usize::MAX {
                    let base_offset = if i + 1 < input_ranges.len() {
                        input_ranges[i + 1].0
                    } else {
                        output_offset
                    };
                    // Check if we crossed a checkpoint boundary
                    let state = node.processor.save_state();
                    node.checkpoints
                        .maybe_save(base_offset + output.len(), state.as_ref());
                }
            }

            data = output;
        }

        // Trim to requested range within the output
        let trim_start = 0; // Already aligned from input calculation
        let trim_end = len.min(data.len());
        if trim_start < data.len() {
            data[trim_start..trim_end].to_vec()
        } else {
            Vec::new()
        }
    }

    /// Compute the total output length for a given input file.
    pub fn output_len(&self, file_len: usize) -> usize {
        if self.nodes.is_empty() {
            file_len
        } else {
            self.translator.output_len(file_len)
        }
    }

    /// Convert output offset to file offset.
    #[allow(dead_code)]
    pub fn output_to_file_offset(&self, output_offset: usize) -> usize {
        if self.nodes.is_empty() {
            output_offset
        } else {
            self.translator.output_to_input(output_offset)
        }
    }

    /// Invalidate all caches and checkpoints.
    pub fn invalidate(&mut self) {
        self.cache.clear();
        for node in &mut self.nodes {
            node.invalidate();
        }
        self.rebuild_translator();
    }

    /// Number of processors in the chain.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the pipeline has no processors.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Remove all processors and reset the pipeline.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.next_id = 0;
        self.active_leaf = None;
        self.cache.clear();
        self.translator = OffsetTranslator::identity();
    }

    /// Iterator over nodes for UI display.
    pub fn nodes(&self) -> &[PipelineNode] {
        &self.nodes
    }

    /// Mutable access to a node's processor config UI.
    pub fn show_node_config(&mut self, idx: usize, ui: &mut eframe::egui::Ui) -> bool {
        if idx < self.nodes.len() {
            self.nodes[idx].processor.show_config(ui)
        } else {
            false
        }
    }

    /// Get a processor's name by index.
    pub fn node_name(&self, idx: usize) -> &str {
        if idx < self.nodes.len() {
            self.nodes[idx].processor.name()
        } else {
            "?"
        }
    }

    fn node_mut(&mut self, id: NodeId) -> Option<&mut PipelineNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }
}
