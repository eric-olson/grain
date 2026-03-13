use std::collections::BTreeMap;

use super::processor::ProcessorState;

/// A saved processor state at a specific output offset.
struct Checkpoint {
    state: Box<dyn ProcessorState>,
}

/// Stores processor state snapshots keyed by output offset.
///
/// Used to restore stateful processors when seeking to arbitrary positions
/// without reprocessing from the beginning.
pub struct CheckpointStore {
    checkpoints: BTreeMap<usize, Checkpoint>,
    interval: usize,
}

impl CheckpointStore {
    pub fn new(interval: usize) -> Self {
        Self {
            checkpoints: BTreeMap::new(),
            interval,
        }
    }

    /// Returns the checkpoint interval.
    pub fn interval(&self) -> usize {
        self.interval
    }

    /// Save state at the given output offset if it aligns with the interval.
    pub fn maybe_save(&mut self, output_offset: usize, state: &dyn ProcessorState) {
        if self.interval == usize::MAX {
            return;
        }
        if output_offset.is_multiple_of(self.interval) {
            self.checkpoints.insert(
                output_offset,
                Checkpoint {
                    state: state.clone_box(),
                },
            );
        }
    }

    /// Find the nearest checkpoint at or before the given output offset.
    /// Returns (checkpoint_offset, state) or None if no checkpoint exists.
    pub fn nearest_before(&self, output_offset: usize) -> Option<(usize, &dyn ProcessorState)> {
        self.checkpoints
            .range(..=output_offset)
            .next_back()
            .map(|(&off, cp)| (off, cp.state.as_ref() as &dyn ProcessorState))
    }

    /// Unconditionally save a checkpoint at the given offset.
    pub fn force_save(&mut self, output_offset: usize, state: &dyn ProcessorState) {
        self.checkpoints.insert(
            output_offset,
            Checkpoint {
                state: state.clone_box(),
            },
        );
    }

    /// Clear all checkpoints (e.g., on config change).
    pub fn clear(&mut self) {
        self.checkpoints.clear();
    }
}
