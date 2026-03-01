use std::sync::Arc;

use eframe::egui;

use crate::file_handler::MappedFile;
use crate::sync_search::{self, SearchMatch, SearchState};
use crate::viewer::PixelGridViewer;

pub struct App {
    file: Option<MappedFile>,
    stride: usize,
    scroll_offset: usize,
    viewer: PixelGridViewer,

    // Sync search
    search_hex: String,
    search_state: Option<SearchState>,
    search_results: Vec<SearchMatch>,
    search_done: bool,
    search_error: Option<String>,
    // Cached copy for background search
    file_data_cache: Option<Arc<Vec<u8>>>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            file: None,
            stride: 256,
            scroll_offset: 0,
            viewer: PixelGridViewer::default(),
            search_hex: String::new(),
            search_state: None,
            search_results: Vec::new(),
            search_done: false,
            search_error: None,
            file_data_cache: None,
        }
    }
}

impl App {
    fn open_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            match MappedFile::open(path) {
                Ok(mf) => {
                    self.file_data_cache = Some(Arc::new(mf.data().to_vec()));
                    self.file = Some(mf);
                    self.scroll_offset = 0;
                    self.viewer.invalidate();
                    self.search_state = None;
                    self.search_results.clear();
                    self.search_done = false;
                }
                Err(e) => {
                    eprintln!("Error opening file: {e}");
                }
            }
        }
    }

    fn poll_search(&mut self) {
        if let Some(state) = &self.search_state {
            let done = *state.done.lock().unwrap();
            if done {
                self.search_results = state.results.lock().unwrap().clone();
                self.search_done = true;
                self.search_state = None;
            } else {
                // Update partial results
                self.search_results = state.results.lock().unwrap().clone();
            }
        }
    }

    fn start_search(&mut self) {
        self.search_error = None;
        self.search_done = false;
        self.search_results.clear();

        let pattern = match sync_search::parse_hex_pattern(&self.search_hex) {
            Ok(p) if !p.is_empty() => p,
            Ok(_) => {
                self.search_error = Some("Pattern is empty".to_string());
                return;
            }
            Err(e) => {
                self.search_error = Some(e);
                return;
            }
        };

        if let Some(data) = &self.file_data_cache {
            self.search_state = Some(sync_search::search_background(data.clone(), pattern));
        }
    }

    fn max_offset(&self) -> usize {
        self.file
            .as_ref()
            .map(|f| f.len().saturating_sub(1))
            .unwrap_or(0)
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_search();

        // If a search is running, keep repainting
        if self.search_state.is_some() {
            ctx.request_repaint();
        }

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open…").clicked() {
                        self.open_file();
                        ui.close_menu();
                    }
                });

                ui.separator();

                ui.label("Stride:");
                let mut stride_val = self.stride as f64;
                let response =
                    ui.add(egui::DragValue::new(&mut stride_val).range(1..=4096).speed(1));
                if response.changed() {
                    self.stride = (stride_val as usize).max(1);
                    self.viewer.invalidate();
                }

                ui.separator();

                if let Some(f) = &self.file {
                    ui.label(format!("{} — {} bytes", f.name(), f.len()));
                } else {
                    ui.label("No file open");
                }
            });
        });

        // Search panel on the right
        egui::SidePanel::right("search_panel")
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.heading("Sync Word Search");

                ui.horizontal(|ui| {
                    ui.label("Hex:");
                    let response = ui.text_edit_singleline(&mut self.search_hex);
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.start_search();
                    }
                });

                if ui.button("Search").clicked() {
                    self.start_search();
                }

                if let Some(err) = &self.search_error {
                    ui.colored_label(egui::Color32::RED, err);
                }

                if self.search_state.is_some() {
                    ui.spinner();
                    ui.label(format!("Searching… {} matches so far", self.search_results.len()));
                } else if self.search_done {
                    ui.label(format!("{} matches found", self.search_results.len()));
                }

                ui.separator();

                let row_height = ui.text_style_height(&egui::TextStyle::Body);
                let results = self.search_results.clone();
                let mut jump_to: Option<usize> = None;

                egui::ScrollArea::vertical().show_rows(
                    ui,
                    row_height,
                    results.len(),
                    |ui, range| {
                        for i in range {
                            let m = &results[i];
                            let label =
                                format!("0x{:08X}  {}", m.offset, m.variation);
                            if ui
                                .selectable_label(false, &label)
                                .clicked()
                            {
                                jump_to = Some(m.offset);
                            }
                        }
                    },
                );

                if let Some(offset) = jump_to {
                    // Align to stride boundary
                    self.scroll_offset = (offset / self.stride) * self.stride;
                    self.viewer.invalidate();
                }
            });

        // Main area: pixel grid
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.file.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.label("Open a binary file to begin (File > Open)");
                });
                return;
            }

            let file = self.file.as_ref().unwrap();
            let viewport_height = ui.available_height();
            let visible_rows = (viewport_height as usize).max(64);
            let visible_bytes = visible_rows * self.stride;
            let data = file.get_range(self.scroll_offset, visible_bytes);

            // Handle scroll input
            let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll_delta != 0.0 {
                let scroll_rows = (-scroll_delta / 4.0) as isize;
                let byte_delta = scroll_rows * self.stride as isize;
                let new_offset = (self.scroll_offset as isize + byte_delta)
                    .max(0)
                    .min(self.max_offset() as isize) as usize;
                if new_offset != self.scroll_offset {
                    self.scroll_offset = new_offset;
                    self.viewer.invalidate();
                }
            }

            // Handle keyboard
            let key_scroll = ui.input(|i| {
                if i.key_pressed(egui::Key::ArrowDown) {
                    self.stride as isize
                } else if i.key_pressed(egui::Key::ArrowUp) {
                    -(self.stride as isize)
                } else if i.key_pressed(egui::Key::PageDown) {
                    (visible_rows * self.stride) as isize
                } else if i.key_pressed(egui::Key::PageUp) {
                    -((visible_rows * self.stride) as isize)
                } else if i.key_pressed(egui::Key::Home) {
                    -(self.scroll_offset as isize)
                } else if i.key_pressed(egui::Key::End) {
                    self.max_offset() as isize - self.scroll_offset as isize
                } else {
                    0
                }
            });
            if key_scroll != 0 {
                let new_offset = (self.scroll_offset as isize + key_scroll)
                    .max(0)
                    .min(self.max_offset() as isize) as usize;
                if new_offset != self.scroll_offset {
                    self.scroll_offset = new_offset;
                    self.viewer.invalidate();
                }
            }

            // Offset indicator
            ui.horizontal(|ui| {
                ui.label(format!(
                    "Offset: 0x{:08X} ({}/{})",
                    self.scroll_offset,
                    self.scroll_offset,
                    file.len()
                ));
            });

            // Render the grid
            self.viewer
                .show(ui, data, self.stride, self.scroll_offset, viewport_height);
        });
    }
}
