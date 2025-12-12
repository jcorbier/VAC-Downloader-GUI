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
