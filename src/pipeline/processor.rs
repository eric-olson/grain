use std::any::Any;

use eframe::egui;

/// Opaque state snapshot for checkpointing stateful processors.
pub trait ProcessorState: Any + Send + Sync {
    fn clone_box(&self) -> Box<dyn ProcessorState>;
    fn as_any(&self) -> &dyn Any;
}

impl ProcessorState for () {
    fn clone_box(&self) -> Box<dyn ProcessorState> {
        Box::new(())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A data transformation stage in the processing pipeline.
///
/// Processors consume input bytes and produce output bytes. Stateful processors
/// (like NRZ-M) must implement `save_state`/`restore_state` for checkpointing;
/// stateless processors return `()` and set `is_stateless() -> true`.
pub trait Processor: Send {
    fn name(&self) -> &str;

    /// Process input bytes, appending results to output.
    fn process(&mut self, input: &[u8], output: &mut Vec<u8>);

    /// Save current internal state for checkpointing.
    fn save_state(&self) -> Box<dyn ProcessorState>;

    /// Restore from a previously saved state.
    fn restore_state(&mut self, state: &dyn ProcessorState);

    /// Stateless processors skip checkpointing entirely.
    fn is_stateless(&self) -> bool {
        false
    }

    /// How often (in output bytes) to create checkpoints.
    fn checkpoint_interval(&self) -> usize {
        if self.is_stateless() {
            usize::MAX
        } else {
            65_536
        }
    }

    /// Input-to-output byte ratio as (input_bytes, output_bytes).
    fn ratio(&self) -> (usize, usize) {
        (1, 1)
    }

    /// Number of output branches (>1 for deinterleavers).
    #[allow(dead_code)]
    fn num_outputs(&self) -> usize {
        1
    }

    /// Reset processor to initial state.
    fn reset(&mut self);

    /// Show configuration UI. Returns true if config changed.
    fn show_config(&mut self, ui: &mut egui::Ui) -> bool;
}
