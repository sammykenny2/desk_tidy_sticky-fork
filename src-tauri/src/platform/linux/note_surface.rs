//! Sticky note windows as wlr-layer-shell surfaces.
//!
//! A Wayland toplevel can neither place itself nor choose a stacking layer, so on a
//! compositor that offers wlr-layer-shell (labwc on Raspberry Pi OS, Wayfire, sway, KDE)
//! every note window becomes a layer surface anchored to the top-left corner of its
//! output, and its margins are its position:
//!
//! - Bottom: above the desktop icons (pcmanfm draws them on the Background layer) and
//!   below application windows. This is the desktop layer.
//! - Background: the wallpaper layer. pcmanfm draws the wallpaper and the icons on one
//!   surface, so a note cannot go under the icons; it lands above them like Bottom.
//! - Top: above application windows. Topmost notes and global operation use it.
//!
//! A layer surface takes its size from the window's size request, not from
//! `gtk_window_resize`, and a WebKit view requests no size of its own: without an
//! explicit request the surface collapses to nothing and is never drawn. Toolkit
//! position and frame queries return zeros for a layer surface, so the logical
//! geometry this module sets is tracked in `LinuxNoteSurfaceState`.
//!
//! Without layer-shell (an X11 session, GNOME) notes stay regular windows and the generic
//! `set_always_on_top` path in `desktop/sticky/layer.rs` applies.

use std::collections::HashMap;
use std::sync::Mutex;

use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use tauri::Manager;

use crate::desktop::layer_debug;

const NAMESPACE: &str = "desk-tidy-sticky";

/// After a note surface is moved, resized or put on another layer, WebKit repaints only
/// the regions it considers damaged (the text block), and the rest of the note keeps a
/// stale shade until the window is recreated. Flipping the root opacity damages the
/// whole page, so the next frame is a full repaint.
const REPAINT_SCRIPT: &str = "(() => { const s = document.documentElement.style; \
    s.opacity = '0.999'; \
    requestAnimationFrame(() => requestAnimationFrame(() => { s.opacity = ''; })); })()";
/// A drag reconfigures the surface on every pointer move; repaint once it settles.
const REPAINT_DELAY: std::time::Duration = std::time::Duration::from_millis(120);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoteLayer {
    Wallpaper,
    Desktop,
    Topmost,
}

impl NoteLayer {
    fn gtk_layer(self) -> Layer {
        match self {
            NoteLayer::Wallpaper => Layer::Background,
            NoteLayer::Desktop => Layer::Bottom,
            NoteLayer::Topmost => Layer::Top,
        }
    }
}

/// Logical desktop geometry of a note layer surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SurfaceGeometry {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug)]
struct SurfaceEntry {
    geometry: SurfaceGeometry,
    /// Bumped on every reconfigure, so only the last one in a burst repaints.
    repaint_generation: u64,
}

/// Geometry of every note window that is a layer surface, keyed by window label.
#[derive(Default)]
pub struct LinuxNoteSurfaceState(Mutex<HashMap<String, SurfaceEntry>>);

impl LinuxNoteSurfaceState {
    fn get(&self, label: &str) -> Option<SurfaceGeometry> {
        Some(self.0.lock().ok()?.get(label)?.geometry)
    }

    fn update(&self, label: &str, apply: impl FnOnce(&mut SurfaceGeometry)) {
        if let Ok(mut guard) = self.0.lock() {
            if let Some(entry) = guard.get_mut(label) {
                apply(&mut entry.geometry);
            }
        }
    }

    fn insert(&self, label: &str, geometry: SurfaceGeometry) {
        if let Ok(mut guard) = self.0.lock() {
            let entry = SurfaceEntry {
                geometry,
                repaint_generation: 0,
            };
            guard.insert(label.to_string(), entry);
        }
    }

    fn next_repaint_generation(&self, label: &str) -> Option<u64> {
        let mut guard = self.0.lock().ok()?;
        let entry = guard.get_mut(label)?;
        entry.repaint_generation += 1;
        Some(entry.repaint_generation)
    }

    fn repaint_generation(&self, label: &str) -> Option<u64> {
        Some(self.0.lock().ok()?.get(label)?.repaint_generation)
    }

    pub fn forget(&self, label: &str) {
        if let Ok(mut guard) = self.0.lock() {
            guard.remove(label);
        }
    }
}

