use eframe::egui;
use egui::{ColorImage, TextureHandle, TextureOptions};

pub struct PixelGridViewer {
    texture: Option<TextureHandle>,
    last_offset: usize,
    last_stride: usize,
    last_data_len: usize,
}

impl Default for PixelGridViewer {
    fn default() -> Self {
        Self {
            texture: None,
            last_offset: usize::MAX,
            last_stride: 0,
            last_data_len: 0,
        }
    }
}

impl PixelGridViewer {
    pub fn invalidate(&mut self) {
        self.last_offset = usize::MAX;
    }

    /// Show the pixel grid. Returns the number of rows displayed.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        data: &[u8],
        stride: usize,
        offset: usize,
        viewport_height: f32,
    ) -> usize {
        if stride == 0 || data.is_empty() {
            ui.label("No data to display");
            return 0;
        }

        let rows = (data.len() + stride - 1) / stride;
        let visible_rows = ((viewport_height / 1.0).ceil() as usize).min(rows);

        let needs_update = self.last_offset != offset
            || self.last_stride != stride
            || self.last_data_len != data.len();

        if needs_update || self.texture.is_none() {
            let image = build_grayscale_image(data, stride, visible_rows);
            let texture = ui.ctx().load_texture("pixel_grid", image, TextureOptions::NEAREST);
            self.texture = Some(texture);
            self.last_offset = offset;
            self.last_stride = stride;
            self.last_data_len = data.len();
        }

        if let Some(tex) = &self.texture {
            let size = tex.size_vec2();
            ui.image(egui::load::SizedTexture::new(tex.id(), size));
        }

        visible_rows
    }
}

fn build_grayscale_image(data: &[u8], stride: usize, max_rows: usize) -> ColorImage {
    let width = stride;
    let height = max_rows;
    let mut pixels = Vec::with_capacity(width * height);

    for row in 0..height {
        for col in 0..width {
            let idx = row * stride + col;
            let val = if idx < data.len() { data[idx] } else { 0 };
            pixels.push(egui::Color32::from_gray(val));
        }
    }

    ColorImage {
        size: [width, height],
        pixels,
    }
}
