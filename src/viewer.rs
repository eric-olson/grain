use std::collections::HashSet;

use eframe::egui;
use egui::{ColorImage, TextureHandle, TextureOptions, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayMode {
    Byte,
    Bit,
}

impl DisplayMode {
    /// How many pixels one byte produces.
    pub fn pixels_per_byte(self) -> usize {
        match self {
            DisplayMode::Byte => 1,
            DisplayMode::Bit => 8,
        }
    }

    /// How many bytes are needed to fill `pixel_count` pixels.
    pub fn bytes_for_pixels(self, pixel_count: usize) -> usize {
        match self {
            DisplayMode::Byte => pixel_count,
            DisplayMode::Bit => pixel_count.div_ceil(8),
        }
    }
}

pub struct PixelGridViewer {
    texture: Option<TextureHandle>,
    last_offset: usize,
    last_stride: usize,
    last_data_len: usize,
    last_zoom: u32,
    last_highlight_count: usize,
    last_selection_count: usize,
    last_mode: Option<DisplayMode>,
}

impl Default for PixelGridViewer {
    fn default() -> Self {
        Self {
            texture: None,
            last_offset: usize::MAX,
            last_stride: 0,
            last_data_len: 0,
            last_zoom: 0,
            last_highlight_count: 0,
            last_selection_count: 0,
            last_mode: None,
        }
    }
}

impl PixelGridViewer {
    pub fn invalidate(&mut self) {
        self.last_offset = usize::MAX;
    }

    /// Show the pixel grid. Returns (`visible_rows`, `image_rect`, `response`).
    #[allow(
        clippy::too_many_arguments,
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        data: &[u8],
        stride: usize,
        offset: usize,
        viewport_height: f32,
        zoom: f32,
        highlights: &HashSet<usize>,
        selection: &HashSet<usize>,
        mode: DisplayMode,
    ) -> (usize, egui::Rect, egui::Response) {
        if stride == 0 || data.is_empty() {
            ui.label("No data to display");
            let r = ui.label("");
            return (0, egui::Rect::NOTHING, r);
        }

        let total_pixels = data.len() * mode.pixels_per_byte();
        let rows = total_pixels.div_ceil(stride);
        let visible_rows = ((viewport_height / zoom).ceil() as usize).min(rows);
        let zoom_bits = zoom.to_bits();

        let needs_update = self.last_offset != offset
            || self.last_stride != stride
            || self.last_data_len != data.len()
            || self.last_zoom != zoom_bits
            || self.last_highlight_count != highlights.len()
            || self.last_selection_count != selection.len()
            || self.last_mode != Some(mode);

        if needs_update || self.texture.is_none() {
            let image = build_image(data, stride, visible_rows, highlights, selection, mode);
            let texture = ui
                .ctx()
                .load_texture("pixel_grid", image, TextureOptions::NEAREST);
            self.texture = Some(texture);
            self.last_offset = offset;
            self.last_stride = stride;
            self.last_data_len = data.len();
            self.last_zoom = zoom_bits;
            self.last_highlight_count = highlights.len();
            self.last_selection_count = selection.len();
            self.last_mode = Some(mode);
        }

        if let Some(tex) = &self.texture {
            let display_size = Vec2::new(stride as f32 * zoom, visible_rows as f32 * zoom);
            let response = ui.allocate_rect(
                egui::Rect::from_min_size(ui.cursor().min, display_size),
                egui::Sense::click_and_drag(),
            );
            let image_rect = response.rect;
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter()
                .image(tex.id(), image_rect, uv, egui::Color32::WHITE);
            (visible_rows, image_rect, response)
        } else {
            let r = ui.label("");
            (0, egui::Rect::NOTHING, r)
        }
    }
}

fn build_image(
    data: &[u8],
    stride: usize,
    max_rows: usize,
    highlights: &HashSet<usize>,
    selection: &HashSet<usize>,
    mode: DisplayMode,
) -> ColorImage {
    match mode {
        DisplayMode::Byte => build_byte_image(data, stride, max_rows, highlights, selection),
        DisplayMode::Bit => build_bit_image(data, stride, max_rows, highlights, selection),
    }
}

fn build_byte_image(
    data: &[u8],
    stride: usize,
    max_rows: usize,
    highlights: &HashSet<usize>,
    selection: &HashSet<usize>,
) -> ColorImage {
    let width = stride;
    let height = max_rows;
    let mut pixels = Vec::with_capacity(width * height);

    for row in 0..height {
        for col in 0..width {
            let idx = row * stride + col;
            let val = if idx < data.len() { data[idx] } else { 0 };
            let color = if selection.contains(&idx) {
                // Blue tint blended with data value
                let v = val as u16;
                egui::Color32::from_rgb(
                    (40 + v / 4) as u8,
                    (70 + v / 3) as u8,
                    (130 + v / 2) as u8,
                )
            } else if highlights.contains(&idx) {
                egui::Color32::from_rgb(255, 60 + val / 2, 0)
            } else {
                let is_border = [
                    idx.wrapping_sub(1),
                    idx + 1,
                    idx.wrapping_sub(stride),
                    idx + stride,
                ]
                .iter()
                .any(|&n| highlights.contains(&n));
                if is_border {
                    egui::Color32::from_rgb(255, 0, 0)
                } else {
                    egui::Color32::from_gray(val)
                }
            };
            pixels.push(color);
        }
    }

    ColorImage {
        size: [width, height],
        pixels,
        source_size: egui::Vec2::new(width as f32, height as f32),
    }
}

fn build_bit_image(
    data: &[u8],
    stride: usize,
    max_rows: usize,
    highlights: &HashSet<usize>,
    selection: &HashSet<usize>,
) -> ColorImage {
    let width = stride;
    let height = max_rows;
    let total_bits = data.len() * 8;
    let mut pixels = Vec::with_capacity(width * height);

    for row in 0..height {
        for col in 0..width {
            let bit_idx = row * stride + col;
            let val = if bit_idx < total_bits {
                let byte_idx = bit_idx / 8;
                let bit_pos = 7 - (bit_idx % 8); // MSB first
                (data[byte_idx] >> bit_pos) & 1
            } else {
                0
            };
            let gray = if val == 1 { 255u8 } else { 0u8 };
            let color = if selection.contains(&bit_idx) {
                if val == 1 {
                    egui::Color32::from_rgb(100, 170, 255)
                } else {
                    egui::Color32::from_rgb(30, 60, 140)
                }
            } else if highlights.contains(&bit_idx) {
                egui::Color32::from_rgb(255, if val == 1 { 180 } else { 60 }, 0)
            } else {
                let is_border = [
                    bit_idx.wrapping_sub(1),
                    bit_idx + 1,
                    bit_idx.wrapping_sub(stride),
                    bit_idx + stride,
                ]
                .iter()
                .any(|&n| highlights.contains(&n));
                if is_border {
                    egui::Color32::from_rgb(255, 0, 0)
                } else {
                    egui::Color32::from_gray(gray)
                }
            };
            pixels.push(color);
        }
    }

    ColorImage {
        size: [width, height],
        pixels,
        source_size: egui::Vec2::new(width as f32, height as f32),
    }
}