/// `DESK_TIDY_LAYER_SHELL=0` keeps notes as regular windows even when layer-shell exists.
fn layer_shell_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        let opted_out = std::env::var_os("DESK_TIDY_LAYER_SHELL").is_some_and(|v| v == "0");
        // Must run on the GTK main thread; the first caller is `init_note_surface`.
        !opted_out && gtk_layer_shell::is_supported()
    })
}

fn surface_state(window: &tauri::WebviewWindow) -> Option<tauri::State<'_, LinuxNoteSurfaceState>> {
    window.try_state::<LinuxNoteSurfaceState>()
}

/// The note's logical desktop geometry when it is a layer surface.
pub(crate) fn note_surface_geometry(window: &tauri::WebviewWindow) -> Option<SurfaceGeometry> {
    surface_state(window)?.get(window.label())
}

pub(crate) fn is_note_surface(window: &tauri::WebviewWindow) -> bool {
    note_surface_geometry(window).is_some()
}

/// Runs `op` with the window's GtkWindow on the GTK main thread.
fn with_gtk_window(
    window: &tauri::WebviewWindow,
    op_name: &'static str,
    op: impl FnOnce(&gtk::ApplicationWindow) + Send + 'static,
) -> Result<(), String> {
    if gtk::is_initialized_main_thread() {
        let gtk_window = window.gtk_window().map_err(|e| e.to_string())?;
        op(&gtk_window);
        return Ok(());
    }
    let target = window.clone();
    window
        .run_on_main_thread(move || match target.gtk_window() {
            Ok(gtk_window) => op(&gtk_window),
            Err(error) => eprintln!("{op_name} failed: {error}"),
        })
        .map_err(|e| e.to_string())
}

/// The monitor that contains the logical point, or the first one.
fn monitor_for_point(display: &gtk::gdk::Display, x: i32, y: i32) -> Option<gtk::gdk::Monitor> {
    (0..display.n_monitors())
        .filter_map(|index| display.monitor(index))
        .find(|monitor| {
            let g = monitor.geometry();
            x >= g.x() && x < g.x() + g.width() && y >= g.y() && y < g.y() + g.height()
        })
        .or_else(|| display.monitor(0))
}

fn place(gtk_window: &gtk::ApplicationWindow, x: f64, y: f64) {
    let (x, y) = (x.round() as i32, y.round() as i32);
    let display = gtk_window.display();
    let Some(monitor) = monitor_for_point(&display, x, y) else {
        return;
    };
    let geometry = monitor.geometry();
    gtk_window.set_monitor(&monitor);
    gtk_window.set_layer_shell_margin(Edge::Left, x - geometry.x());
    gtk_window.set_layer_shell_margin(Edge::Top, y - geometry.y());
}

fn default_position(gtk_window: &gtk::ApplicationWindow, width: i32, height: i32) -> (f64, f64) {
    let display = gtk_window.display();
    let Some(monitor) = display.primary_monitor().or_else(|| display.monitor(0)) else {
        return (80.0, 80.0);
    };
    let g = monitor.geometry();
    (
        f64::from(g.x() + (g.width() - width).max(0) / 2),
        f64::from(g.y() + (g.height() - height).max(0) / 2),
    )
}

/// Turns a freshly created, still unrealized note window into a layer surface on the
/// desktop layer. Returns false when layer-shell is unavailable and the window stays a
/// regular toplevel.
pub(crate) fn init_note_surface(
    window: &tauri::WebviewWindow,
    position: Option<(f64, f64)>,
) -> Result<bool, String> {
    if !gtk::is_initialized_main_thread() {
        return Err("init_note_surface must run on the GTK main thread".to_string());
    }
    if !layer_shell_enabled() {
        return Ok(false);
    }
    let gtk_window = window.gtk_window().map_err(|e| e.to_string())?;
    if gtk_window.is_layer_window() {
        return Ok(true);
    }
    if gtk_window.is_realized() {
        return Err(format!(
            "{} was realized before layer-shell setup",
            window.label()
        ));
    }
    // The size the window was created with.
    let (width, height) = gtk_window.size();
    let (x, y) = position.unwrap_or_else(|| default_position(&gtk_window, width, height));
    gtk_window.init_layer_shell();
    gtk_window.set_namespace(NAMESPACE);
    gtk_window.set_layer(Layer::Bottom);
    gtk_window.set_anchor(Edge::Left, true);
    gtk_window.set_anchor(Edge::Top, true);
    // Position relative to the output edges, ignoring panels' exclusive zones, so the
    // margins equal the stored desktop coordinates.
    gtk_window.set_exclusive_zone(-1);
    gtk_window.set_keyboard_mode(KeyboardMode::OnDemand);
    gtk_window.set_size_request(width, height);
    place(&gtk_window, x, y);
    // tao unwraps the native window when it changes the input region, which notes do
    // while still hidden; realize it now that the layer-shell role is set up.
    gtk_window.realize();
    if let Some(state) = surface_state(window) {
        state.insert(
            window.label(),
            SurfaceGeometry {
                x,
                y,
                width: f64::from(width),
                height: f64::from(height),
            },
        );
    }
    Ok(true)
}

