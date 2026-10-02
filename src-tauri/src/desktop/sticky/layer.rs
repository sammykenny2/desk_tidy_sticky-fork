use crate::notes::{service as notes_service, store as notes_store, NoteSortMode};
#[cfg(target_os = "macos")]
use crate::platform::{macos, run_macos_window_op};
#[cfg(target_os = "windows")]
use crate::platform::{window_hwnd_isize, windows};
use crate::runtime::GlobalControlState;
use tauri::Manager;

use super::effects::apply_note_window_frost_by_label;

/// `DESK_TIDY_LAYER_DEBUG=1` prints every note-window layer and input-state transition to
/// stderr. Layer bugs are invisible otherwise: the Win32 calls report success while the
/// window ends up somewhere else, so the actual parent and styles are what get logged.
pub(super) fn layer_debug(message: impl FnOnce() -> String) {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let enabled = *ENABLED
        .get_or_init(|| std::env::var_os("DESK_TIDY_LAYER_DEBUG").is_some_and(|v| v == "1"));
    if enabled {
        eprintln!("[layer-debug] {}", message());
    }
}

#[cfg(target_os = "windows")]
fn describe_note_window(w: &tauri::WebviewWindow) -> String {
    match window_hwnd_isize(w) {
        Ok(Some(hwnd)) => windows::describe_window(hwnd),
        Ok(None) => "hwnd unavailable".to_string(),
        Err(error) => format!("hwnd error: {error}"),
    }
}

pub fn apply_overlay_input_state(app: &tauri::AppHandle, interaction_disabled: bool) {
    let notes =
        notes_store::with_notes_store(app, || notes_service::load_notes(NoteSortMode::Custom))
            .unwrap_or_else(|error| {
                eprintln!(
                    "[notes_storage] cannot apply overlay input state: {}",
                    error
                );
                Vec::new()
            });
    layer_debug(|| {
        let labels: Vec<String> = app.webview_windows().into_keys().collect();
        format!(
            "apply_overlay_input_state interaction_disabled={interaction_disabled} notes={} windows={labels:?}",
            notes.len()
        )
    });
    for (label, w) in app.webview_windows() {
        if label.starts_with("note-") {
            let note_id = label.trim_start_matches("note-");
            if let Some(n) = notes.iter().find(|x| x.id == note_id) {
                let ignore_cursor = resolve_note_ignore_cursor(
                    n.is_always_on_top,
                    n.is_wallpaper,
                    interaction_disabled,
                );
                layer_debug(|| {
                    format!(
                        "  {label}: top={} wallpaper={} -> ignore_cursor={ignore_cursor}",
                        n.is_always_on_top, n.is_wallpaper
                    )
                });
                if let Err(error) = w.set_ignore_cursor_events(ignore_cursor) {
                    eprintln!("[layer] {label}: set_ignore_cursor_events failed: {error}");
                }
                if let Err(error) = apply_note_window_layer_with_interaction_by_label(
                    app,
                    &label,
                    n.is_always_on_top,
                    interaction_disabled,
                    n.is_wallpaper,
                ) {
                    eprintln!("[layer] {label}: apply layer failed: {error}");
                }
                let _ = apply_note_window_frost_by_label(app, &label, n.frost.unwrap_or_default());
            } else {
                layer_debug(|| format!("  {label}: no matching note"));
                if let Err(error) = w.set_ignore_cursor_events(interaction_disabled) {
                    eprintln!("[layer] {label}: set_ignore_cursor_events failed: {error}");
                }
            }
        }
    }
}

pub(super) fn apply_note_window_layer_with_interaction_by_label(
    app: &tauri::AppHandle,
    label: &str,
    is_always_on_top: bool,
    interaction_disabled: bool,
    is_wallpaper: bool,
) -> Result<(), String> {
    let Some(w) = app.get_webview_window(label) else {
        layer_debug(|| format!("apply layer {label}: no webview window"));
        return Ok(());
    };
    let force_global_top = !interaction_disabled;

    #[cfg(target_os = "windows")]
    {
        let Some(hwnd_isize) = window_hwnd_isize(&w)? else {
            layer_debug(|| format!("apply layer {label}: hwnd unavailable"));
            return Ok(());
        };
        let branch = if force_global_top || is_always_on_top {
            "topmost"
        } else if is_wallpaper {
            "wallpaper"
        } else {
            "desktop"
        };
        layer_debug(|| {
            format!(
                "apply layer {label} branch={branch} (top={is_always_on_top} wallpaper={is_wallpaper} interaction_disabled={interaction_disabled}) before: {}",
                describe_note_window(&w)
            )
        });
        let result = apply_windows_layer(&w, hwnd_isize, branch);
        layer_debug(|| {
            format!(
                "apply layer {label} branch={branch} result={result:?} after: {}",
                describe_note_window(&w)
            )
        });
        return result;
    }

    #[cfg(target_os = "macos")]
    {
        if force_global_top || is_always_on_top {
            run_macos_window_op(&w, "macos_set_topmost_true", |ptr| {
                macos::set_topmost_no_activate(ptr, true)
            })?;
        } else {
            run_macos_window_op(&w, "macos_set_topmost_false", |ptr| {
                macos::set_topmost_no_activate(ptr, false)
            })?;
            if is_wallpaper {
                run_macos_window_op(&w, "macos_attach_to_wallpaper_layer", move |ptr| {
                    // Wallpaper mode should stay behind icons.
                    macos::attach_to_wallpaper_layer_with_interaction(ptr, true)
                })?;
            } else {
                let ignore_cursor = resolve_note_ignore_cursor(
                    is_always_on_top,
                    is_wallpaper,
                    interaction_disabled,
                );
                run_macos_window_op(&w, "macos_attach_to_desktop_layer", move |ptr| {
                    macos::attach_to_desktop_layer_with_interaction(ptr, ignore_cursor)
                })?;
            }
        }
        return Ok(());
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        let _ = is_wallpaper;
        let _ = interaction_disabled;
        let _ = w.set_always_on_top(force_global_top || is_always_on_top);
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn apply_windows_layer(
    w: &tauri::WebviewWindow,
    hwnd_isize: isize,
    branch: &str,
) -> Result<(), String> {
    match branch {
        "topmost" => {
            windows::detach_from_worker_w(hwnd_isize)?;
            let _ = w.set_always_on_top(true);
            windows::set_topmost_no_activate(hwnd_isize, true)?;
        }
        "wallpaper" => {
            let _ = w.set_always_on_top(false);
            windows::set_topmost_no_activate(hwnd_isize, false)?;
            windows::attach_to_wallpaper_worker_w(hwnd_isize)?;
        }
        _ => {
            let _ = w.set_always_on_top(false);
            windows::set_topmost_no_activate(hwnd_isize, false)?;
            windows::attach_to_worker_w(hwnd_isize)?;
        }
    }
    Ok(())
}

fn resolve_note_ignore_cursor(
    is_always_on_top: bool,
    is_wallpaper: bool,
    interaction_disabled: bool,
) -> bool {
    if !interaction_disabled {
        return false;
    }
    if is_always_on_top {
        return false;
    }
    if is_wallpaper {
        return true;
    }
    true
}

pub(super) fn get_overlay_interaction_disabled(app: &tauri::AppHandle) -> bool {
    if let Some(state) = app.try_state::<GlobalControlState>() {
        if let Ok(guard) = state.0.lock() {
            return *guard;
        }
    }
    true
}
