use std::collections::HashSet;

use eframe::egui;

use crate::file_handler::MappedFile;
use crate::pipeline::Pipeline;
use crate::sync_search::SearchMatch;
use crate::types::{CursorInfo, InspectType, Selection};
use crate::viewer::{DisplayMode, PixelGridViewer};

pub struct Viewport {
    viewer: PixelGridViewer,
    drag_anchor: Option<usize>,
    h_scroll_target: Option<f32>,
}

pub struct ViewportResponse {
    pub cursor_info: Option<CursorInfo>,
    pub scroll_offset: Option<usize>,
    pub selection: Option<Option<Selection>>, // Some(None)=clear, Some(Some(s))=set
    pub zoom: Option<f32>,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            viewer: PixelGridViewer::default(),
            drag_anchor: None,
            h_scroll_target: None,
        }
    }
}

impl Viewport {
    #[allow(
        clippy::too_many_arguments,
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        file: &MappedFile,
        mut pipeline: Option<&mut Pipeline>,
        stride: usize,
        scroll_offset: usize,
        zoom: f32,
        display_mode: DisplayMode,
        inspect_type: InspectType,
        search_results: &[SearchMatch],
        pattern_len: usize,
        selection: &Option<Selection>,
        max_offset: usize,
    ) -> ViewportResponse {
        let mut resp = ViewportResponse {
            cursor_info: None,
            scroll_offset: None,
            selection: None,
            zoom: None,
        };

        let file_len = if let Some(ref pipeline) = pipeline {
            pipeline.output_len(file.len())
        } else {
            file.len()
        };
        let viewport_height = ui.available_height();
        let mode = display_mode;

        // How many data rows fit on screen at this zoom level
        let visible_rows = ((viewport_height / zoom).ceil() as usize).max(1);
        let bytes_per_row = mode.bytes_for_pixels(stride);
        let visible_bytes = visible_rows * bytes_per_row;
        let total_pixels = file_len * mode.pixels_per_byte();
        let total_rows = total_pixels.div_ceil(stride);

        // Handle scroll input (mouse wheel)
        let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll_delta != 0.0 {
            let scroll_rows = (-scroll_delta / (4.0 * zoom)) as isize;
            let byte_delta = scroll_rows * bytes_per_row as isize;
            let new_offset = (scroll_offset as isize + byte_delta)
                .max(0)
                .min(max_offset as isize) as usize;
            if new_offset != scroll_offset {
                resp.scroll_offset = Some(new_offset);
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
                -(scroll_offset as isize)
            } else if i.key_pressed(egui::Key::End) {
                max_offset as isize - scroll_offset as isize
            } else {
                0
            }
        });
        if key_scroll != 0 {
            let new_offset = (scroll_offset as isize + key_scroll)
                .max(0)
                .min(max_offset as isize) as usize;
            if new_offset != scroll_offset {
                resp.scroll_offset = Some(new_offset);
            }
        }

        // Ctrl+scroll to zoom
        let zoom_delta = ui.input(egui::InputState::zoom_delta);
        #[allow(clippy::float_cmp)] // egui returns exactly 1.0 when no zoom
        if zoom_delta != 1.0 {
            resp.zoom = Some((zoom * zoom_delta).clamp(1.0, 32.0));
        }

        // Use the effective scroll offset (updated or original)
        let eff_scroll = resp.scroll_offset.unwrap_or(scroll_offset);

        // Offset indicator
        let current_pixel = eff_scroll * mode.pixels_per_byte();
        let current_row = current_pixel / stride.max(1);
        ui.horizontal(|ui| {
            if pipeline.is_some() {
                ui.label(format!(
                    "Output: 0x{:08X} ({}/{})  Row: {}/{}",
                    eff_scroll, eff_scroll, file_len, current_row, total_rows,
                ));
            } else {
                ui.label(format!(
                    "Offset: 0x{:08X} ({}/{})  Row: {}/{}",
                    eff_scroll, eff_scroll, file_len, current_row, total_rows,
                ));
            }
        });

        // Get data slice
        let data_vec: Vec<u8> = if let Some(ref mut pipeline) = pipeline {
            pipeline.get_range(file, eff_scroll, visible_bytes)
        } else {
            file.get_range(eff_scroll, visible_bytes).to_vec()
        };

        // Build highlight set: local pixel indices within the visible data
        let highlights = if !search_results.is_empty() && pattern_len > 0 {
            let view_start = eff_scroll;
            let view_end = view_start + data_vec.len();
            let ppb = mode.pixels_per_byte();
            let total_local_pixels = data_vec.len() * ppb;
            let mut set = HashSet::new();
            for m in search_results {
                let match_end = m.offset + pattern_len;
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
        let selection_highlights = if let Some(sel) = selection {
            let view_start = eff_scroll;
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
                new_scroll = Some((new_row * bytes_per_row).min(max_offset));
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
            let (_rows, rect, r) = self.viewer.show(
                ui,
                &data_vec,
                stride,
                eff_scroll,
                viewport_height,
                zoom,
                &highlights,
                &selection_highlights,
                mode,
            );
            image_rect = rect;
            grid_response = Some(r);
        });

        // Handle mouse interaction for selection
        if let Some(grid_resp) = &grid_response {
            if image_rect.is_positive() {
                // Drag started — set anchor using press origin
                if grid_resp.drag_started() {
                    if let Some(origin) = ctx.input(|i| i.pointer.press_origin()) {
                        if let Some(off) = pos_to_file_offset(
                            origin,
                            image_rect,
                            zoom,
                            stride,
                            eff_scroll,
                            data_vec.len(),
                            mode,
                        ) {
                            self.drag_anchor = Some(off);
                            let type_size = inspect_type.byte_size();
                            let end = (off + type_size - 1)
                                .min(eff_scroll + data_vec.len() - 1)
                                .min(max_offset);
                            resp.selection = Some(Some(Selection { start: off, end }));
                        }
                    }
                }
                // Dragging — snap selection to type-size intervals
                if grid_resp.dragged() {
                    if let Some(anchor) = self.drag_anchor {
                        if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                            if let Some(off) = pos_to_file_offset(
                                pos,
                                image_rect,
                                zoom,
                                stride,
                                eff_scroll,
                                data_vec.len(),
                                mode,
                            ) {
                                let type_size = inspect_type.byte_size();
                                let start = anchor.min(off);
                                let end_raw = anchor.max(off);
                                let span = end_raw - start + 1;
                                let count = span.div_ceil(type_size);
                                let end = (start + count * type_size - 1)
                                    .min(eff_scroll + data_vec.len() - 1)
                                    .min(max_offset);
                                resp.selection = Some(Some(Selection { start, end }));
                            }
                        }
                    }
                }
                // Click without drag — select inspect_type.byte_size() bytes
                if grid_resp.clicked() {
                    if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                        if let Some(off) = pos_to_file_offset(
                            pos,
                            image_rect,
                            zoom,
                            stride,
                            eff_scroll,
                            data_vec.len(),
                            mode,
                        ) {
                            let type_size = inspect_type.byte_size();
                            let end = (off + type_size - 1)
                                .min(eff_scroll + data_vec.len() - 1)
                                .min(max_offset);
                            resp.selection = Some(Some(Selection { start: off, end }));
                            self.drag_anchor = None;
                        }
                    }
                }
                // Drag released
                if grid_resp.drag_stopped() {
                    self.drag_anchor = None;
                }
            }
        }

        // Cursor info from mouse position over the image
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
                            let file_offset = eff_scroll + byte_index_in_view;
                            let byte_value = data_vec[byte_index_in_view];
                            let bit_index = match mode {
                                DisplayMode::Bit => Some(pixel_index % 8),
                                DisplayMode::Byte => None,
                            };
                            resp.cursor_info = Some(CursorInfo {
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
            if off != eff_scroll {
                resp.scroll_offset = Some(off);
            }
        }

        resp
    }

    pub fn invalidate(&mut self) {
        self.viewer.invalidate();
    }

    pub fn set_h_scroll_target(&mut self, target: f32) {
        self.h_scroll_target = Some(target);
    }
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
