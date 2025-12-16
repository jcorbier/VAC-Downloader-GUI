/*
 * Copyright (c) 2025 Jeremie Corbier
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy of
 * this software and associated documentation files (the "Software"), to deal in
 * the Software without restriction, including without limitation the rights to
 * use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of
 * the Software, and to permit persons to whom the Software is furnished to do so,
 * subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS
 * FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
 * COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER
 * IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
 * CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 */

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Path to the SQLite database file
    pub database_path: String,
    /// Directory where VAC PDFs will be downloaded
    pub download_directory: String,
}

impl Default for Config {
    fn default() -> Self {
        if let Some(cache_dir) = dirs::cache_dir() {
            let app_cache_dir = cache_dir.join("vac-downloader-gui");
            fs::create_dir_all(&app_cache_dir).ok();

            Self {
                database_path: app_cache_dir.join("cache.db").to_string_lossy().to_string(),
                download_directory: app_cache_dir
                    .join("downloads")
                    .to_string_lossy()
                    .to_string(),
            }
        } else {
            Self {
                database_path: "vac_cache.db".to_string(),
                download_directory: "downloads".to_string(),
            }
        }
    }
}

impl Config {
    /// Get the path to the configuration file
    pub fn config_file_path() -> PathBuf {
        if let Some(config_dir) = dirs::config_dir() {
            let app_config_dir = config_dir.join("vac-downloader-gui");
            fs::create_dir_all(&app_config_dir).ok();
            app_config_dir.join("config.toml")
        } else {
            PathBuf::from("config.toml")
        }
    }

    /// Load configuration from file, or create default if it doesn't exist
    pub fn load() -> Self {
        let config_path = Self::config_file_path();

        if config_path.exists() {
            match fs::read_to_string(&config_path) {
                Ok(contents) => match toml::from_str(&contents) {
                    Ok(config) => {
                        println!("📁 Loaded config from: {:?}", config_path);
                        return config;
                    }
                    Err(e) => {
                        eprintln!("⚠️  Failed to parse config file: {}", e);
                        eprintln!("   Using default configuration");
                    }
                },
                Err(e) => {
                    eprintln!("⚠️  Failed to read config file: {}", e);
                    eprintln!("   Using default configuration");
                }
            }
        }

        // Create default config file
        let config = Self::default();
        if let Err(e) = config.save() {
            eprintln!("⚠️  Failed to save default config: {}", e);
        } else {
            println!("📝 Created default config at: {:?}", config_path);
        }

        config
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = Self::config_file_path();
        let toml_string = toml::to_string_pretty(self)?;
        fs::write(&config_path, toml_string)?;
        println!("💾 Saved config to: {:?}", config_path);
        Ok(())
    }
}
