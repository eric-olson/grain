use std::collections::HashSet;
use std::fmt;

use eframe::egui;

use crate::file_handler::MappedFile;
use crate::stride_detect::{self, StrideCandidate, StrideDetectState};
use crate::sync_search::{self, SearchMatch, SearchState};
use crate::viewer::{DisplayMode, PixelGridViewer};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectType {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F32,
    F64,
}

impl InspectType {
    pub const ALL: [InspectType; 10] = [
        InspectType::U8,
        InspectType::I8,
        InspectType::U16,
        InspectType::I16,
        InspectType::U32,
        InspectType::I32,
        InspectType::U64,
        InspectType::I64,
        InspectType::F32,
        InspectType::F64,
    ];

    pub fn byte_size(self) -> usize {
        match self {
            InspectType::U8 | InspectType::I8 => 1,
            InspectType::U16 | InspectType::I16 => 2,
            InspectType::U32 | InspectType::I32 | InspectType::F32 => 4,
            InspectType::U64 | InspectType::I64 | InspectType::F64 => 8,
        }
    }
}

impl fmt::Display for InspectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InspectType::U8 => write!(f, "u8"),
            InspectType::I8 => write!(f, "i8"),
            InspectType::U16 => write!(f, "u16"),
            InspectType::I16 => write!(f, "i16"),
            InspectType::U32 => write!(f, "u32"),
            InspectType::I32 => write!(f, "i32"),
            InspectType::U64 => write!(f, "u64"),
            InspectType::I64 => write!(f, "i64"),
            InspectType::F32 => write!(f, "f32"),
            InspectType::F64 => write!(f, "f64"),
        }
    }
}

/// Info about the pixel under the cursor.
pub struct CursorInfo {
    pub file_offset: usize,
    pub byte_value: u8,
    pub row: usize,
    pub col: usize,
    pub bit_index: Option<usize>, // bit within byte (0=MSB) in bit mode
}

/// A byte-range selection in the file.
#[derive(Clone, Copy)]
pub struct Selection {
    pub start: usize,
    pub end: usize, // inclusive
}

/// Search-related state grouped together.
#[derive(Default)]
struct SearchPanel {
    hex: String,
    state: Option<SearchState>,
    results: Vec<SearchMatch>,
    done: bool,
    error: Option<String>,
    pattern_len: usize,
}

/// Stride detection state grouped together.
struct StrideDetect {
    state: Option<StrideDetectState>,
    candidates: Vec<StrideCandidate>,
    show_popup: bool,
    min: usize,
    max: usize,
}

impl Default for StrideDetect {
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

pub struct App {
    file: Option<MappedFile>,
    stride: usize,
    scroll_offset: usize,
    zoom: f32,
    display_mode: DisplayMode,
    viewer: PixelGridViewer,
    cursor_info: Option<CursorInfo>,
    show_hex_panel: bool,
    show_inspector: bool,
    inspect_type: InspectType,
    /// Horizontal pixel offset to scroll to on next frame (from jump-to-match)
    h_scroll_target: Option<f32>,

    selection: Option<Selection>,
    /// Anchor byte offset for drag selection (set on mouse-down)
    drag_anchor: Option<usize>,

