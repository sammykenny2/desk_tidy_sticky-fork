use crate::desktop::{apply_overlay_input_state, show_preferred_panel_window};
use crate::runtime::GlobalControlState;
use std::collections::HashMap;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};

/// Tray menu labels. The defaults show until the frontend sends its locale's texts.
#[derive(Clone)]
pub(super) struct TrayLabels {
    pub show: String,
    pub github: String,
    pub toggle_stickies: String,
    pub toggle_interaction: String,
    pub quit: String,
}

impl Default for TrayLabels {
    fn default() -> Self {
        Self {
            show: "Show main window".to_string(),
            github: "Star on GitHub".to_string(),
            toggle_stickies: "Stickers: Close".to_string(),
            toggle_interaction: "Stickers: Toggle Global Control".to_string(),
            quit: "Exit".to_string(),
        }
    }
}

impl TrayLabels {
    /// Takes the labels present in the frontend's text table.
    fn apply_texts(&mut self, texts: &HashMap<String, String>) {
        let take = |label: &mut String, text: Option<&String>| {
            if let Some(text) = text {
                *label = text.clone();
            }
        };
        take(
            &mut self.show,
            texts
                .get("trayShowMain")
                .or_else(|| texts.get("trayShowNotes")),
        );
        take(&mut self.github, texts.get("trayGithub"));
        // "Show" wins when both stickies texts are sent.
        take(
            &mut self.toggle_stickies,
            texts
                .get("trayStickiesShow")
                .or_else(|| texts.get("trayStickiesClose")),
        );
        take(&mut self.toggle_interaction, texts.get("trayInteraction"));
        take(&mut self.quit, texts.get("trayQuit"));
    }
}

struct TrayMenuState {
    show: MenuItem<tauri::Wry>,
    github: MenuItem<tauri::Wry>,
    toggle_stickies: MenuItem<tauri::Wry>,
    toggle_interaction: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}

impl TrayMenuState {
    fn labels(&self) -> TrayLabels {
        let text = |item: &MenuItem<tauri::Wry>| item.text().unwrap_or_default();
        TrayLabels {
            show: text(&self.show),
            github: text(&self.github),
            toggle_stickies: text(&self.toggle_stickies),
            toggle_interaction: text(&self.toggle_interaction),
            quit: text(&self.quit),
        }
    }

    fn set_labels(&self, labels: &TrayLabels) {
        let _ = self.show.set_text(&labels.show);
        let _ = self.github.set_text(&labels.github);
        let _ = self.toggle_stickies.set_text(&labels.toggle_stickies);
        let _ = self.toggle_interaction.set_text(&labels.toggle_interaction);
        let _ = self.quit.set_text(&labels.quit);
    }
}

/// Runs a tray menu item by id; shared by Tauri's tray and the Linux one.
pub(super) fn run_tray_action(app: &tauri::AppHandle, id: &str) {
    match id {
        "show" => show_preferred_panel_window(app),
        "github" => {
            let _ = open::that("https://github.com/sqmw/desk_tidy_sticky");
        }
        "toggle_stickies" => {
            let _ = app.emit("tray_overlay_toggle", ());
        }
        "toggle_interaction" => {
            if let Some(state) = app.try_state::<GlobalControlState>() {
                let interaction_disabled = state.toggle();
                apply_overlay_input_state(app, interaction_disabled);
                let _ = app.emit("global_control_changed", interaction_disabled);
            }
        }
        "quit" => app.exit(0),
        _ => {}
    }
}

#[tauri::command]
pub fn update_tray_texts(
    app: tauri::AppHandle,
    texts: HashMap<String, String>,
) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    super::tray_linux::update_labels(&app, |labels| labels.apply_texts(&texts));
    if let Some(state) = app.try_state::<TrayMenuState>() {
        let mut labels = state.labels();
        labels.apply_texts(&texts);
        state.set_labels(&labels);
    }
    Ok(())
}

pub fn build_tray(app: &mut tauri::App) -> tauri::Result<()> {
    // libayatana's tray cannot show on wf-panel-pi; see `tray_linux`.
    #[cfg(target_os = "linux")]
    if super::tray_linux::build(app.handle()) {
        return Ok(());
    }

    let labels = TrayLabels::default();
    let show_i = MenuItem::with_id(app, "show", &labels.show, true, None::<&str>)?;
    let github_i = MenuItem::with_id(app, "github", &labels.github, true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let toggle_stickies_i = MenuItem::with_id(
        app,
        "toggle_stickies",
        &labels.toggle_stickies,
        true,
        None::<&str>,
    )?;
    let toggle_interaction_i = MenuItem::with_id(
        app,
        "toggle_interaction",
        &labels.toggle_interaction,
        true,
        None::<&str>,
    )?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit_i = MenuItem::with_id(app, "quit", &labels.quit, true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &show_i,
            &github_i,
            &sep1,
            &toggle_stickies_i,
            &toggle_interaction_i,
            &sep2,
            &quit_i,
        ],
    )?;

    app.manage(TrayMenuState {
        show: show_i,
        github: github_i,
        toggle_stickies: toggle_stickies_i,
        toggle_interaction: toggle_interaction_i,
        quit: quit_i,
    });

    #[cfg(target_os = "macos")]
    let tray_icon = Image::from_bytes(include_bytes!("../../icons/tray-template.png"))?;
    #[cfg(not(target_os = "macos"))]
    let tray_icon = Image::from_bytes(include_bytes!("../../icons/tray-color.png"))?;

    let tray_builder = TrayIconBuilder::new()
        .icon(tray_icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                run_tray_action(tray.app_handle(), "show");
            }
        })
        .on_menu_event(|app, event| run_tray_action(app, event.id.as_ref()));

    #[cfg(target_os = "macos")]
    let tray_builder = tray_builder.icon_as_template(true);
    #[cfg(not(target_os = "macos"))]
    let tray_builder = tray_builder.icon_as_template(false);

    let _tray = tray_builder.build(app)?;
    Ok(())
}
