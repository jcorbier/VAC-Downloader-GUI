use crate::config::Config;
use crate::models::{OperationStatus, VacEntryWithSelection};
use eframe::egui;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug, Clone, Copy, PartialEq)]
enum SortColumn {
    Oaci,
    City,
}

pub struct VacDownloaderApp {
    /// List of VAC entries
    vac_entries: Arc<Mutex<Vec<VacEntryWithSelection>>>,
    /// Current operation status
    status: Arc<Mutex<OperationStatus>>,
    /// Shared VacDownloader instance (benefits from caching)
    downloader: Arc<Mutex<vac_downloader::VacDownloader>>,
    /// Application configuration
    config: Config,
    /// Editable download directory path (for UI input)
    download_dir_input: String,
    /// Show delete confirmation dialog
    delete_confirmation: Option<String>,
    /// Current sort column
    sort_column: SortColumn,
    /// Sort ascending or descending
    sort_ascending: bool,
    /// Search query for filtering VAC list
    search_query: String,
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
            download_dir_input: config.download_directory.clone(),
            config,
            delete_confirmation: None,
            sort_column: SortColumn::Oaci,
            sort_ascending: true,
            search_query: String::new(),
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

    fn update_vac(&self, oaci_code: String) {
        let status = self.status.clone();
        let vac_entries = self.vac_entries.clone();
        let downloader = self.downloader.clone();

        *status.lock().unwrap() = OperationStatus::Downloading {
            current: 1,
            total: 1,
        };

        thread::spawn(move || {
            let downloader = downloader.lock().unwrap();
            // Use sync with specific OACI code to update this entry
            match downloader.sync(Some(&[oaci_code.clone()])) {
                Ok(_) => {
                    // Refresh the list to update the entry
                    match downloader.list_vacs(None) {
                        Ok(vacs) => {
                            let new_entries: Vec<VacEntryWithSelection> =
                                vacs.into_iter().map(VacEntryWithSelection::new).collect();
                            *vac_entries.lock().unwrap() = new_entries;
                        }
                        Err(_) => {}
                    }
                    *status.lock().unwrap() = OperationStatus::Idle;
                }
                Err(e) => {
                    *status.lock().unwrap() =
                        OperationStatus::Error(format!("Update failed: {}", e));
                }
            }
        });
    }

    fn sort_entries(&mut self) {
        let mut entries = self.vac_entries.lock().unwrap();

        match self.sort_column {
            SortColumn::Oaci => {
                entries.sort_by(|a, b| {
                    let cmp = a.entry.oaci.cmp(&b.entry.oaci);
                    if self.sort_ascending {
                        cmp
                    } else {
                        cmp.reverse()
                    }
                });
            }
            SortColumn::City => {
                entries.sort_by(|a, b| {
                    let cmp = a.entry.city.cmp(&b.entry.city);
                    if self.sort_ascending {
                        cmp
                    } else {
                        cmp.reverse()
                    }
                });
            }
        }
    }

