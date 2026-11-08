//! The tray on Linux, published by the app itself as a StatusNotifierItem (ksni).
//!
//! Tauri's tray goes through libayatana-appindicator, which publishes the icon as an
//! absolute PNG path in `IconName` and sends no `IconPixmap`. wf-panel-pi (Raspberry Pi
//! OS) resolves only icon-theme names and pixmaps, so that tray never appears there.
//! Publishing the pixels, as Electron apps do, works there and with other hosts.

use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::StandardItem;
use tauri::Manager;

use super::tray::{run_tray_action, TrayLabels};

const ICON_PNG: &[u8] = include_bytes!("../../icons/tray-color.png");

struct StickyTray {
    app: tauri::AppHandle,
    icon: Vec<ksni::Icon>,
    labels: TrayLabels,
}

impl StickyTray {
    fn item(&self, label: &str, id: &'static str) -> ksni::MenuItem<Self> {
        StandardItem {
            label: label.to_string(),
            activate: Box::new(move |tray: &mut Self| run_tray_action(&tray.app, id)),
            ..Default::default()
        }
        .into()
    }
}

impl ksni::Tray for StickyTray {
    fn id(&self) -> String {
        "desk_tidy_sticky".to_string()
    }

    fn title(&self) -> String {
        "Desk Tidy Sticky".to_string()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.icon.clone()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        run_tray_action(&self.app, "show");
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        let labels = &self.labels;
        vec![
            self.item(&labels.show, "show"),
            self.item(&labels.github, "github"),
            ksni::MenuItem::Separator,
            self.item(&labels.toggle_stickies, "toggle_stickies"),
            self.item(&labels.toggle_interaction, "toggle_interaction"),
            ksni::MenuItem::Separator,
            self.item(&labels.quit, "quit"),
        ]
    }
}

struct LinuxTrayState(Handle<StickyTray>);

/// StatusNotifierItem pixmaps are ARGB32 in network byte order.
fn icon_pixmap() -> Result<ksni::Icon, String> {
    let image = tauri::image::Image::from_bytes(ICON_PNG).map_err(|e| e.to_string())?;
    let data = image
        .rgba()
        .chunks_exact(4)
        .flat_map(|px| [px[3], px[0], px[1], px[2]])
        .collect();
    Ok(ksni::Icon {
        width: image.width() as i32,
        height: image.height() as i32,
        data,
    })
}

/// Publishes the tray. Returns false when no StatusNotifierItem host is reachable, so
/// the caller can fall back to Tauri's tray.
pub(super) fn build(app: &tauri::AppHandle) -> bool {
    let icon = match icon_pixmap() {
        Ok(icon) => vec![icon],
        Err(error) => {
            eprintln!("[tray] cannot decode the tray icon: {error}");
            return false;
        }
    };
    let tray = StickyTray {
        app: app.clone(),
        icon,
        labels: TrayLabels::default(),
    };
    match tray.spawn() {
        Ok(handle) => {
            app.manage(LinuxTrayState(handle));
            true
        }
        Err(error) => {
            eprintln!("[tray] StatusNotifierItem unavailable, using the default tray: {error}");
            false
        }
    }
}

/// Applies label changes when this tray is the one in use.
pub(super) fn update_labels(app: &tauri::AppHandle, apply: impl FnOnce(&mut TrayLabels)) {
    if let Some(state) = app.try_state::<LinuxTrayState>() {
        state.0.update(|tray| apply(&mut tray.labels));
    }
}