fn schedule_repaint(window: &tauri::WebviewWindow) {
    let Some(generation) =
        surface_state(window).and_then(|state| state.next_repaint_generation(window.label()))
    else {
        return;
    };
    let target = window.clone();
    let _ = with_gtk_window(window, "schedule_repaint", move |_| {
        gtk::glib::timeout_add_local_once(REPAINT_DELAY, move || {
            let current =
                surface_state(&target).and_then(|state| state.repaint_generation(target.label()));
            if current == Some(generation) {
                if let Err(error) = target.eval(REPAINT_SCRIPT) {
                    eprintln!("[linux] {}: repaint failed: {error}", target.label());
                }
            }
        });
    });
}

/// Makes a note window let the cursor through to what is below it, or take it again.
///
/// tao sets the input region on the GdkWindow directly. On Wayland tao gives every window
/// a header bar, which makes GTK treat it as client-side decorated, and GTK then rebuilds
/// the GdkWindow input region from the widget's own input shape on every size allocation,
/// which moving a layer surface or changing its layer triggers. The note would start
/// catching the cursor again; an input shape set on the widget survives the rebuild.
/// GDK only sends a new input region with the window's next frame, so queue one.
pub(crate) fn set_note_ignore_cursor(
    window: &tauri::WebviewWindow,
    ignore: bool,
) -> Result<(), String> {
    with_gtk_window(window, "set_note_ignore_cursor", move |gtk_window| {
        if ignore {
            gtk_window.input_shape_combine_region(Some(&gtk::cairo::Region::create()));
        } else {
            gtk_window.input_shape_combine_region(None);
        }
        gtk_window.queue_draw();
    })
}

pub(crate) fn set_note_surface_layer(
    window: &tauri::WebviewWindow,
    layer: NoteLayer,
) -> Result<(), String> {
    with_gtk_window(window, "set_note_surface_layer", move |gtk_window| {
        if gtk_window.is_layer_window() {
            gtk_window.set_layer(layer.gtk_layer());
        }
    })?;
    schedule_repaint(window);
    Ok(())
}

pub(crate) fn move_note_surface(
    window: &tauri::WebviewWindow,
    x: f64,
    y: f64,
) -> Result<(), String> {
    layer_debug(|| format!("move {} to ({x}, {y})", window.label()));
    if let Some(state) = surface_state(window) {
        state.update(window.label(), |geometry| {
            geometry.x = x;
            geometry.y = y;
        });
    }
    with_gtk_window(window, "move_note_surface", move |gtk_window| {
        if gtk_window.is_layer_window() {
            place(gtk_window, x, y);
        }
    })?;
    schedule_repaint(window);
    Ok(())
}

pub(crate) fn resize_note_surface(
    window: &tauri::WebviewWindow,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let (width, height) = (width.round().max(1.0), height.round().max(1.0));
    layer_debug(|| format!("resize {} to {width}x{height}", window.label()));
    if let Some(state) = surface_state(window) {
        state.update(window.label(), |geometry| {
            geometry.width = width;
            geometry.height = height;
        });
    }
    with_gtk_window(window, "resize_note_surface", move |gtk_window| {
        if gtk_window.is_layer_window() {
            gtk_window.set_size_request(width as i32, height as i32);
        }
    })?;
    schedule_repaint(window);
    Ok(())
}
