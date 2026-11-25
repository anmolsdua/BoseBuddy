//! QC Buddy tray app: a tray icon and a flyout window above it. All headset
//! work happens in qcbuddy-core's worker thread; the flyout sends it commands
//! and renders the `state` events it emits.
//!
//! Set QCBUDDY_FAKE=1 to run against the simulated QC45 (no Bluetooth).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use qcbuddy_core::eq_presets::EqPreset;
use qcbuddy_core::fake::FakeHeadset;
use qcbuddy_core::log::{data_dir, default_path, Log};
use qcbuddy_core::platform::Platform;
use qcbuddy_core::{Command, Controller, LinkState, State, Worker};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewWindow, WindowEvent};

mod presets;
mod snapshot;

use snapshot::{Latest, Snapshot};

const FLYOUT: &str = "flyout";
const TRAY: &str = "main";
const WIDTH: f64 = 372.0;
/// Gap between the flyout and the screen edge / taskbar, like Windows 11 flyouts.
const MARGIN: f64 = 12.0;

struct AppState {
    worker: Worker,
    latest: Arc<Latest>,
    /// When the flyout last hid on losing focus. A tray click that arrives
    /// right after is the same click, so it shouldn't reopen the flyout.
    hidden_at: Mutex<Option<Instant>>,
    /// The taskbar is light, so tray icons are drawn dark.
    light_taskbar: bool,
    /// Battery shown on the tray icon (None = headphones glyph), to redraw only on change.
    tray_battery: Mutex<Option<Option<u8>>>,
}

/// The newest snapshot. The flyout calls this after subscribing to `state`.
#[tauri::command]
fn get_state(app: tauri::State<'_, AppState>) -> Option<Snapshot> {
    app.latest.get()
}

/// Queue a command; the flyout shows the error if the worker is busy or stopped.
#[tauri::command]
fn send(command: Command, app: tauri::State<'_, AppState>) -> Result<(), String> {
    app.worker.send(command).map_err(|e| e.to_string())
}

#[tauri::command]
fn hide_flyout(app: AppHandle) {
    hide(&app);
}

/// The UI reports its content height; the window takes it, anchored above the tray.
#[tauri::command]
fn fit_flyout(height: f64, window: WebviewWindow) {
    let max = window
        .current_monitor()
        .ok()
        .flatten()
        .map(|m| m.work_area().size.height as f64 / m.scale_factor() - 2.0 * MARGIN)
        .unwrap_or(800.0);
    let _ = window.set_size(LogicalSize::new(WIDTH, height.clamp(120.0, max)));
    place(&window);
}

// EQ presets: the same file as the CLI (see presets.rs). The simulated
// headset gets its own file.

fn presets_path() -> std::path::PathBuf {
    let file = if std::env::var_os("QCBUDDY_FAKE").is_some() { "eq-presets-fake.json" } else { "eq-presets.json" };
    data_dir().join(file)
}

#[tauri::command]
fn presets() -> Vec<EqPreset> {
    presets::list(&presets_path())
}

#[tauri::command]
fn save_preset(name: String, values: Vec<i32>) -> Result<Vec<EqPreset>, String> {
    presets::save(&presets_path(), &name, &values)
}

#[tauri::command]
fn rename_preset(id: String, name: String) -> Result<Vec<EqPreset>, String> {
    presets::rename(&presets_path(), &id, &name)
}

#[tauri::command]
fn delete_preset(id: String) -> Vec<EqPreset> {
    presets::delete(&presets_path(), &id)
}

