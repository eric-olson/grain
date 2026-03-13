use std::any::Any;

use eframe::egui;

use crate::pipeline::processor::{Processor, ProcessorState};

/// NRZ-M decoder state: the previous bit value.
#[derive(Clone)]
struct NrzmState {
    prev_bit: u8,
}

impl ProcessorState for NrzmState {
    fn clone_box(&self) -> Box<dyn ProcessorState> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// NRZ-M (Non-Return-to-Zero Mark) decoder.
///
/// In NRZ-M encoding, a transition (bit change) represents a 1,
/// and no transition represents a 0. This decoder recovers the
/// original data from an NRZ-M encoded stream.
pub struct NrzmDecode {
    prev_bit: u8,
}

impl NrzmDecode {
    pub fn new() -> Self {
        Self { prev_bit: 0 }
    }
}

impl Default for NrzmDecode {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor for NrzmDecode {
    fn name(&self) -> &str {
        "NRZ-M"
    }

    fn process(&mut self, input: &[u8], output: &mut Vec<u8>) {
        output.reserve(input.len());

        for &byte in input {
            let mut out_byte = 0u8;
            for bit_pos in (0..8).rev() {
                let current_bit = (byte >> bit_pos) & 1;
                let decoded = current_bit ^ self.prev_bit;
                out_byte |= decoded << bit_pos;
                self.prev_bit = current_bit;
            }
            output.push(out_byte);
        }
    }

    fn save_state(&self) -> Box<dyn ProcessorState> {
        Box::new(NrzmState {
            prev_bit: self.prev_bit,
        })
    }

    fn restore_state(&mut self, state: &dyn ProcessorState) {
        if let Some(s) = state.as_any().downcast_ref::<NrzmState>() {
            self.prev_bit = s.prev_bit;
        }
    }

    fn is_stateless(&self) -> bool {
        false
    }

    fn reset(&mut self) {
        self.prev_bit = 0;
    }

    fn show_config(&mut self, ui: &mut egui::Ui) -> bool {
        ui.label("NRZ-M decoder (no configuration)");
        false
    }
}
