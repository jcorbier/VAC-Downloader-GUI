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

/// Wrapper around vac_downloader::VacEntry with UI-specific state
use std::fmt::Display;

pub struct VacEntryWithSelection {
    /// The underlying VAC entry from the library
    pub entry: vac_downloader::VacEntry,
    /// Whether this entry is selected for download (UI state)
    pub selected: bool,
}

impl VacEntryWithSelection {
    pub fn new(entry: vac_downloader::VacEntry) -> Self {
        Self {
            entry,
            selected: false,
        }
    }
}

/// Application operation status
#[derive(Debug, Clone, PartialEq)]
pub enum OperationStatus {
    Idle,
    FetchingList,
    Downloading { current: usize, total: usize },
    Deleting(String),
    Error(String),
}

impl OperationStatus {
    pub fn is_busy(&self) -> bool {
        !matches!(self, OperationStatus::Idle | OperationStatus::Error(_))
    }
}

impl Display for OperationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            OperationStatus::Idle => "Ready".to_string(),
            OperationStatus::FetchingList => "Fetching VAC list...".to_string(),
            OperationStatus::Downloading { current, total } => {
                format!("Downloading {} of {}...", current, total)
            }
            OperationStatus::Deleting(oaci) => format!("Deleting {}...", oaci),
            OperationStatus::Error(msg) => format!("Error: {}", msg),
        };

        write!(f, "{}", s)
    }
}