fn main() {
    tauri::Builder::default()
        // Must be first: a second launch just opens the running app's flyout.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show(app)))
        .invoke_handler(tauri::generate_handler![
            get_state,
            send,
            hide_flyout,
            fit_flyout,
            presets,
            save_preset,
            rename_preset,
            delete_preset
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let controller = Controller::new(platform(), Log::file(default_path()));
            let emitter = handle.clone();
            let latest = Arc::new(Latest::default());
            let recorder = latest.clone();
            let worker = Worker::spawn(controller, move |state: &State| publish(&emitter, &recorder, state));
            app.manage(AppState {
                worker,
                latest,
                hidden_at: Mutex::new(None),
                light_taskbar: light_taskbar(),
                tray_battery: Mutex::new(None),
            });

            build_tray(&handle)?;
            if let Some(window) = handle.get_webview_window(FLYOUT) {
                let app = handle.clone();
                window.on_window_event(move |event| match event {
                    WindowEvent::Focused(false) => hide(&app),
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        hide(&app);
                    }
                    _ => {}
                });
            }
            // A fresh worker's queue is empty, so this can't be refused.
            let _ = handle.state::<AppState>().worker.send(Command::Start);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running QC Buddy");
}

fn platform() -> Box<dyn Platform> {
    if std::env::var_os("QCBUDDY_FAKE").is_some() {
        return Box::new(FakeHeadset::default());
    }
    #[cfg(windows)]
    {
        Box::new(qcbuddy_core::platform::WindowsPlatform)
    }
    #[cfg(not(windows))]
    {
        Box::new(FakeHeadset::default())
    }
}

/// Called on the worker thread with every state change: record it as the
/// next revision, then emit it, then update the tray.
fn publish(app: &AppHandle, latest: &Latest, state: &State) {
    let _ = app.emit("state", latest.record(state));
    let Some(shared) = app.try_state::<AppState>() else { return };
    let Some(tray) = app.tray_by_id(TRAY) else { return };
    let _ = tray.set_tooltip(Some(tooltip(state)));
    let battery = if state.link.is_connected() { state.battery } else { None };
    if let Ok(mut shown) = shared.tray_battery.lock() {
        if *shown != Some(battery) {
            *shown = Some(battery);
            let _ = tray.set_icon(Some(tray_icon(battery, shared.light_taskbar)));
        }
    };
}

/// The battery level as digits, or the headphones glyph when unknown.
fn tray_icon(battery: Option<u8>, light_taskbar: bool) -> Image<'static> {
    match battery {
        Some(level) => {
            let color = if level <= 20 {
                [0xE8, 0x11, 0x23, 0xFF]
            } else if light_taskbar {
                [0x1B, 0x1B, 0x1B, 0xFF]
            } else {
                [0xFF, 0xFF, 0xFF, 0xFF]
            };
            Image::new_owned(battery_digits(level, color), ICON_SIZE as u32, ICON_SIZE as u32)
        }
        None if light_taskbar => Image::from_bytes(include_bytes!("../icons/tray-dark.png")).expect("tray icon"),
        None => Image::from_bytes(include_bytes!("../icons/tray-light.png")).expect("tray icon"),
    }
}

const ICON_SIZE: usize = 32;

/// 32×32 RGBA with `level` drawn in a 3×5 pixel font, scaled to fill the
/// icon (×5 for two digits, ×3 for "100"), so it stays sharp at 16 px.
fn battery_digits(level: u8, color: [u8; 4]) -> Vec<u8> {
    const FONT: [[u8; 5]; 10] = [
        [0b111, 0b101, 0b101, 0b101, 0b111],
        [0b010, 0b110, 0b010, 0b010, 0b111],
        [0b111, 0b001, 0b111, 0b100, 0b111],
        [0b111, 0b001, 0b111, 0b001, 0b111],
        [0b101, 0b101, 0b111, 0b001, 0b001],
        [0b111, 0b100, 0b111, 0b001, 0b111],
        [0b111, 0b100, 0b111, 0b101, 0b111],
        [0b111, 0b001, 0b001, 0b001, 0b001],
        [0b111, 0b101, 0b111, 0b101, 0b111],
        [0b111, 0b101, 0b111, 0b001, 0b111],
    ];
    let digits: Vec<usize> = level.min(100).to_string().bytes().map(|b| (b - b'0') as usize).collect();
    let (scale, gap) = if digits.len() == 3 { (3, 2) } else { (5, 2) };
    let width = digits.len() * 3 * scale + (digits.len() - 1) * gap;
    let (x0, y0) = ((ICON_SIZE - width) / 2, (ICON_SIZE - 5 * scale) / 2);
    let mut rgba = vec![0u8; ICON_SIZE * ICON_SIZE * 4];
    for (i, digit) in digits.iter().enumerate() {
        let left = x0 + i * (3 * scale + gap);
        for (row, bits) in FONT[*digit].iter().enumerate() {
            for col in 0..3 {
                if bits & (0b100 >> col) == 0 {
                    continue;
                }
                for y in y0 + row * scale..y0 + (row + 1) * scale {
                    for x in left + col * scale..left + (col + 1) * scale {
                        let at = (y * ICON_SIZE + x) * 4;
                        rgba[at..at + 4].copy_from_slice(&color);
                    }
                }
            }
        }
    }
    rgba
}

