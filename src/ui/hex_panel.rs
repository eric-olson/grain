use eframe::egui;

use crate::file_handler::MappedFile;

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn show(
    ctx: &egui::Context,
    file: &MappedFile,
    scroll_offset: usize,
    cursor_offset: Option<usize>,
) {
    egui::SidePanel::left("hex_panel")
        .default_width(520.0)
        .show(ctx, |ui| {
            ui.heading("Hex Dump");
            let bytes_per_line = 16usize;
            let line_count =
                ui.available_height() / ui.text_style_height(&egui::TextStyle::Monospace);
            let num_lines = (line_count as usize).max(1);
            let data = file.get_range(scroll_offset, num_lines * bytes_per_line);

            egui::ScrollArea::vertical().show(ui, |ui| {
                for line in 0..num_lines {
                    let line_start = line * bytes_per_line;
                    if line_start >= data.len() {
                        break;
                    }
                    let file_addr = scroll_offset + line_start;
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
        });
}
