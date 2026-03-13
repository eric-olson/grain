use eframe::egui;

use crate::types::InspectType;
use crate::viewer::DisplayMode;

pub struct MenuBarResponse {
    pub open_file: bool,
    pub show_stride_detect: bool,
    pub stride: Option<usize>,
    pub zoom: Option<f32>,
    pub display_mode: Option<DisplayMode>,
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_arguments
)]
pub fn show(
    ui: &mut egui::Ui,
    stride: usize,
    zoom: f32,
    display_mode: DisplayMode,
    show_hex_panel: &mut bool,
    show_inspector: &mut bool,
    show_processors: &mut bool,
    inspect_type: &mut InspectType,
    file_info: Option<(&str, usize)>,
    has_file: bool,
) -> MenuBarResponse {
    let mut resp = MenuBarResponse {
        open_file: false,
        show_stride_detect: false,
        stride: None,
        zoom: None,
        display_mode: None,
    };

    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            if ui.button("Open…").clicked() {
                resp.open_file = true;
                ui.close();
            }
        });

        ui.separator();

        ui.label("Stride:");
        let mut stride_val = stride as f64;
        let response = ui.add(
            egui::DragValue::new(&mut stride_val)
                .range(1..=4096)
                .speed(1),
        );
        if response.changed() {
            resp.stride = Some((stride_val as usize).max(1));
        }
        if ui.button("Auto").clicked() && has_file {
            resp.show_stride_detect = true;
        }

        ui.separator();

        ui.label("Zoom:");
        let mut zoom_val = zoom;
        let response = ui.add(
            egui::DragValue::new(&mut zoom_val)
                .range(1.0..=32.0)
                .speed(0.1)
                .suffix("x"),
        );
        if response.changed() {
            resp.zoom = Some(zoom_val.clamp(1.0, 32.0));
        }

        ui.separator();

        ui.label("Mode:");
        let byte_selected = display_mode == DisplayMode::Byte;
        if ui.selectable_label(byte_selected, "Byte").clicked() && !byte_selected {
            resp.display_mode = Some(DisplayMode::Byte);
        }
        if ui.selectable_label(!byte_selected, "Bit").clicked() && byte_selected {
            resp.display_mode = Some(DisplayMode::Bit);
        }

        ui.separator();

        if ui.selectable_label(*show_hex_panel, "Hex").clicked() {
            *show_hex_panel = !*show_hex_panel;
        }
        if ui.selectable_label(*show_inspector, "Inspector").clicked() {
            *show_inspector = !*show_inspector;
        }
        if ui
            .selectable_label(*show_processors, "Processors")
            .clicked()
        {
            *show_processors = !*show_processors;
        }

        ui.separator();

        ui.label("Type:");
        egui::ComboBox::from_id_salt("inspect_type")
            .selected_text(inspect_type.to_string())
            .width(50.0)
            .show_ui(ui, |ui| {
                for ty in InspectType::ALL {
                    ui.selectable_value(inspect_type, ty, ty.to_string());
                }
            });

        ui.separator();

        if let Some((name, len)) = file_info {
            ui.label(format!("{name} — {len} bytes"));
        } else {
            ui.label("No file open");
        }
    });

    resp
}
