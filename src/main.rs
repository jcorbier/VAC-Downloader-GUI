mod app;
mod config;
mod models;

use eframe::egui;

fn main() -> eframe::Result<()> {
    // Load application icon
    let icon_data = load_icon();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_min_inner_size([800.0, 600.0])
            .with_icon(icon_data),
        ..Default::default()
    };

    eframe::run_native(
        "VAC Downloader",
        options,
        Box::new(|cc| Ok(Box::new(app::VacDownloaderApp::new(cc)))),
    )
}

fn load_icon() -> egui::IconData {
    let icon_bytes = include_bytes!("../assets/icons/256x256.png");
    let image = image::load_from_memory(icon_bytes)
        .expect("Failed to load application icon")
        .to_rgba8();

    egui::IconData {
        rgba: image.as_raw().clone(),
        width: image.width(),
        height: image.height(),
    }
}
