use eframe::egui;

use crate::file_handler::MappedFile;
use crate::stride_detect::{self, StrideCandidate, StrideDetectState};
use crate::viewer::DisplayMode;

pub struct StrideDialog {
    state: Option<StrideDetectState>,
    candidates: Vec<StrideCandidate>,
    show_popup: bool,
    min: usize,
    max: usize,
}

impl Default for StrideDialog {
    fn default() -> Self {
        Self {
            state: None,
            candidates: Vec::new(),
            show_popup: false,
            min: 2,
            max: 4096,
        }
    }
}

impl StrideDialog {
    /// Show the dialog. Returns `Some(stride)` if the user picked a candidate.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        file: &Option<MappedFile>,
        display_mode: DisplayMode,
    ) -> Option<usize> {
        if !self.show_popup {
            return None;
        }

        let mut chosen = None;
        let mut open = self.show_popup;

        egui::Window::new("Auto-Detect Stride")
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                // Range inputs
                ui.horizontal(|ui| {
                    ui.label("Min:");
                    let mut min_val = self.min as f64;
                    if ui
                        .add(
                            egui::DragValue::new(&mut min_val)
                                .range(2..=self.max)
                                .speed(1),
                        )
                        .changed()
                    {
                        self.min = (min_val as usize).max(2);
                    }
                    ui.label("Max:");
                    let mut max_val = self.max as f64;
                    if ui
                        .add(
                            egui::DragValue::new(&mut max_val)
                                .range(self.min..=65536)
                                .speed(1),
                        )
                        .changed()
                    {
                        self.max = (max_val as usize).max(self.min);
                    }
                });

                let detecting = self.state.is_some();
                ui.add_enabled_ui(!detecting, |ui| {
                    if ui.button("Detect").clicked() {
                        if let Some(file) = file {
                            self.candidates.clear();
                            let bit_mode = display_mode == DisplayMode::Bit;
                            self.state = Some(stride_detect::detect_stride_background(
                                file.clone(),
                                self.min,
                                self.max,
                                8,
                                bit_mode,
                            ));
                        }
                    }
                });

                ui.separator();

                if detecting {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Detecting...");
                    });
                } else if self.candidates.is_empty() {
                    ui.label("Click Detect to search for periodic patterns.");
                } else {
                    ui.label("Candidates (click to apply):");
                    let unit = if display_mode == DisplayMode::Bit {
                        "bits"
                    } else {
                        "bytes"
                    };
                    for c in &self.candidates {
                        let label = format!("{} {}  ({:.1}σ)", c.stride, unit, c.score);
                        if ui.selectable_label(false, label).clicked() {
                            chosen = Some(c.stride);
                        }
                    }
                }
            });

        self.show_popup = open;

        if chosen.is_some() {
            self.show_popup = false;
        }

        chosen
    }

    pub fn poll(&mut self) {
        if let Some(state) = &self.state {
            if let Some(candidates) = state.poll() {
                self.candidates = candidates;
                self.state = None;
            }
        }
    }

    pub fn is_running(&self) -> bool {
        self.state.is_some()
    }

    pub fn open(&mut self) {
        self.show_popup = true;
    }
}
