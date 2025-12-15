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
    /// Show delete confirmation dialog (list of OACI codes to delete)
    delete_confirmation: Option<Vec<String>>,
    /// Current sort column
    sort_column: SortColumn,
    /// Sort ascending or descending
    sort_ascending: bool,
    /// Search query for filtering VAC list
    search_query: String,
    /// Cache of needs_update status for each OACI code
    needs_update_cache: Arc<Mutex<std::collections::HashMap<String, bool>>>,
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
            needs_update_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
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

    fn delete_selected(&self) {
        let vac_entries = self.vac_entries.clone();
        let status = self.status.clone();
        let downloader = self.downloader.clone();

        thread::spawn(move || {
            let entries = vac_entries.lock().unwrap();
            let selected_codes: Vec<String> = entries
                .iter()
                .filter(|e| e.selected && e.entry.available_locally)
                .map(|e| e.entry.oaci.clone())
                .collect();
            drop(entries);

            if selected_codes.is_empty() {
                return;
            }

            let total = selected_codes.len();
            let downloader = downloader.lock().unwrap();

            for (idx, oaci_code) in selected_codes.iter().enumerate() {
                *status.lock().unwrap() =
                    OperationStatus::Deleting(format!("{} ({}/{})", oaci_code, idx + 1, total));

                match downloader.delete(oaci_code) {
                    Ok(_) => {
                        // Update the local status in the list
                        let mut entries = vac_entries.lock().unwrap();
                        if let Some(entry) = entries.iter_mut().find(|e| e.entry.oaci == *oaci_code)
                        {
                            entry.entry.available_locally = false;
                            entry.selected = false; // Deselect after deletion
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to delete {}: {}", oaci_code, e);
                    }
                }
            }

            *status.lock().unwrap() = OperationStatus::Idle;
        });
    }

    fn update_vac(&self, oaci_code: String) {
        let status = self.status.clone();
        let vac_entries = self.vac_entries.clone();
        let downloader = self.downloader.clone();
        let needs_update_cache = self.needs_update_cache.clone();

        *status.lock().unwrap() = OperationStatus::Downloading {
            current: 1,
            total: 1,
        };

        thread::spawn(move || {
            let downloader = downloader.lock().unwrap();
            // Use sync with specific OACI code to update this entry
            match downloader.sync(Some(&[oaci_code.clone()])) {
                Ok(_) => {
                    // Clear the needs_update cache for this entry
                    needs_update_cache.lock().unwrap().remove(&oaci_code);

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

    fn check_needs_update(&self, oaci_code: String) {
        let downloader = self.downloader.clone();
        let needs_update_cache = self.needs_update_cache.clone();

        thread::spawn(move || {
            let downloader = downloader.lock().unwrap();
            match downloader.needs_update(&oaci_code) {
                Ok(needs_update) => {
                    let mut cache = needs_update_cache.lock().unwrap();
                    cache.insert(oaci_code, needs_update);
                }
                Err(_) => {
                    // If we can't determine, assume it doesn't need update
                    let mut cache = needs_update_cache.lock().unwrap();
                    cache.insert(oaci_code, false);
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

    fn open_pdf(&self, oaci_code: &str) {
        let downloader = self.downloader.lock().unwrap();
        match downloader.get_pdf_path(oaci_code) {
            Ok(path) => {
                if let Err(e) = open::that(&path) {
                    eprintln!("Failed to open PDF for {}: {}", oaci_code, e);
                }
            }
            Err(e) => {
                eprintln!("Failed to get PDF path for {}: {}", oaci_code, e);
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

                // Check if any selected entries are available locally
                let entries = self.vac_entries.lock().unwrap();
                let has_local_selection = entries
                    .iter()
                    .any(|e| e.selected && e.entry.available_locally);
                drop(entries);

                if ui
                    .add_enabled(
                        !is_busy && has_local_selection,
                        egui::Button::new("🗑 Delete Selected"),
                    )
                    .clicked()
                {
                    // Collect selected OACI codes for confirmation
                    let entries = self.vac_entries.lock().unwrap();
                    let selected_codes: Vec<String> = entries
                        .iter()
                        .filter(|e| e.selected && e.entry.available_locally)
                        .map(|e| e.entry.oaci.clone())
                        .collect();
                    drop(entries);
                    self.delete_confirmation = Some(selected_codes);
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
                let mut delete_oaci: Option<Vec<String>> = None;
                let mut open_pdf_oaci: Option<String> = None;
                let mut need_sort = false;
                let mut oaci_codes_to_check: Vec<String> = Vec::new();

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
                            // Select All checkbox
                            let all_filtered_selected =
                                filtered_indices.iter().all(|&idx| entries[idx].selected);
                            let mut select_all = all_filtered_selected;
                            if ui.checkbox(&mut select_all, "").changed() {
                                // Toggle all filtered entries
                                for &idx in &filtered_indices {
                                    entries[idx].selected = select_all;
                                }
                            }

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

                                // OACI code - clickable if available locally
                                if entry.entry.available_locally {
                                    if ui.link(&entry.entry.oaci).clicked() {
                                        open_pdf_oaci = Some(entry.entry.oaci.clone());
                                    }
                                } else {
                                    ui.label(&entry.entry.oaci);
                                }

                                // City name - clickable if available locally
                                if entry.entry.available_locally {
                                    if ui.link(&entry.entry.city).clicked() {
                                        open_pdf_oaci = Some(entry.entry.oaci.clone());
                                    }
                                } else {
                                    ui.label(&entry.entry.city);
                                }

                                // Check update status once for this entry (if available locally)
                                let needs_update = if entry.entry.available_locally {
                                    let needs_update_cache =
                                        self.needs_update_cache.lock().unwrap();
                                    let status = needs_update_cache.get(&entry.entry.oaci).copied();
                                    drop(needs_update_cache);

                                    // If we don't have the status yet, mark it for checking
                                    if status.is_none() {
                                        oaci_codes_to_check.push(entry.entry.oaci.clone());
                                    }
                                    status
                                } else {
                                    None
                                };

                                // Local status icon
                                if entry.entry.available_locally {
                                    // Show appropriate icon based on update status
                                    if needs_update.unwrap_or(false) {
                                        ui.label(
                                            egui::RichText::new("U")
                                                .color(egui::Color32::from_rgb(255, 165, 0)),
                                        ); // Orange/yellow warning
                                    } else {
                                        ui.label(
                                            egui::RichText::new("Y").color(egui::Color32::GREEN),
                                        );
                                    }
                                } else {
                                    ui.label(egui::RichText::new("N").color(egui::Color32::RED));
                                }

                                // Actions column
                                ui.horizontal(|ui| {
                                    if entry.entry.available_locally {
                                        // Enable Update button only if we know it needs an update
                                        let update_enabled =
                                            !is_busy && needs_update.unwrap_or(false);

                                        if ui
                                            .add_enabled(
                                                update_enabled,
                                                egui::Button::new("Update"),
                                            )
                                            .clicked()
                                        {
                                            update_oaci = Some(entry.entry.oaci.clone());
                                        }

                                        if ui.button("Delete").clicked() {
                                            delete_oaci = Some(vec![entry.entry.oaci.clone()]);
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

                // Check update status for entries that need it
                for oaci in oaci_codes_to_check {
                    self.check_needs_update(oaci);
                }

                if let Some(oaci) = update_oaci {
                    self.update_vac(oaci);
                }
                if let Some(oaci) = open_pdf_oaci {
                    self.open_pdf(&oaci);
                }
                if let Some(oaci_codes) = delete_oaci {
                    self.delete_confirmation = Some(oaci_codes);
                }
            });
        });

        // Delete confirmation dialog
        if let Some(oaci_codes) = &self.delete_confirmation.clone() {
            egui::Window::new("Confirm Delete")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    if oaci_codes.len() == 1 {
                        ui.label(format!(
                            "Are you sure you want to delete {}?",
                            oaci_codes[0]
                        ));
                    } else {
                        ui.label(format!(
                            "Are you sure you want to delete {} VAC entries?",
                            oaci_codes.len()
                        ));
                        ui.label(format!("Entries: {}", oaci_codes.join(", ")));
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Yes").clicked() {
                            if oaci_codes.len() == 1 {
                                self.delete_vac(oaci_codes[0].clone());
                            } else {
                                self.delete_selected();
                            }
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
