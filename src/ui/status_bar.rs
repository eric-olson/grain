use eframe::egui;

use crate::types::CursorInfo;

pub fn show(ui: &mut egui::Ui, cursor_info: &Option<CursorInfo>) {
    ui.horizontal(|ui| {
        if let Some(info) = cursor_info {
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
}
