use crate::config::Config;
use crate::models::{OperationStatus, VacEntryWithSelection};
use eframe::egui;
use std::sync::{Arc, Mutex};
use std::thread;

pub struct VacDownloaderApp {
    /// List of VAC entries
    vac_entries: Arc<Mutex<Vec<VacEntryWithSelection>>>,
    /// Current operation status
    status: Arc<Mutex<OperationStatus>>,
    /// Shared VacDownloader instance (benefits from caching)
    downloader: Arc<Mutex<vac_downloader::VacDownloader>>,
    /// Application configuration
    config: Config,
    /// Show delete confirmation dialog
    delete_confirmation: Option<String>,
}

impl VacDownloaderApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Customize egui style
        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        cc.egui_ctx.set_style(style);

        // Load configuration
        let config = Config::load();
        println!("📂 Database: {}", config.database_path);
        println!("📥 Downloads: {}", config.download_directory);

        // Initialize VacDownloader with config paths
        let downloader =
            vac_downloader::VacDownloader::new(&config.database_path, &config.download_directory)
                .expect("Failed to initialize VacDownloader");

        let app = Self {
            vac_entries: Arc::new(Mutex::new(Vec::new())),
            status: Arc::new(Mutex::new(OperationStatus::Idle)),
            downloader: Arc::new(Mutex::new(downloader)),
            config,
            delete_confirmation: None,
        };

        // Fetch the VAC list on startup
        app.fetch_vac_list();

        app
    }

    fn fetch_vac_list(&self) {
        let vac_entries = self.vac_entries.clone();
        let status = self.status.clone();
        let downloader = self.downloader.clone();

        *status.lock().unwrap() = OperationStatus::FetchingList;

        thread::spawn(move || {
            let downloader = downloader.lock().unwrap();
            match downloader.list_vacs(None) {
                Ok(vacs) => {
                    let entries: Vec<VacEntryWithSelection> =
                        vacs.into_iter().map(VacEntryWithSelection::new).collect();
                    *vac_entries.lock().unwrap() = entries;
                    *status.lock().unwrap() = OperationStatus::Idle;
                }
                Err(e) => {
                    *status.lock().unwrap() =
                        OperationStatus::Error(format!("Failed to fetch list: {}", e));
                }
            }
        });
    }

    fn download_all(&self) {
        let status = self.status.clone();
        let vac_entries = self.vac_entries.clone();
        let downloader = self.downloader.clone();

        thread::spawn(move || {
            let entries = vac_entries.lock().unwrap();
            let total = entries.len();
            drop(entries);

            *status.lock().unwrap() = OperationStatus::Downloading { current: 0, total };

            let downloader = downloader.lock().unwrap();
            match downloader.sync(None) {
                Ok(_) => {
                    // Refresh the list to update local status
                    if let Ok(vacs) = downloader.list_vacs(None) {
                        let entries: Vec<VacEntryWithSelection> =
                            vacs.into_iter().map(VacEntryWithSelection::new).collect();
                        *vac_entries.lock().unwrap() = entries;
                    }
                    *status.lock().unwrap() = OperationStatus::Idle;
                }
                Err(e) => {
                    *status.lock().unwrap() =
                        OperationStatus::Error(format!("Download failed: {}", e));
                }
            }
        });
    }

    fn download_selected(&self) {
        let vac_entries = self.vac_entries.clone();
        let status = self.status.clone();
        let downloader = self.downloader.clone();

        thread::spawn(move || {
            let entries = vac_entries.lock().unwrap();
            let selected_codes: Vec<String> = entries
                .iter()
                .filter(|e| e.selected)
                .map(|e| e.entry.oaci.clone())
                .collect();
            let total = selected_codes.len();
            drop(entries);

            if total == 0 {
                return;
            }

            *status.lock().unwrap() = OperationStatus::Downloading { current: 0, total };

            let downloader = downloader.lock().unwrap();
            match downloader.sync(Some(&selected_codes)) {
                Ok(_) => {
                    // Refresh the list to update local status
                    if let Ok(vacs) = downloader.list_vacs(None) {
                        let new_entries: Vec<VacEntryWithSelection> =
                            vacs.into_iter().map(VacEntryWithSelection::new).collect();
                        *vac_entries.lock().unwrap() = new_entries;
                    }
                    *status.lock().unwrap() = OperationStatus::Idle;
                }
                Err(e) => {
                    *status.lock().unwrap() =
                        OperationStatus::Error(format!("Download failed: {}", e));
                }
            }
        });
    }

    fn delete_vac(&self, oaci_code: String) {
        let status = self.status.clone();
        let vac_entries = self.vac_entries.clone();
        let downloader = self.downloader.clone();

        *status.lock().unwrap() = OperationStatus::Deleting(oaci_code.clone());

        thread::spawn(move || {
            let downloader = downloader.lock().unwrap();
            match downloader.delete(&oaci_code) {
                Ok(_) => {
                    // Update the local status in the list
                    let mut entries = vac_entries.lock().unwrap();
                    if let Some(entry) = entries.iter_mut().find(|e| e.entry.oaci == oaci_code) {
                        entry.entry.available_locally = false;
                    }
                    *status.lock().unwrap() = OperationStatus::Idle;
                }
                Err(e) => {
                    *status.lock().unwrap() =
                        OperationStatus::Error(format!("Delete failed: {}", e));
                }
            }
        });
    }
}

