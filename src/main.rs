mod app;
mod file_handler;
mod stride_detect;
mod sync_search;
mod types;
mod ui;
mod viewer;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1024.0, 768.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Grain",
        options,
        Box::new(|_cc| Ok(Box::new(app::App::default()))),
    )
}
