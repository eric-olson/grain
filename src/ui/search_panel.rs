use eframe::egui;

use crate::file_handler::MappedFile;
use crate::sync_search::{self, SearchMatch, SearchState};
use crate::viewer::DisplayMode;

pub struct SearchAction {
    pub jump_to_offset: usize,
}

#[derive(Default)]
pub struct SearchPanel {
    hex: String,
    state: Option<SearchState>,
    results: Vec<SearchMatch>,
    done: bool,
    error: Option<String>,
    pattern_len: usize,
}

impl SearchPanel {
    pub fn show(&mut self, ctx: &egui::Context, file: &Option<MappedFile>) -> Option<SearchAction> {
        let mut action = None;

        egui::SidePanel::right("search_panel")
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.heading("Sync Word Search");

                ui.horizontal(|ui| {
                    ui.label("Hex:");
                    let response = ui.text_edit_singleline(&mut self.hex);
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.start_search(file);
                    }
                });

                if ui.button("Search").clicked() {
                    self.start_search(file);
                }

                if let Some(err) = &self.error {
                    ui.colored_label(egui::Color32::RED, err);
                }

                if self.state.is_some() {
                    ui.spinner();
                    ui.label("Searching…");
                } else if self.done {
                    ui.label(format!("{} matches found", self.results.len()));
                }

                ui.separator();

                let row_height = ui.text_style_height(&egui::TextStyle::Body);
                let num_results = self.results.len();

                egui::ScrollArea::vertical().show_rows(ui, row_height, num_results, |ui, range| {
                    for i in range {
                        let m = &self.results[i];
                        let label = format!("0x{:08X}  {}", m.offset, m.variation);
                        if ui.selectable_label(false, &label).clicked() {
                            action = Some(SearchAction {
                                jump_to_offset: m.offset,
                            });
                        }
                    }
                });
            });

        action
    }

    /// Compute the scroll offset and h_scroll target for jumping to a match.
    #[allow(clippy::cast_precision_loss)]
    pub fn compute_jump(
        offset: usize,
        stride: usize,
        zoom: f32,
        display_mode: DisplayMode,
    ) -> (usize, f32) {
        let ppb = display_mode.pixels_per_byte();
        let bpr = display_mode.bytes_for_pixels(stride);
        let match_pixel = offset * ppb;
        let match_row = match_pixel / stride;
        let target_row = match_row.saturating_sub(3);
        let scroll_offset = target_row * bpr;
        let match_col = match_pixel % stride;
        let h_scroll = match_col as f32 * zoom;
        (scroll_offset, h_scroll)
    }

    pub fn poll(&mut self) {
        if let Some(state) = &self.state {
            if let Some(results) = state.poll() {
                self.results = results;
                self.done = true;
                self.state = None;
            }
        }
    }

    fn start_search(&mut self, file: &Option<MappedFile>) {
        self.error = None;
        self.done = false;
        self.results.clear();

        let pattern = match sync_search::parse_hex_pattern(&self.hex) {
            Ok(p) if !p.is_empty() => p,
            Ok(_) => {
                self.error = Some("Pattern is empty".to_string());
                return;
            }
            Err(e) => {
                self.error = Some(e.to_string());
                return;
            }
        };

        if let Some(file) = file {
            self.pattern_len = pattern.len();
            self.state = Some(sync_search::search_background(file.clone(), pattern));
        }
    }

    pub fn results(&self) -> &[SearchMatch] {
        &self.results
    }

    pub fn pattern_len(&self) -> usize {
        self.pattern_len
    }

    pub fn is_running(&self) -> bool {
        self.state.is_some()
    }

    pub fn reset(&mut self) {
        self.state = None;
        self.results.clear();
        self.done = false;
    }
}