impl eframe::App for VacDownloaderApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request repaint to keep UI responsive during async operations
        ctx.request_repaint();

        // Top panel with toolbar
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("VAC Downloader");

                ui.separator();

                let status_guard = self.status.lock().unwrap();
                let is_busy = status_guard.is_busy();
                drop(status_guard);

                if ui
                    .add_enabled(!is_busy, egui::Button::new("🔄 Refresh"))
                    .clicked()
                {
                    self.fetch_vac_list();
                }

                if ui
                    .add_enabled(!is_busy, egui::Button::new("⬇ Download All"))
                    .clicked()
                {
                    self.download_all();
                }

                let entries = self.vac_entries.lock().unwrap();
                let has_selection = entries.iter().any(|e| e.selected);
                drop(entries);

                if ui
                    .add_enabled(
                        !is_busy && has_selection,
                        egui::Button::new("⬇ Download Selected"),
                    )
                    .clicked()
                {
                    self.download_selected();
                }
            });
        });

        // Bottom panel with status bar
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Status:");
                let status = self.status.lock().unwrap();
                ui.label(status.to_string());
            });
        });

        // Central panel with VAC list
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Available VAC Charts");
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut entries = self.vac_entries.lock().unwrap();

                if entries.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label("No VAC entries loaded. Click Refresh to fetch the list.");
                    });
                } else {
                    // Table header
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Select").strong());
                        ui.separator();
                        ui.label(egui::RichText::new("OACI Code").strong());
                        ui.separator();
                        ui.label(egui::RichText::new("City").strong());
                        ui.separator();
                        ui.label(egui::RichText::new("Local").strong());
                        ui.separator();
                        ui.label(egui::RichText::new("Actions").strong());
                    });
                    ui.separator();

                    // Table rows
                    for entry in entries.iter_mut() {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut entry.selected, "");
                            ui.separator();
                            ui.label(&entry.entry.oaci);
                            ui.separator();
                            ui.label(&entry.entry.city);
                            ui.separator();

                            if entry.entry.available_locally {
                                ui.label(egui::RichText::new("✓").color(egui::Color32::GREEN));
                            } else {
                                ui.label(egui::RichText::new("✗").color(egui::Color32::RED));
                            }

                            ui.separator();

                            if entry.entry.available_locally && ui.button("🗑 Delete").clicked() {
                                self.delete_confirmation = Some(entry.entry.oaci.clone());
                            }
                        });
                        ui.separator();
                    }
                }
            });
        });

        // Delete confirmation dialog
        if let Some(oaci_code) = &self.delete_confirmation.clone() {
            egui::Window::new("Confirm Delete")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(format!("Are you sure you want to delete {}?", oaci_code));
                    ui.horizontal(|ui| {
                        if ui.button("Yes").clicked() {
                            self.delete_vac(oaci_code.clone());
                            self.delete_confirmation = None;
                        }
                        if ui.button("No").clicked() {
                            self.delete_confirmation = None;
                        }
                    });
                });
        }
    }
}