    search: SearchPanel,
    stride_detect: StrideDetect,
}

impl Default for App {
    fn default() -> Self {
        Self {
            file: None,
            stride: 256,
            scroll_offset: 0,
            zoom: 1.0,
            display_mode: DisplayMode::Byte,
            viewer: PixelGridViewer::default(),
            cursor_info: None,
            show_hex_panel: false,
            show_inspector: true,
            inspect_type: InspectType::U8,
            h_scroll_target: None,
            selection: None,
            drag_anchor: None,
            search: SearchPanel::default(),
            stride_detect: StrideDetect::default(),
        }
    }
}

impl App {
    fn open_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            match MappedFile::open(path) {
                Ok(mf) => {
                    self.file = Some(mf);
                    self.scroll_offset = 0;
                    self.selection = None;
                    self.drag_anchor = None;
                    self.viewer.invalidate();
                    self.search.state = None;
                    self.search.results.clear();
                    self.search.done = false;
                }
                Err(e) => {
                    eprintln!("Error opening file: {e}");
                }
            }
        }
    }

    fn poll_search(&mut self) {
        if let Some(state) = &self.search.state {
            if let Some(results) = state.poll() {
                self.search.results = results;
                self.search.done = true;
                self.search.state = None;
            }
        }
    }

    fn poll_stride_detect(&mut self) {
        if let Some(state) = &self.stride_detect.state {
            if let Some(candidates) = state.poll() {
                self.stride_detect.candidates = candidates;
                self.stride_detect.state = None;
            }
        }
    }

    fn start_search(&mut self) {
        self.search.error = None;
        self.search.done = false;
        self.search.results.clear();

        let pattern = match sync_search::parse_hex_pattern(&self.search.hex) {
            Ok(p) if !p.is_empty() => p,
            Ok(_) => {
                self.search.error = Some("Pattern is empty".to_string());
                return;
            }
            Err(e) => {
                self.search.error = Some(e.to_string());
                return;
            }
        };

        if let Some(file) = &self.file {
            self.search.pattern_len = pattern.len();
            self.search.state = Some(sync_search::search_background(file.clone(), pattern));
        }
    }

    fn max_offset(&self) -> usize {
        self.file.as_ref().map_or(0, |f| f.len().saturating_sub(1))
    }

    /// Convert a pointer position over the image rect to a file byte offset.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    fn pos_to_file_offset(
        pos: egui::Pos2,
        image_rect: egui::Rect,
        zoom: f32,
        stride: usize,
        scroll_offset: usize,
        data_len: usize,
        mode: DisplayMode,
    ) -> Option<usize> {
        let rel = pos - image_rect.min;
        let px_col = (rel.x / zoom) as usize;
        let px_row = (rel.y / zoom) as usize;
        if px_col >= stride {
            return None;
        }
        let pixel_index = px_row * stride + px_col;
        let ppb = mode.pixels_per_byte();
        let byte_index_in_view = pixel_index / ppb;
        if byte_index_in_view >= data_len {
            return None;
        }
        Some(scroll_offset + byte_index_in_view)
    }

    fn copy_button(ui: &mut egui::Ui, text: &str) {
        if ui.small_button("\u{1f4cb}").on_hover_text("Copy").clicked() {
            ui.ctx().copy_text(text.to_string());
        }
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::cast_precision_loss
    )]
    fn draw_inspector(
        ui: &mut egui::Ui,
        sel_start: usize,
        sel_end: usize,
        sel_len: usize,
        bytes: &[u8],
        inspect_type: InspectType,
    ) {
        if bytes.is_empty() {
            return;
        }

        egui::Grid::new("inspector_grid")
            .num_columns(3)
            .striped(true)
            .show(ui, |ui| {
                // Range
                let range_str = format!(
                    "0x{sel_start:08X}..0x{sel_end:08X} ({sel_len} byte{})",
                    if sel_len == 1 { "" } else { "s" }
                );
                ui.label("Range:");
                ui.monospace(&range_str);
                Self::copy_button(ui, &range_str);
                ui.end_row();

                // Hex dump (capped display)
                let hex_str: String = bytes
                    .iter()
                    .take(64)
                    .map(|b| format!("{b:02X}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                let hex_display = if sel_len > 64 {
                    format!("{hex_str} …({} more)", sel_len - 64)
                } else {
                    hex_str.clone()
                };
                let hex_full: String = bytes
                    .iter()
                    .map(|b| format!("{b:02X}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                ui.label("Hex:");
                ui.monospace(&hex_display);
                Self::copy_button(ui, &hex_full);
                ui.end_row();

                // Type-specific interpretation (single value or array)
                let needed = inspect_type.byte_size();
                if sel_len < needed {
                    ui.label(format!("{inspect_type}:"));
                    ui.monospace(format!("(select {needed} bytes)"));
                    ui.label("");
                    ui.end_row();
                } else {
                    let count = sel_len / needed;
                    let chunks: Vec<&[u8]> = bytes[..count * needed].chunks_exact(needed).collect();

                    // Helper: format one chunk as "BE / LE" for the given type
                    let format_chunk = |c: &[u8]| -> (String, String) {
                        match inspect_type {
                            InspectType::U8 => {
                                let v = format!("{}", c[0]);
                                (v.clone(), v)
                            }
                            InspectType::I8 => {
                                let v = format!("{}", c[0] as i8);
                                (v.clone(), v)
                            }
                            InspectType::U16 => {
                                let be = u16::from_be_bytes([c[0], c[1]]);
                                let le = u16::from_le_bytes([c[0], c[1]]);
                                (format!("{be}"), format!("{le}"))
                            }
                            InspectType::I16 => {
                                let be = i16::from_be_bytes([c[0], c[1]]);
                                let le = i16::from_le_bytes([c[0], c[1]]);
                                (format!("{be}"), format!("{le}"))
                            }
                            InspectType::U32 => {
                                let be = u32::from_be_bytes([c[0], c[1], c[2], c[3]]);
                                let le = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                                (format!("{be}"), format!("{le}"))
                            }
                            InspectType::I32 => {
                                let be = i32::from_be_bytes([c[0], c[1], c[2], c[3]]);
                                let le = i32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                                (format!("{be}"), format!("{le}"))
                            }
                            InspectType::U64 => {
                                let mut a = [0u8; 8]; a.copy_from_slice(c);
                                let be = u64::from_be_bytes(a);
                                let le = u64::from_le_bytes(a);
                                (format!("{be}"), format!("{le}"))
                            }
                            InspectType::I64 => {
                                let mut a = [0u8; 8]; a.copy_from_slice(c);
                                let be = i64::from_be_bytes(a);
                                let le = i64::from_le_bytes(a);
                                (format!("{be}"), format!("{le}"))
                            }
                            InspectType::F32 => {
                                let be = f32::from_be_bytes([c[0], c[1], c[2], c[3]]);
                                let le = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                                (format!("{be}"), format!("{le}"))
                            }
                            InspectType::F64 => {
                                let mut a = [0u8; 8]; a.copy_from_slice(c);
                                let be = f64::from_be_bytes(a);
                                let le = f64::from_le_bytes(a);
                                (format!("{be}"), format!("{le}"))
                            }
                        }
                    };

                    let is_single_byte = needed == 1;

                    if count == 1 {
                        // Single value
                        let (be_str, le_str) = format_chunk(chunks[0]);
                        if is_single_byte {
                            ui.label(format!("{inspect_type}:"));
                            ui.monospace(&be_str);
                            Self::copy_button(ui, &be_str);
                        } else {
                            ui.label(format!("{inspect_type} BE / LE:"));
                            ui.monospace(format!("{be_str} / {le_str}"));
                            Self::copy_button(ui, &format!("BE:{be_str} LE:{le_str}"));
                        }
                        ui.end_row();
                    } else {
                        // Array display
                        let cap = 64usize; // max elements to display

                        if is_single_byte {
                            let vals: Vec<String> = chunks.iter().take(cap).map(|c| format_chunk(c).0).collect();
                            let arr_str = format!("[{}]", vals.join(", "));
                            let suffix = if count > cap { format!(" …({} more)", count - cap) } else { String::new() };
                            ui.label(format!("[{inspect_type}; {count}]:"));
                            ui.monospace(format!("{arr_str}{suffix}"));
                            let full: Vec<String> = chunks.iter().map(|c| format_chunk(c).0).collect();
                            Self::copy_button(ui, &format!("[{}]", full.join(", ")));
                            ui.end_row();
                        } else {
                            let be_vals: Vec<String> = chunks.iter().take(cap).map(|c| format_chunk(c).0).collect();
                            let le_vals: Vec<String> = chunks.iter().take(cap).map(|c| format_chunk(c).1).collect();
                            let be_arr = format!("[{}]", be_vals.join(", "));
                            let le_arr = format!("[{}]", le_vals.join(", "));
                            let suffix = if count > cap { format!(" …({} more)", count - cap) } else { String::new() };

                            ui.label(format!("[{inspect_type}; {count}] BE:"));
                            ui.monospace(format!("{be_arr}{suffix}"));
                            let full_be: Vec<String> = chunks.iter().map(|c| format_chunk(c).0).collect();
                            Self::copy_button(ui, &format!("[{}]", full_be.join(", ")));
                            ui.end_row();

                            ui.label(format!("[{inspect_type}; {count}] LE:"));
                            ui.monospace(format!("{le_arr}{suffix}"));
                            let full_le: Vec<String> = chunks.iter().map(|c| format_chunk(c).1).collect();
                            Self::copy_button(ui, &format!("[{}]", full_le.join(", ")));
                            ui.end_row();
                        }
                    }
                }

                // ASCII
                let ascii: String = bytes
                    .iter()
                    .take(128)
                    .map(|&b| {
                        if b.is_ascii_graphic() || b == b' ' {
                            b as char
                        } else {
                            '.'
                        }
                    })
                    .collect();
                let ascii_display = if sel_len > 128 {
                    format!("{ascii}…")
                } else {
                    ascii.clone()
                };
                ui.label("ASCII:");
                ui.monospace(&ascii_display);
                Self::copy_button(ui, &ascii);
                ui.end_row();
            });
    }
}

impl eframe::App for App {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::too_many_lines
    )]
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_search();
        self.poll_stride_detect();

        // If a background task is running, keep repainting
        if self.search.state.is_some() || self.stride_detect.state.is_some() {
            ctx.request_repaint();
        }

        // Escape clears selection
        if self.selection.is_some() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.selection = None;
            self.viewer.invalidate();
        }

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open…").clicked() {
                        self.open_file();
                        ui.close();
                    }
                });

                ui.separator();

                ui.label("Stride:");
                let mut stride_val = self.stride as f64;
                let response = ui.add(
                    egui::DragValue::new(&mut stride_val)
                        .range(1..=4096)
                        .speed(1),
                );
                if response.changed() {
                    self.stride = (stride_val as usize).max(1);
                    self.viewer.invalidate();
                }
                if ui.button("Auto").clicked() && self.file.is_some() {
                    self.stride_detect.show_popup = true;
                }

                ui.separator();

                ui.label("Zoom:");
                let response = ui.add(
                    egui::DragValue::new(&mut self.zoom)
                        .range(1.0..=32.0)
                        .speed(0.1)
                        .suffix("x"),
                );
                if response.changed() {
                    self.zoom = self.zoom.clamp(1.0, 32.0);
                    self.viewer.invalidate();
                }

                ui.separator();

                ui.label("Mode:");
                let byte_selected = self.display_mode == DisplayMode::Byte;
                if ui.selectable_label(byte_selected, "Byte").clicked() && !byte_selected {
                    self.display_mode = DisplayMode::Byte;
                    self.viewer.invalidate();
                }
                if ui.selectable_label(!byte_selected, "Bit").clicked() && byte_selected {
                    self.display_mode = DisplayMode::Bit;
                    self.viewer.invalidate();
                }

                ui.separator();

                if ui.selectable_label(self.show_hex_panel, "Hex").clicked() {
                    self.show_hex_panel = !self.show_hex_panel;
                }
                if ui.selectable_label(self.show_inspector, "Inspector").clicked() {
                    self.show_inspector = !self.show_inspector;
                }

                ui.separator();

                ui.label("Type:");
                egui::ComboBox::from_id_salt("inspect_type")
                    .selected_text(self.inspect_type.to_string())
                    .width(50.0)
                    .show_ui(ui, |ui| {
                        for ty in InspectType::ALL {
                            ui.selectable_value(&mut self.inspect_type, ty, ty.to_string());
                        }
                    });

                ui.separator();

                if let Some(f) = &self.file {
                    ui.label(format!("{} — {} bytes", f.name(), f.len()));
                } else {
                    ui.label("No file open");
                }
            });
        });

        // Bottom status bar for cursor info
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(info) = &self.cursor_info {
                    ui.monospace(format!(
                        "Offset: 0x{:08X} ({})  Byte: 0x{:02X} ({})  Bin: {:08b}  Row: {}  Col: {}",
                        info.file_offset,
                        info.file_offset,
                        info.byte_value,
                        info.byte_value,
                        info.byte_value,
                        info.row,
                        info.col,
                    ));
                    if let Some(bit) = info.bit_index {
                        ui.monospace(format!(
                            "  Bit: {} ({})",
                            bit,
                            if (info.byte_value >> (7 - bit)) & 1 == 1 {
                                "1"
                            } else {
                                "0"
                            }
                        ));
                    }
                } else {
                    ui.monospace("Hover over the image to see byte info");
                }
            });
        });

        // Inspector panel (above status bar)
        let mut clear_selection = false;
        if self.show_inspector {
            if let (Some(sel), Some(data)) = (&self.selection, &self.file) {
                let sel_start = sel.start;
                let sel_end = sel.end;
                let sel_len = sel_end - sel_start + 1;
                let sel_bytes: Vec<u8> = data
                    .data()
                    .get(sel_start..=sel_end)
                    .map(|s| s.to_vec())
                    .unwrap_or_default();

                egui::TopBottomPanel::bottom("inspector_panel")
                    .max_height(200.0)
                    .resizable(true)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.heading("Data Inspector");
                            if ui.small_button("Clear").clicked() {
                                clear_selection = true;
                            }
                        });
                        let itype = self.inspect_type;
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            Self::draw_inspector(ui, sel_start, sel_end, sel_len, &sel_bytes, itype);
                        });
                    });
            }
        }
        if clear_selection {
            self.selection = None;
            self.viewer.invalidate();
        }

        // Search panel on the right
        egui::SidePanel::right("search_panel")
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.heading("Sync Word Search");

                ui.horizontal(|ui| {
                    ui.label("Hex:");
                    let response = ui.text_edit_singleline(&mut self.search.hex);
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.start_search();
                    }
                });

                if ui.button("Search").clicked() {
                    self.start_search();
                }

                if let Some(err) = &self.search.error {
                    ui.colored_label(egui::Color32::RED, err);
                }

                if self.search.state.is_some() {
                    ui.spinner();
                    ui.label("Searching…");
                } else if self.search.done {
                    ui.label(format!("{} matches found", self.search.results.len()));
                }

                ui.separator();

                let row_height = ui.text_style_height(&egui::TextStyle::Body);
                let num_results = self.search.results.len();
                let mut jump_to: Option<usize> = None;

                egui::ScrollArea::vertical().show_rows(ui, row_height, num_results, |ui, range| {
                    for i in range {
                        let m = &self.search.results[i];
                        let label = format!("0x{:08X}  {}", m.offset, m.variation);
                        if ui.selectable_label(false, &label).clicked() {
                            jump_to = Some(m.offset);
                        }
                    }
                });

                if let Some(offset) = jump_to {
                    let mode = self.display_mode;
                    let ppb = mode.pixels_per_byte();
                    let bpr = mode.bytes_for_pixels(self.stride);

                    // Align vertically: put the match row a few rows from the top
                    let match_pixel = offset * ppb;
                    let match_row = match_pixel / self.stride;
                    let target_row = match_row.saturating_sub(3);
                    self.scroll_offset = target_row * bpr;

                    // Snap horizontal scroll so the match column is visible
                    let match_col = match_pixel % self.stride;
                    self.h_scroll_target = Some(match_col as f32 * self.zoom);

                    self.viewer.invalidate();
                }
            });

        // Auto-stride results popup
        if self.stride_detect.show_popup {
            let mut open = self.stride_detect.show_popup;
            egui::Window::new("Auto-Detect Stride")
                .open(&mut open)
                .resizable(false)
                .show(ctx, |ui| {
                    // Range inputs
                    ui.horizontal(|ui| {
                        ui.label("Min:");
                        let mut min_val = self.stride_detect.min as f64;
                        if ui
                            .add(
                                egui::DragValue::new(&mut min_val)
                                    .range(2..=self.stride_detect.max)
                                    .speed(1),
                            )
                            .changed()
                        {
                            self.stride_detect.min = (min_val as usize).max(2);
                        }
                        ui.label("Max:");
                        let mut max_val = self.stride_detect.max as f64;
                        if ui
                            .add(
                                egui::DragValue::new(&mut max_val)
                                    .range(self.stride_detect.min..=65536)
                                    .speed(1),
                            )
                            .changed()
                        {
                            self.stride_detect.max = (max_val as usize).max(self.stride_detect.min);
                        }
                    });

                    let detecting = self.stride_detect.state.is_some();
                    ui.add_enabled_ui(!detecting, |ui| {
                        if ui.button("Detect").clicked() {
                            if let Some(file) = &self.file {
                                self.stride_detect.candidates.clear();
                                let bit_mode = self.display_mode == DisplayMode::Bit;
                                self.stride_detect.state =
                                    Some(stride_detect::detect_stride_background(
                                        file.clone(),
                                        self.stride_detect.min,
                                        self.stride_detect.max,
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
                    } else if self.stride_detect.candidates.is_empty() {
                        ui.label("Click Detect to search for periodic patterns.");
                    } else {
                        ui.label("Candidates (click to apply):");
                        let unit = if self.display_mode == DisplayMode::Bit {
                            "bits"
                        } else {
                            "bytes"
                        };
                        let mut chosen = None;
                        for c in &self.stride_detect.candidates {
                            let label = format!("{} {}  ({:.1}σ)", c.stride, unit, c.score);
                            if ui.selectable_label(false, label).clicked() {
                                chosen = Some(c.stride);
                            }
                        }
                        if let Some(s) = chosen {
                            self.stride = s;
                            self.viewer.invalidate();
                            self.stride_detect.show_popup = false;
                        }
                    }
                });
            self.stride_detect.show_popup = open;
        }

        // Hex dump panel on the left
        if self.show_hex_panel {
            egui::SidePanel::left("hex_panel")
                .default_width(520.0)
                .show(ctx, |ui| {
                    ui.heading("Hex Dump");
                    if let Some(file) = &self.file {
                        let bytes_per_line = 16usize;
                        let line_count = ui.available_height()
                            / ui.text_style_height(&egui::TextStyle::Monospace);
                        let num_lines = (line_count as usize).max(1);
                        let data = file.get_range(self.scroll_offset, num_lines * bytes_per_line);
                        let cursor_offset = self.cursor_info.as_ref().map(|c| c.file_offset);

                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for line in 0..num_lines {
                                let line_start = line * bytes_per_line;
                                if line_start >= data.len() {
                                    break;
                                }
                                let file_addr = self.scroll_offset + line_start;
                                let line_end = (line_start + bytes_per_line).min(data.len());
                                let line_data = &data[line_start..line_end];

                                let mut job = egui::text::LayoutJob::default();
                                let mono = egui::TextFormat {
                                    font_id: egui::FontId::monospace(12.0),
                                    color: ui.visuals().text_color(),
                                    ..Default::default()
                                };
                                let highlight_fmt = egui::TextFormat {
                                    font_id: egui::FontId::monospace(12.0),
                                    color: egui::Color32::BLACK,
                                    background: egui::Color32::from_rgb(255, 180, 0),
                                    ..Default::default()
                                };

                                // Address
                                job.append(&format!("{file_addr:08X}  "), 0.0, mono.clone());

                                // Hex bytes
                                for (i, &b) in line_data.iter().enumerate() {
                                    let byte_offset = file_addr + i;
                                    let fmt = if cursor_offset == Some(byte_offset) {
                                        highlight_fmt.clone()
                                    } else {
                                        mono.clone()
                                    };
                                    let sep = if i == 7 { "  " } else { " " };
                                    job.append(&format!("{b:02X}{sep}"), 0.0, fmt);
                                }
                                // Pad if short line
                                for i in line_data.len()..bytes_per_line {
                                    let sep = if i == 7 { "     " } else { "   " };
                                    job.append(sep, 0.0, mono.clone());
                                }

                                // ASCII
                                job.append(" |", 0.0, mono.clone());
                                for (i, &b) in line_data.iter().enumerate() {
                                    let ch = if b.is_ascii_graphic() || b == b' ' {
                                        b as char
                                    } else {
                                        '.'
                                    };
                                    let byte_offset = file_addr + i;
                                    let fmt = if cursor_offset == Some(byte_offset) {
                                        highlight_fmt.clone()
                                    } else {
                                        mono.clone()
                                    };
                                    job.append(&ch.to_string(), 0.0, fmt);
                                }
                                job.append("|", 0.0, mono.clone());

                                ui.label(job);
                            }
                        });
                    } else {
                        ui.label("No file open");
                    }
                });
        }

        // Main area: pixel grid
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.file.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.label("Open a binary file to begin (File > Open)");
                });
                return;
            }

            let file = self.file.as_ref().unwrap();
            let file_len = file.len();
            let viewport_height = ui.available_height();
            let zoom = self.zoom;
            let mode = self.display_mode;

            // How many data rows fit on screen at this zoom level
            let visible_rows = ((viewport_height / zoom).ceil() as usize).max(1);
            let bytes_per_row = mode.bytes_for_pixels(self.stride);
            let visible_bytes = visible_rows * bytes_per_row;
            let total_pixels = file_len * mode.pixels_per_byte();
            let total_rows = total_pixels.div_ceil(self.stride);

            // Handle scroll input (mouse wheel)
            let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll_delta != 0.0 {
                let scroll_rows = (-scroll_delta / (4.0 * zoom)) as isize;
                let byte_delta = scroll_rows * bytes_per_row as isize;
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
                    bytes_per_row as isize
                } else if i.key_pressed(egui::Key::ArrowUp) {
                    -(bytes_per_row as isize)
                } else if i.key_pressed(egui::Key::PageDown) {
                    (visible_rows * bytes_per_row) as isize
                } else if i.key_pressed(egui::Key::PageUp) {
                    -((visible_rows * bytes_per_row) as isize)
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

            // Ctrl+scroll to zoom
            let zoom_delta = ui.input(egui::InputState::zoom_delta);
            #[allow(clippy::float_cmp)] // egui returns exactly 1.0 when no zoom
            if zoom_delta != 1.0 {
                self.zoom = (self.zoom * zoom_delta).clamp(1.0, 32.0);
                self.viewer.invalidate();
            }

            // Offset indicator
            let current_pixel = self.scroll_offset * mode.pixels_per_byte();
            let current_row = current_pixel / self.stride.max(1);
            ui.horizontal(|ui| {
                ui.label(format!(
                    "Offset: 0x{:08X} ({}/{})  Row: {}/{}",
                    self.scroll_offset, self.scroll_offset, file_len, current_row, total_rows,
                ));
            });

            // Get data slice before entering closures to avoid borrow conflicts
            let data = file.get_range(self.scroll_offset, visible_bytes);
            let data_vec: Vec<u8> = data.to_vec();
            let stride = self.stride;
            let scroll_offset = self.scroll_offset;
            let max_off = self.max_offset();

            // Build highlight set: local pixel indices within the visible data
            let highlights = if !self.search.results.is_empty() && self.search.pattern_len > 0 {
                let view_start = self.scroll_offset;
                let view_end = view_start + data_vec.len();
                let pat_len = self.search.pattern_len;
                let ppb = mode.pixels_per_byte();
                let total_local_pixels = data_vec.len() * ppb;
                let mut set = HashSet::new();
                for m in &self.search.results {
                    let match_end = m.offset + pat_len;
                    if match_end > view_start && m.offset < view_end {
                        let local_byte_start = m.offset.saturating_sub(view_start);
                        let local_byte_end = (match_end - view_start).min(data_vec.len());
                        let px_start = local_byte_start * ppb;
                        let px_end = (local_byte_end * ppb).min(total_local_pixels);
                        for i in px_start..px_end {
                            set.insert(i);
                        }
                    }
                }
                set
            } else {
                HashSet::new()
            };

            // Build selection highlight set (local pixel indices)
            let selection_highlights = if let Some(sel) = &self.selection {
                let view_start = self.scroll_offset;
                let view_end = view_start + data_vec.len();
                let ppb = mode.pixels_per_byte();
                let total_local_pixels = data_vec.len() * ppb;
                let sel_end_excl = sel.end + 1;
                if sel_end_excl > view_start && sel.start < view_end {
                    let local_byte_start = sel.start.saturating_sub(view_start);
                    let local_byte_end = (sel_end_excl - view_start).min(data_vec.len());
                    let px_start = local_byte_start * ppb;
                    let px_end = (local_byte_end * ppb).min(total_local_pixels);
                    (px_start..px_end).collect::<HashSet<usize>>()
                } else {
                    HashSet::new()
                }
            } else {
                HashSet::new()
            };

            // Layout: scrollbar pinned to right edge, pixel grid fills the rest
            let mut new_scroll: Option<usize> = None;
            let available = ui.available_rect_before_wrap();
            let scrollbar_width = 24.0;

            // Scrollbar on the right
            if total_rows > 0 {
                let mut row_f = current_row as f64;
                let max_row = total_rows.saturating_sub(visible_rows) as f64;
                let bar_rect = egui::Rect::from_min_size(
                    egui::pos2(available.right() - scrollbar_width, available.top()),
                    egui::vec2(scrollbar_width, available.height()),
                );
                let mut bar_ui = ui.new_child(egui::UiBuilder::new().max_rect(bar_rect));
                let response = bar_ui.add(
                    egui::Slider::new(&mut row_f, max_row..=0.0)
                        .vertical()
                        .show_value(false),
                );
                if response.changed() {
                    let new_row = row_f as usize;
                    new_scroll = Some((new_row * bytes_per_row).min(max_off));
                }
            }

            // Pixel grid in the remaining space, horizontally scrollable
            let grid_rect = egui::Rect::from_min_max(
                available.min,
                egui::pos2(
                    available.right() - scrollbar_width - 4.0,
                    available.bottom(),
                ),
            );
            let mut grid_ui = ui.new_child(egui::UiBuilder::new().max_rect(grid_rect));
            let h_target = self.h_scroll_target.take();
            let mut scroll_area = egui::ScrollArea::horizontal();
            if let Some(target_x) = h_target {
                scroll_area = scroll_area.horizontal_scroll_offset(target_x.max(0.0));
            }
            let mut image_rect = egui::Rect::NOTHING;
            let mut grid_response: Option<egui::Response> = None;
            scroll_area.show(&mut grid_ui, |ui| {
                let (_rows, rect, resp) = self.viewer.show(
                    ui,
                    &data_vec,
                    stride,
                    scroll_offset,
                    viewport_height,
                    zoom,
                    &highlights,
                    &selection_highlights,
                    mode,
                );
                image_rect = rect;
                grid_response = Some(resp);
            });

            // Handle mouse interaction for selection
            if let Some(resp) = &grid_response {
                if image_rect.is_positive() {
                    // Drag started — set anchor using press origin (where mouse-down occurred)
                    if resp.drag_started() {
                        if let Some(origin) = ctx.input(|i| i.pointer.press_origin()) {
                            if let Some(off) = Self::pos_to_file_offset(
                                origin, image_rect, zoom, stride, scroll_offset,
                                data_vec.len(), mode,
                            ) {
                                self.drag_anchor = Some(off);
                                let type_size = self.inspect_type.byte_size();
                                let end = (off + type_size - 1).min(scroll_offset + data_vec.len() - 1).min(self.max_offset());
                                self.selection = Some(Selection { start: off, end });
                                self.viewer.invalidate();
                            }
                        }
                    }
                    // Dragging — snap selection to type-size intervals
                    if resp.dragged() {
                        if let Some(anchor) = self.drag_anchor {
                            if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                                if let Some(off) = Self::pos_to_file_offset(
                                    pos, image_rect, zoom, stride, scroll_offset,
                                    data_vec.len(), mode,
                                ) {
                                    let type_size = self.inspect_type.byte_size();
                                    let start = anchor.min(off);
                                    let end_raw = anchor.max(off);
                                    // Extend end so the selection covers a whole number of elements
                                    let span = end_raw - start + 1;
                                    let count = span.div_ceil(type_size);
                                    let end = (start + count * type_size - 1)
                                        .min(scroll_offset + data_vec.len() - 1)
                                        .min(self.max_offset());
                                    self.selection = Some(Selection { start, end });
                                    self.viewer.invalidate();
                                }
                            }
                        }
                    }
                    // Click without drag — select inspect_type.byte_size() bytes
                    if resp.clicked() {
                        if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                            if let Some(off) = Self::pos_to_file_offset(
                                pos, image_rect, zoom, stride, scroll_offset,
                                data_vec.len(), mode,
                            ) {
                                let type_size = self.inspect_type.byte_size();
                                let end = (off + type_size - 1).min(scroll_offset + data_vec.len() - 1).min(self.max_offset());
                                self.selection = Some(Selection { start: off, end });
                                self.drag_anchor = None;
                                self.viewer.invalidate();
                            }
                        }
                    }
                    // Drag released
                    if resp.drag_stopped() {
                        self.drag_anchor = None;
                    }
                }
            }

            // Cursor info from mouse position over the image
            self.cursor_info = None;
            if image_rect.is_positive() {
                if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
                    if image_rect.contains(pos) {
                        let rel = pos - image_rect.min;
                        let px_col = (rel.x / zoom) as usize;
                        let px_row = (rel.y / zoom) as usize;
                        if px_col < stride {
                            let pixel_index = px_row * stride + px_col;
                            let ppb = mode.pixels_per_byte();
                            let byte_index_in_view = pixel_index / ppb;
                            if byte_index_in_view < data_vec.len() {
                                let file_offset = scroll_offset + byte_index_in_view;
                                let byte_value = data_vec[byte_index_in_view];
                                let bit_index = match mode {
                                    DisplayMode::Bit => Some(pixel_index % 8),
                                    DisplayMode::Byte => None,
                                };
                                self.cursor_info = Some(CursorInfo {
                                    file_offset,
                                    byte_value,
                                    row: px_row,
                                    col: px_col,
                                    bit_index,
                                });
                            }
                        }
                    }
                }
            }

            if let Some(off) = new_scroll {
                if off != self.scroll_offset {
                    self.scroll_offset = off;
                    self.viewer.invalidate();
                }
            }
        });
    }
}
