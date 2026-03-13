use eframe::egui;

use crate::pipeline::processor::{Processor, ProcessorState};

/// Shifts byte alignment by 0-7 bits.
///
/// Shifts all data left by `shift` bits, effectively realigning
/// non-byte-aligned protocol data.
pub struct BitShift {
    shift: u8,
}

impl BitShift {
    pub fn new(shift: u8) -> Self {
        Self { shift: shift % 8 }
    }
}

impl Default for BitShift {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Processor for BitShift {
    fn name(&self) -> &str {
        "Bit Shift"
    }

    fn process(&mut self, input: &[u8], output: &mut Vec<u8>) {
        if self.shift == 0 || input.is_empty() {
            output.extend_from_slice(input);
            return;
        }

        let shift = self.shift as u32;
        let anti_shift = 8 - shift;
        output.reserve(input.len());

        for i in 0..input.len() {
            let hi = input[i] << shift;
            let lo = if i + 1 < input.len() {
                input[i + 1] >> anti_shift
            } else {
                0
            };
            output.push(hi | lo);
        }
    }

    fn save_state(&self) -> Box<dyn ProcessorState> {
        Box::new(())
    }

    fn restore_state(&mut self, _state: &dyn ProcessorState) {}

    fn is_stateless(&self) -> bool {
        true
    }

    fn reset(&mut self) {}

    fn show_config(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("Shift:");
            let mut val = self.shift as f64;
            if ui
                .add(egui::DragValue::new(&mut val).range(0..=7).speed(0.1))
                .changed()
            {
                self.shift = val as u8;
                changed = true;
            }
            ui.label("bits");
        });
        changed
    }
}