    fn save_config(&mut self) {
        // Update config with new download directory
        self.config.download_directory = self.download_dir_input.clone();

        // Save to file
        match self.config.save() {
            Ok(_) => {
                println!("✅ Configuration saved!");

                // Delete the old database file to reset the cache
                if std::path::Path::new(&self.config.database_path).exists() {
                    match std::fs::remove_file(&self.config.database_path) {
                        Ok(_) => println!("🗑️  Deleted old database cache"),
                        Err(e) => println!("⚠️  Warning: Could not delete old database: {}", e),
                    }
                }

                // Reinitialize VacDownloader with new paths (creates fresh database)
                match vac_downloader::VacDownloader::new(
                    &self.config.database_path,
                    &self.config.download_directory,
                ) {
                    Ok(new_downloader) => {
                        *self.downloader.lock().unwrap() = new_downloader;
                        println!("🔄 VacDownloader reinitialized with new download location");
                        println!("🗄️  Fresh database created");

                        // Refresh the VAC list to update local availability with new path
                        self.fetch_vac_list();

                        *self.status.lock().unwrap() = OperationStatus::Idle;
                    }
                    Err(e) => {
                        *self.status.lock().unwrap() =
                            OperationStatus::Error(format!("Failed to reinitialize: {}", e));
                    }
                }
            }
            Err(e) => {
                *self.status.lock().unwrap() =
                    OperationStatus::Error(format!("Failed to save config: {}", e));
            }
        }
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
            // Download location configuration section
            ui.horizontal(|ui| {
                ui.label("Download Location:");
                ui.text_edit_singleline(&mut self.download_dir_input);

                let status_guard = self.status.lock().unwrap();
                let is_busy = status_guard.is_busy();
                drop(status_guard);

                if ui
                    .add_enabled(!is_busy, egui::Button::new("💾 Save"))
                    .clicked()
                {
                    self.save_config();
                }

                if ui.button("📁 Browse").clicked() {
                    // Open directory picker dialog
                    if let Some(path) = rfd::FileDialog::new()
                        .set_directory(&self.download_dir_input)
                        .pick_folder()
                    {
                        self.download_dir_input = path.display().to_string();
                    }
                }
            });
            ui.label("💡 Warning: changing location will reset the database");
            ui.separator();

            ui.heading("Available VAC Charts");
            ui.separator();

            // Search box
            ui.horizontal(|ui| {
                ui.label("🔍 Search:");
                ui.text_edit_singleline(&mut self.search_query);
                if ui.button("✖").clicked() {
                    self.search_query.clear();
                }
            });
            ui.label("💡 Filter by OACI code or city name");
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut entries = self.vac_entries.lock().unwrap();
                let status_guard = self.status.lock().unwrap();
                let is_busy = status_guard.is_busy();
                drop(status_guard);

                // Collect actions to perform after releasing the lock
                let mut update_oaci: Option<String> = None;
                let mut delete_oaci: Option<String> = None;
                let mut need_sort = false;

                if entries.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label("No VAC entries loaded. Click Refresh to fetch the list.");
                    });
                } else {
                    // Filter entries based on search query - collect indices
                    let search_query_lower = self.search_query.to_lowercase();
                    let filtered_indices: Vec<usize> = entries
                        .iter()
                        .enumerate()
                        .filter(|(_, entry)| {
                            if search_query_lower.is_empty() {
                                true
                            } else {
                                entry
                                    .entry
                                    .oaci
                                    .to_lowercase()
                                    .contains(&search_query_lower)
                                    || entry
                                        .entry
                                        .city
                                        .to_lowercase()
                                        .contains(&search_query_lower)
                            }
                        })
                        .map(|(idx, _)| idx)
                        .collect();

                    // Display count of filtered results
                    if !search_query_lower.is_empty() {
                        ui.label(format!(
                            "Showing {} of {} entries",
                            filtered_indices.len(),
                            entries.len()
                        ));
                    }

                    // Use Grid for proper column alignment
                    egui::Grid::new("vac_table")
                        .striped(true)
                        .spacing([10.0, 4.0])
                        .show(ui, |ui| {
                            // Table header with clickable sort columns
                            ui.label(egui::RichText::new("Select").strong());

                            // OACI Code column - clickable for sorting
                            let oaci_label = if self.sort_column == SortColumn::Oaci {
                                let arrow = if self.sort_ascending { "^" } else { "v" };
                                format!("OACI Code {}", arrow)
                            } else {
                                "OACI Code".to_string()
                            };
                            if ui
                                .button(egui::RichText::new(oaci_label).strong())
                                .clicked()
                            {
                                if self.sort_column == SortColumn::Oaci {
                                    self.sort_ascending = !self.sort_ascending;
                                } else {
                                    self.sort_column = SortColumn::Oaci;
                                    self.sort_ascending = true;
                                }
                                need_sort = true;
                            }

                            // City column - clickable for sorting
                            let city_label = if self.sort_column == SortColumn::City {
                                let arrow = if self.sort_ascending { "^" } else { "v" };
                                format!("City {}", arrow)
                            } else {
                                "City".to_string()
                            };
                            if ui
                                .button(egui::RichText::new(city_label).strong())
                                .clicked()
                            {
                                if self.sort_column == SortColumn::City {
                                    self.sort_ascending = !self.sort_ascending;
                                } else {
                                    self.sort_column = SortColumn::City;
                                    self.sort_ascending = true;
                                }
                                need_sort = true;
                            }

                            ui.label(egui::RichText::new("Local").strong());
                            ui.label(egui::RichText::new("Actions").strong());
                            ui.end_row();

                            // Table rows - only show filtered entries
                            for &idx in &filtered_indices {
                                let entry = &mut entries[idx];
                                ui.checkbox(&mut entry.selected, "");
                                ui.label(&entry.entry.oaci);
                                ui.label(&entry.entry.city);

                                // Local status icon
                                if entry.entry.available_locally {
                                    ui.label(egui::RichText::new("Y").color(egui::Color32::GREEN));
                                } else {
                                    ui.label(egui::RichText::new("N").color(egui::Color32::RED));
                                }

                                // Actions column
                                ui.horizontal(|ui| {
                                    if entry.entry.available_locally {
                                        // Update button (always shown for local entries)
                                        if ui
                                            .add_enabled(!is_busy, egui::Button::new("Update"))
                                            .clicked()
                                        {
                                            update_oaci = Some(entry.entry.oaci.clone());
                                        }

                                        if ui.button("Delete").clicked() {
                                            delete_oaci = Some(entry.entry.oaci.clone());
                                        }
                                    }
                                });

                                ui.end_row();
                            }
                        });
                }

                drop(entries);

                // Execute actions after releasing the lock
                if need_sort {
                    self.sort_entries();
                }
                if let Some(oaci) = update_oaci {
                    self.update_vac(oaci);
                }
                if let Some(oaci) = delete_oaci {
                    self.delete_confirmation = Some(oaci);
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
