use crate::platform::{window_hwnd_isize, windows};
use tauri::Manager;

/// Logical corner radius of a note; matches `noteWindowRadius` in `src/routes/note/[id]/+page.svelte`.
const NOTE_CORNER_RADIUS: f64 = 12.0;

/// Clips a note window to its rounded shape with a native window region.
///
/// CSS alone cannot round a note on Windows: the transparent corners of an embedded note do
/// not show the desktop correctly. Without a region of our own, Windows gives the embedded
/// note (a captioned child window that DWM does not frame) the legacy themed frame region,
/// which rounds only the top corners. Windows keeps an app-set region across layer and style
/// changes, but it does not follow resizes, so this runs again on every resize and DPI change.
pub(super) fn round_note_window_corners(
    w: &tauri::WebviewWindow,
    hwnd_isize: isize,
) -> Result<(), String> {
    let scale = w.scale_factor().map_err(|e| e.to_string())?;
    let radius = (NOTE_CORNER_RADIUS * scale).round() as i32;
    windows::set_rounded_window_region(hwnd_isize, radius)
}

pub fn round_note_window_corners_by_label(app: &tauri::AppHandle, label: &str) {
    if !label.starts_with("note-") {
        return;
    }
    let Some(w) = app.get_webview_window(label) else {
        return;
    };
    let Ok(Some(hwnd_isize)) = window_hwnd_isize(&w) else {
        return;
    };
    if let Err(error) = round_note_window_corners(&w, hwnd_isize) {
        eprintln!("[corners] {label}: {error}");
    }
}
