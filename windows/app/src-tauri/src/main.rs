//! QC Buddy tray app: a tray icon and a flyout window above it. All headset
//! work happens in qcbuddy-core's worker thread; the flyout sends it commands
//! and renders the `state` events it emits.
//!
//! Set QCBUDDY_FAKE=1 to run against the simulated QC45 (no Bluetooth).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;
use std::time::{Duration, Instant};

use qcbuddy_core::fake::FakeHeadset;
use qcbuddy_core::log::{default_path, Log};
use qcbuddy_core::platform::Platform;
use qcbuddy_core::{Command, Controller, LinkState, State, Worker};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewWindow, WindowEvent};

const FLYOUT: &str = "flyout";
const TRAY: &str = "main";
const WIDTH: f64 = 372.0;
/// Gap between the flyout and the screen edge / taskbar, like Windows 11 flyouts.
const MARGIN: f64 = 12.0;

struct AppState {
    worker: Worker,
    latest: Mutex<Option<State>>,
    /// When the flyout last hid on losing focus. A tray click that arrives
    /// right after is the same click, so it shouldn't reopen the flyout.
    hidden_at: Mutex<Option<Instant>>,
}

#[tauri::command]
fn get_state(app: tauri::State<'_, AppState>) -> Option<State> {
    app.latest.lock().ok()?.clone()
}

#[tauri::command]
fn send(command: Command, app: tauri::State<'_, AppState>) {
    app.worker.send(command);
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

fn main() {
    tauri::Builder::default()
        // Must be first: a second launch just opens the running app's flyout.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show(app)))
        .invoke_handler(tauri::generate_handler![get_state, send, hide_flyout, fit_flyout])
        .setup(|app| {
            let handle = app.handle().clone();
            let controller = Controller::new(platform(), Log::file(default_path()));
            let emitter = handle.clone();
            let worker = Worker::spawn(controller, move |state: &State| publish(&emitter, state));
            app.manage(AppState { worker, latest: Mutex::new(None), hidden_at: Mutex::new(None) });

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
            handle.state::<AppState>().worker.send(Command::Start);
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

/// Called on the worker thread with every state change.
fn publish(app: &AppHandle, state: &State) {
    if let Some(shared) = app.try_state::<AppState>() {
        if let Ok(mut latest) = shared.latest.lock() {
            *latest = Some(state.clone());
        }
    }
    let _ = app.emit("state", state);
    if let Some(tray) = app.tray_by_id(TRAY) {
        let _ = tray.set_tooltip(Some(tooltip(state)));
    }
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
    let icon = if light_taskbar() {
        Image::from_bytes(include_bytes!("../icons/tray-dark.png"))?
    } else {
        Image::from_bytes(include_bytes!("../icons/tray-light.png"))?
    };
    TrayIconBuilder::with_id(TRAY)
        .icon(icon)
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
        .is_some_and(|t| t.elapsed() < Duration::from_millis(300));
    if !just_hidden {
        show(app);
    }
}

fn show(app: &AppHandle) {
    let Some(window) = app.get_webview_window(FLYOUT) else { return };
    place(&window);
    let _ = window.show();
    let _ = window.set_focus();
    app.state::<AppState>().worker.send(Command::FlyoutOpened);
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
    state.worker.send(Command::FlyoutClosed);
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
    let margin = (MARGIN * monitor.scale_factor()).round() as i32;
    let x = area.position.x + area.size.width as i32 - size.width as i32 - margin;
    let y = area.position.y + area.size.height as i32 - size.height as i32 - margin;
    let _ = window.set_position(PhysicalPosition::new(x.max(area.position.x), y.max(area.position.y)));
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
