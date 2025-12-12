# VAC Downloader GUI

A cross-platform GUI application for managing VAC (Visual Approach Charts) downloads from the French SIA SOFIA API.

![VAC Downloader GUI](docs/screenshot.png)

## Features

- 📋 **Browse VAC Charts**: View all 505+ available VAC charts with OACI codes and city names
- ⬇️ **Download Options**: Download all charts or select specific ones
- 🗑️ **Delete Management**: Remove local VAC entries with confirmation
- ✓ **Status Indicators**: Visual indicators show which charts are available locally
- 🔄 **Auto-Refresh**: Fetch the latest VAC list from the remote API
- 🚀 **Responsive UI**: Background processing keeps the interface smooth

## Requirements

- Rust 1.70 or later
- macOS, Linux, or Windows

## Installation

1. Clone the repository:
```bash
git clone <repository-url>
cd vac-downloader-gui
```

2. Build the application:
```bash
cargo build --release
```

3. Run the application:
```bash
cargo run --release
```

## Usage

### Viewing VAC Charts

On startup, the application automatically fetches the list of available VAC charts from the remote API. You'll see:
- **OACI Code**: Airport identifier (e.g., "LFPG")
- **City**: Airport city name (e.g., "Paris")
- **Local Status**: ✓ (available) or ✗ (not downloaded)

### Downloading VAC Charts

**Download All Charts:**
1. Click the "⬇ Download All" button
2. Wait for the download to complete
3. The status bar shows progress

**Download Selected Charts:**
1. Check the boxes next to desired charts
2. Click "⬇ Download Selected"
3. Only selected charts will be downloaded

### Deleting Local Charts

1. Find a chart with ✓ in the "Local" column
2. Click the "🗑 Delete" button
3. Confirm the deletion
4. The chart is removed from local storage

### Refreshing the List

Click the "🔄 Refresh" button to fetch the latest VAC list from the API and update local availability status.

## Project Structure

```
vac-downloader-gui/
├── src/
│   ├── main.rs       # Application entry point
│   ├── app.rs        # Main application logic and UI
│   └── models.rs     # Data models
├── Cargo.toml        # Dependencies
├── vac_cache.db      # SQLite database (created at runtime)
└── downloads/        # PDF download directory (created at runtime)
```

## Dependencies

- **eframe/egui**: Cross-platform GUI framework
- **vac_downloader**: Core library for VAC management
- **serde**: Data serialization

## Architecture

The application uses:
- **egui**: Immediate mode GUI for responsive interface
- **Thread-based concurrency**: Background operations don't block the UI
- **Arc<Mutex<>>**: Thread-safe state sharing
- **vac_downloader crate**: Handles API communication and local storage

## Configuration

By default, the application uses:
- Database: `vac_cache.db` in the current directory
- Downloads: `downloads/` folder in the current directory

These paths are currently hardcoded but can be modified in `src/app.rs`.

## License

See the vac_downloader crate for license information.

## Contributing

Contributions are welcome! Please feel free to submit pull requests or open issues.

## Acknowledgments

- Built with [egui](https://github.com/emilk/egui) - An easy-to-use immediate mode GUI library
- Uses the [vac_downloader](../vac-downloader) crate for VAC management
