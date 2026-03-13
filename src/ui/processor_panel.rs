use eframe::egui;

use crate::pipeline::processors::{BitShift, NrzmDecode, TakeSkip};
use crate::pipeline::Pipeline;

/// Actions returned from the processor panel to the app.
pub struct ProcessorPanelAction {
    pub changed: bool,
}

/// Show the processor panel sidebar.
pub fn show(ctx: &egui::Context, pipeline: &mut Pipeline) -> ProcessorPanelAction {
    let mut action = ProcessorPanelAction { changed: false };

    egui::SidePanel::left("processor_panel")
        .resizable(true)
        .default_width(200.0)
        .min_width(160.0)
        .show(ctx, |ui| {
            ui.heading("Processors");
            ui.separator();

            // Add processor menu
            ui.horizontal(|ui| {
                ui.menu_button("Add…", |ui| {
                    if ui.button("Take/Skip").clicked() {
                        pipeline.push(Box::new(TakeSkip::default()));
                        action.changed = true;
                        ui.close();
                    }
                    if ui.button("Bit Shift").clicked() {
                        pipeline.push(Box::new(BitShift::default()));
                        action.changed = true;
                        ui.close();
                    }
                    if ui.button("NRZ-M").clicked() {
                        pipeline.push(Box::new(NrzmDecode::default()));
                        action.changed = true;
                        ui.close();
                    }
                });
                if !pipeline.is_empty() && ui.button("Clear All").clicked() {
                    pipeline.clear();
                    action.changed = true;
                }
            });

            ui.separator();

            if pipeline.is_empty() {
                ui.label("No processors. Data is displayed raw.");
                return;
            }

            // List processors with config and controls
            let mut remove_idx: Option<usize> = None;
            let mut move_up_idx: Option<usize> = None;
            let mut move_down_idx: Option<usize> = None;
            let count = pipeline.len();

            for i in 0..count {
                let name = pipeline.node_name(i).to_owned();
                egui::CollapsingHeader::new(&name)
                    .id_salt(format!("proc_{i}"))
                    .default_open(true)
                    .show(ui, |ui| {
                        // Reorder buttons
                        ui.horizontal(|ui| {
                            if ui.add_enabled(i > 0, egui::Button::new("Up")).clicked() {
                                move_up_idx = Some(i);
                            }
                            if ui
                                .add_enabled(i + 1 < count, egui::Button::new("Down"))
                                .clicked()
                            {
                                move_down_idx = Some(i);
                            }
                            if ui.button("X").on_hover_text("Remove").clicked() {
                                remove_idx = Some(i);
                            }
                        });

                        // Processor-specific config
                        if pipeline.show_node_config(i, ui) {
                            action.changed = true;
                        }
                    });
            }

            // Apply deferred mutations
            if let Some(i) = move_up_idx {
                pipeline.move_up(i);
                action.changed = true;
            }
            if let Some(i) = move_down_idx {
                pipeline.move_down(i);
                action.changed = true;
            }
            if let Some(i) = remove_idx {
                let id = pipeline.nodes()[i].id;
                pipeline.remove(id);
                action.changed = true;
            }
        });

    action
}
