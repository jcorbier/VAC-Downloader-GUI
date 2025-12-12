mod app;
mod config;
mod models;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "VAC Downloader",
        options,
        Box::new(|cc| Ok(Box::new(app::VacDownloaderApp::new(cc)))),
    )
}
