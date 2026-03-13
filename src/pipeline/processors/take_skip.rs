use eframe::egui;

use crate::pipeline::processor::{Processor, ProcessorState};

/// Repeating take/skip pattern processor.
///
/// Takes `take` bytes/bits, skips `skip` bytes/bits, repeating through the input.
/// In bit mode, operates on individual bits rather than bytes.
pub struct TakeSkip {
    take: usize,
    skip: usize,
    bit_mode: bool,
}

impl TakeSkip {
    pub fn new(take: usize, skip: usize) -> Self {
        Self {
            take: take.max(1),
            skip,
            bit_mode: false,
        }
    }
}

impl Default for TakeSkip {
    fn default() -> Self {
        Self::new(1, 1)
    }
}

impl Processor for TakeSkip {
    fn name(&self) -> &str {
        "Take/Skip"
    }

    fn process(&mut self, input: &[u8], output: &mut Vec<u8>) {
        if self.bit_mode {
            process_bits(input, output, self.take, self.skip);
        } else {
            process_bytes(input, output, self.take, self.skip);
        }
    }

    fn save_state(&self) -> Box<dyn ProcessorState> {
        Box::new(())
    }

    fn restore_state(&mut self, _state: &dyn ProcessorState) {}

    fn is_stateless(&self) -> bool {
        true
    }

    fn ratio(&self) -> (usize, usize) {
        (self.take + self.skip, self.take)
    }

    fn reset(&mut self) {}

    fn show_config(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;

        ui.horizontal(|ui| {
            ui.label("Take:");
            let mut take = self.take as f64;
            if ui
                .add(egui::DragValue::new(&mut take).range(1..=4096).speed(1))
                .changed()
            {
                self.take = (take as usize).max(1);
                changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Skip:");
            let mut skip = self.skip as f64;
            if ui
                .add(egui::DragValue::new(&mut skip).range(0..=4096).speed(1))
                .changed()
            {
                self.skip = skip as usize;
                changed = true;
            }
        });

        if ui.checkbox(&mut self.bit_mode, "Bit mode").changed() {
            changed = true;
        }

        changed
    }
}

fn process_bytes(input: &[u8], output: &mut Vec<u8>, take: usize, skip: usize) {
    let period = take + skip;
    if period == 0 {
        return;
    }
    output.reserve(input.len() * take / period);
    let mut i = 0;
    while i < input.len() {
        let end = (i + take).min(input.len());
        output.extend_from_slice(&input[i..end]);
        i += period;
    }
}

fn process_bits(input: &[u8], output: &mut Vec<u8>, take: usize, skip: usize) {
    let period = take + skip;
    if period == 0 {
        return;
    }
    let total_bits = input.len() * 8;
    let out_bits = total_bits * take / period;
    output.reserve(out_bits.div_ceil(8));

    let mut out_bit_idx = 0usize;
    let mut in_bit = 0usize;

    while in_bit < total_bits {
        // Take phase
        for _ in 0..take {
            if in_bit >= total_bits {
                break;
            }
            let byte_idx = in_bit / 8;
            let bit_pos = 7 - (in_bit % 8); // MSB first
            let bit_val = (input[byte_idx] >> bit_pos) & 1;

            let out_byte_idx = out_bit_idx / 8;
            let out_bit_pos = 7 - (out_bit_idx % 8);

            if out_byte_idx >= output.len() {
                output.push(0);
            }
            output[out_byte_idx] |= bit_val << out_bit_pos;

            in_bit += 1;
            out_bit_idx += 1;
        }
        // Skip phase
        in_bit += skip;
    }
}
