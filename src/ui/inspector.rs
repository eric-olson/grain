use eframe::egui;

use crate::file_handler::MappedFile;
use crate::types::{InspectType, Selection};

/// Show the inspector panel. Returns true if the "Clear" button was clicked.
pub fn show(
    ctx: &egui::Context,
    selection: &Selection,
    file: &MappedFile,
    inspect_type: InspectType,
) -> bool {
    let sel_start = selection.start;
    let sel_end = selection.end;
    let sel_len = sel_end - sel_start + 1;
    let sel_bytes: Vec<u8> = file
        .data()
        .get(sel_start..=sel_end)
        .map(|s| s.to_vec())
        .unwrap_or_default();

    let mut clear_clicked = false;

    egui::TopBottomPanel::bottom("inspector_panel")
        .max_height(200.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Data Inspector");
                if ui.small_button("Clear").clicked() {
                    clear_clicked = true;
                }
            });
            egui::ScrollArea::vertical().show(ui, |ui| {
                draw_inspector(ui, sel_start, sel_end, sel_len, &sel_bytes, inspect_type);
            });
        });

    clear_clicked
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
            copy_button(ui, &range_str);
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
            copy_button(ui, &hex_full);
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
                            let mut a = [0u8; 8];
                            a.copy_from_slice(c);
                            let be = u64::from_be_bytes(a);
                            let le = u64::from_le_bytes(a);
                            (format!("{be}"), format!("{le}"))
                        }
                        InspectType::I64 => {
                            let mut a = [0u8; 8];
                            a.copy_from_slice(c);
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
                            let mut a = [0u8; 8];
                            a.copy_from_slice(c);
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
                        copy_button(ui, &be_str);
                    } else {
                        ui.label(format!("{inspect_type} BE / LE:"));
                        ui.monospace(format!("{be_str} / {le_str}"));
                        copy_button(ui, &format!("BE:{be_str} LE:{le_str}"));
                    }
                    ui.end_row();
                } else {
                    // Array display
                    let cap = 64usize; // max elements to display

                    if is_single_byte {
                        let vals: Vec<String> =
                            chunks.iter().take(cap).map(|c| format_chunk(c).0).collect();
                        let arr_str = format!("[{}]", vals.join(", "));
                        let suffix = if count > cap {
                            format!(" …({} more)", count - cap)
                        } else {
                            String::new()
                        };
                        ui.label(format!("[{inspect_type}; {count}]:"));
                        ui.monospace(format!("{arr_str}{suffix}"));
                        let full: Vec<String> = chunks.iter().map(|c| format_chunk(c).0).collect();
                        copy_button(ui, &format!("[{}]", full.join(", ")));
                        ui.end_row();
                    } else {
                        let be_vals: Vec<String> =
                            chunks.iter().take(cap).map(|c| format_chunk(c).0).collect();
                        let le_vals: Vec<String> =
                            chunks.iter().take(cap).map(|c| format_chunk(c).1).collect();
                        let be_arr = format!("[{}]", be_vals.join(", "));
                        let le_arr = format!("[{}]", le_vals.join(", "));
                        let suffix = if count > cap {
                            format!(" …({} more)", count - cap)
                        } else {
                            String::new()
                        };

                        ui.label(format!("[{inspect_type}; {count}] BE:"));
                        ui.monospace(format!("{be_arr}{suffix}"));
                        let full_be: Vec<String> =
                            chunks.iter().map(|c| format_chunk(c).0).collect();
                        copy_button(ui, &format!("[{}]", full_be.join(", ")));
                        ui.end_row();

                        ui.label(format!("[{inspect_type}; {count}] LE:"));
                        ui.monospace(format!("{le_arr}{suffix}"));
                        let full_le: Vec<String> =
                            chunks.iter().map(|c| format_chunk(c).1).collect();
                        copy_button(ui, &format!("[{}]", full_le.join(", ")));
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
            copy_button(ui, &ascii);
            ui.end_row();
        });
}