fn tooltip(state: &State) -> String {
    match (&state.link, state.battery) {
        (LinkState::Connected, Some(battery)) => format!("QC Buddy · {} · {battery}%", state.device_name),
        (LinkState::Connected, None) => format!("QC Buddy · {}", state.device_name),
        (LinkState::Connecting, _) => "QC Buddy · connecting…".into(),
        _ => "QC Buddy · not connected".into(),
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open QC Buddy", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let light = app.state::<AppState>().light_taskbar;
    TrayIconBuilder::with_id(TRAY)
        .icon(tray_icon(None, light))
        .tooltip("QC Buddy")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                toggle(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn toggle(app: &AppHandle) {
    let Some(window) = app.get_webview_window(FLYOUT) else { return };
    if window.is_visible().unwrap_or(false) {
        hide(app);
        return;
    }
    // Clicking the tray icon while the flyout is open first takes focus away
    // (which hides it), then delivers the click: don't reopen it.
    let just_hidden = app
        .state::<AppState>()
        .hidden_at
        .lock()
        .ok()
        .and_then(|t| *t)
        .is_some_and(|t| is_same_click(t, Instant::now()));
    if !just_hidden {
        show(app);
    }
}

fn show(app: &AppHandle) {
    let Some(window) = app.get_webview_window(FLYOUT) else { return };
    place(&window);
    let _ = window.show();
    let _ = window.set_focus();
    app.state::<AppState>().worker.set_flyout_open(true);
}

fn hide(app: &AppHandle) {
    let Some(window) = app.get_webview_window(FLYOUT) else { return };
    if !window.is_visible().unwrap_or(false) {
        return;
    }
    let _ = window.hide();
    let state = app.state::<AppState>();
    if let Ok(mut hidden_at) = state.hidden_at.lock() {
        *hidden_at = Some(Instant::now());
    }
    state.worker.set_flyout_open(false);
}

/// Bottom-right of the work area of the monitor under the cursor (where the
/// tray icon was clicked), above the taskbar.
fn place(window: &WebviewWindow) {
    let monitor = window
        .cursor_position()
        .ok()
        .and_then(|p| window.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten());
    let (Some(monitor), Ok(size)) = (monitor, window.outer_size()) else { return };
    let area = monitor.work_area();
    let (x, y) = flyout_origin(
        (area.position.x, area.position.y, area.size.width, area.size.height),
        (size.width, size.height),
        monitor.scale_factor(),
    );
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

/// Top-left corner for the flyout: bottom-right of the work area (x, y, w, h,
/// physical pixels), MARGIN logical pixels in, never off its top-left.
fn flyout_origin(area: (i32, i32, u32, u32), size: (u32, u32), scale: f64) -> (i32, i32) {
    let (ax, ay, aw, ah) = area;
    let margin = (MARGIN * scale).round() as i32;
    let x = ax + aw as i32 - size.0 as i32 - margin;
    let y = ay + ah as i32 - size.1 as i32 - margin;
    (x.max(ax), y.max(ay))
}

/// A tray click this soon after the flyout hid on losing focus is the click
/// that took the focus: it should close the flyout, not reopen it.
fn is_same_click(hidden_at: Instant, now: Instant) -> bool {
    now.saturating_duration_since(hidden_at) < Duration::from_millis(300)
}

/// Windows' "SystemUsesLightTheme": the taskbar is light, so the tray icon must be dark.
#[cfg(windows)]
fn light_taskbar() -> bool {
    use windows::core::w;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    let mut value: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let rc = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut _),
            Some(&mut size),
        )
    };
    rc.is_ok() && value == 1
}

#[cfg(not(windows))]
fn light_taskbar() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use qcbuddy_core::fake::{FakeHeadset, FakeState};

    #[test]
    fn flyout_sits_above_the_taskbar_at_the_bottom_right() {
        // 1920x1080 at 100%, taskbar 48 px at the bottom.
        assert_eq!(flyout_origin((0, 0, 1920, 1032), (372, 500), 1.0), (1920 - 372 - 12, 1032 - 500 - 12));
        // 150%: the margin scales too.
        assert_eq!(flyout_origin((0, 0, 2880, 1548), (558, 750), 1.5), (2880 - 558 - 18, 1548 - 750 - 18));
        // Second monitor to the left (negative coordinates).
        assert_eq!(flyout_origin((-1920, 0, 1920, 1032), (372, 500), 1.0), (-372 - 12, 520));
        // Taller than the work area: pinned to its top, not off screen.
        assert_eq!(flyout_origin((0, 0, 1920, 600), (372, 900), 1.0), (1536, 0));
    }

    #[test]
    fn tray_click_right_after_hiding_is_the_same_click() {
        let hidden = Instant::now();
        assert!(is_same_click(hidden, hidden + Duration::from_millis(120)));
        assert!(!is_same_click(hidden, hidden + Duration::from_millis(400)));
        // A click stamped before the hide (clock order) counts as the same click; no panic.
        assert!(is_same_click(hidden + Duration::from_millis(5), hidden));
    }

    #[test]
    fn tooltips() {
        let mut c = Controller::new(Box::new(FakeHeadset::default()), Log::memory());
        assert_eq!(tooltip(c.state()), "QC Buddy · not connected");
        c.start();
        assert_eq!(tooltip(c.state()), "QC Buddy · Bose QC45 · 80%");
        let off = Controller::new(Box::new(FakeHeadset::new(FakeState { powered_off: true, ..FakeState::default() })), Log::memory());
        assert_eq!(tooltip(off.state()), "QC Buddy · not connected");
    }

    #[test]
    fn tray_icon_colors() {
        let color = |battery: Option<u8>, light: bool| {
            let icon = tray_icon(battery, light);
            let rgba = icon.rgba();
            let p = rgba.chunks(4).find(|p| p[3] > 200).expect("visible pixels");
            [p[0], p[1], p[2]]
        };
        assert_eq!(color(Some(80), false), [0xFF, 0xFF, 0xFF], "white digits on a dark taskbar");
        assert_eq!(color(Some(80), true), [0x1B, 0x1B, 0x1B], "dark digits on a light taskbar");
        assert_eq!(color(Some(20), false), [0xE8, 0x11, 0x23], "red at 20% or less");
        assert_eq!(color(Some(20), true), [0xE8, 0x11, 0x23]);
        // Headphones glyph when the battery is unknown, in the taskbar's contrast color.
        assert!(color(None, false).iter().all(|c| *c > 200));
        assert!(color(None, true).iter().all(|c| *c < 60));
    }

    fn lit(rgba: &[u8]) -> usize {
        rgba.chunks(4).filter(|p| p[3] != 0).count()
    }

    #[test]
    fn battery_digits_fit_the_icon() {
        for level in [0u8, 5, 42, 80, 99, 100, 255] {
            let rgba = battery_digits(level, [255; 4]);
            assert_eq!(rgba.len(), ICON_SIZE * ICON_SIZE * 4);
            assert!(lit(&rgba) > 0, "{level} draws something");
        }
        // "88" lights every segment of both digits: 2 digits x 13 cells x 5x5 px.
        assert_eq!(lit(&battery_digits(88, [255; 4])), 2 * 13 * 25);
    }
}
