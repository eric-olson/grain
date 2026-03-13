use eframe::egui;

use crate::file_handler::MappedFile;
use crate::pipeline::Pipeline;
use crate::types::{CursorInfo, InspectType, Selection};
use crate::ui::search_panel::{SearchAction, SearchPanel};
use crate::ui::stride_dialog::StrideDialog;
use crate::ui::viewport::Viewport;
use crate::viewer::DisplayMode;

pub struct App {
    file: Option<MappedFile>,
    stride: usize,
    scroll_offset: usize,
    zoom: f32,
    display_mode: DisplayMode,
    cursor_info: Option<CursorInfo>,
    show_inspector: bool,
    show_processor_panel: bool,
    inspect_type: InspectType,
    selection: Option<Selection>,
    // Panel state
    search: SearchPanel,
    stride_dialog: StrideDialog,
    viewport: Viewport,
    pipeline: Pipeline,
}

impl Default for App {
    fn default() -> Self {
        Self {
            file: None,
            stride: 256,
            scroll_offset: 0,
            zoom: 1.0,
            display_mode: DisplayMode::Byte,
            cursor_info: None,
            show_inspector: true,
            show_processor_panel: false,
            inspect_type: InspectType::U8,
            selection: None,
            search: SearchPanel::default(),
            stride_dialog: StrideDialog::default(),
            viewport: Viewport::default(),
            pipeline: Pipeline::new(),
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
                    self.viewport.invalidate();
                    self.pipeline.invalidate();
                    self.search.reset();
                }
                Err(e) => {
                    eprintln!("Error opening file: {e}");
                }
            }
        }
    }

    fn max_offset(&self) -> usize {
        self.file.as_ref().map_or(0, |f| {
            if self.pipeline.is_active() {
                self.pipeline.output_len(f.len()).saturating_sub(1)
            } else {
                f.len().saturating_sub(1)
            }
        })
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll background tasks
        self.search.poll();
        self.stride_dialog.poll();

        if self.search.is_running() || self.stride_dialog.is_running() {
            ctx.request_repaint();
        }

        // Escape clears selection
        if self.selection.is_some() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.selection = None;
            self.viewport.invalidate();
        }

        // Menu bar
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            let file_info = self.file.as_ref().map(|f| (f.name(), f.len()));
            // Convert (&str, usize) for the lifetime — need to reborrow
            let file_info_ref = file_info.as_ref().map(|(n, l)| (*n, *l));
            let menu_resp = crate::ui::menu_bar::show(
                ui,
                self.stride,
                self.zoom,
                self.display_mode,
                &mut self.show_inspector,
                &mut self.show_processor_panel,
                &mut self.inspect_type,
                file_info_ref,
                self.file.is_some(),
            );
            if menu_resp.open_file {
                self.open_file();
            }
            if menu_resp.show_stride_detect {
                self.stride_dialog.open();
            }
            if let Some(s) = menu_resp.stride {
                self.stride = s;
                self.viewport.invalidate();
            }
            if let Some(z) = menu_resp.zoom {
                self.zoom = z;
                self.viewport.invalidate();
            }
            if let Some(m) = menu_resp.display_mode {
                self.display_mode = m;
                self.viewport.invalidate();
            }
        });

        // Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            crate::ui::status_bar::show(ui, &self.cursor_info);
        });

        // Inspector panel
        if self.show_inspector {
            if let (Some(sel), Some(file)) = (&self.selection, &self.file) {
                if crate::ui::inspector::show(ctx, sel, file, self.inspect_type) {
                    self.selection = None;
                    self.viewport.invalidate();
                }
            }
        }

        // Search panel
        let search_action = self.search.show(ctx, &self.file);
        if let Some(SearchAction { jump_to_offset }) = search_action {
            let (scroll, h_scroll) = SearchPanel::compute_jump(
                jump_to_offset,
                self.stride,
                self.zoom,
                self.display_mode,
            );
            self.scroll_offset = scroll;
            self.viewport.set_h_scroll_target(h_scroll);
            self.viewport.invalidate();
        }

        // Stride detection dialog
        if let Some(s) = self.stride_dialog.show(ctx, &self.file, self.display_mode) {
            self.stride = s;
            self.viewport.invalidate();
        }

        // Processor panel
        if self.show_processor_panel {
            let panel_action = crate::ui::processor_panel::show(ctx, &mut self.pipeline);
            if panel_action.changed {
                self.pipeline.invalidate();
                self.viewport.invalidate();
            }
        }

        // Central viewport
        let max_offset = self.max_offset();
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.file.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.label("Open a binary file to begin (File > Open)");
                });
                return;
            }

            let file = self.file.as_ref().unwrap();
            let pipeline = if self.pipeline.is_active() {
                Some(&mut self.pipeline)
            } else {
                None
            };
            let vp_resp = self.viewport.show(
                ui,
                ctx,
                file,
                pipeline,
                self.stride,
                self.scroll_offset,
                self.zoom,
                self.display_mode,
                self.inspect_type,
                self.search.results(),
                self.search.pattern_len(),
                &self.selection,
                max_offset,
            );

            // Apply viewport response
            self.cursor_info = vp_resp.cursor_info;
            if let Some(off) = vp_resp.scroll_offset {
                self.scroll_offset = off;
                self.viewport.invalidate();
            }
            if let Some(sel) = vp_resp.selection {
                self.selection = sel;
                self.viewport.invalidate();
            }
            if let Some(z) = vp_resp.zoom {
                self.zoom = z;
                self.viewport.invalidate();
            }
        });
    }
}
