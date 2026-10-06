//! The Windows tray surface's composition root.
//!
//! Assembly only: claim the lock, find a hub, start the reader, run the loop.
//! Every decision it makes is a function somewhere else.
//!
//! Fail-open throughout. A surface that cannot lock, cannot find a hub, or
//! cannot register its notification identity still starts and paints its
//! offline state. The one outcome that must never happen is a tray icon that
//! is simply not there, because then the user has no way back in.

// No console window on Windows: this is a tray application, and a black box
// flashing up behind it at every login is the first thing a user would file.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::time::Duration;

use nerve_surface_core::locate::{FileProbe, HubLocator};
use nerve_surface_core::store::JobsStore;
use nerve_surface_core::stream::stream_path;
use nerve_windows_surface::app::App;
use nerve_windows_surface::platform::instance::{activate, claim, Claim, LOCK_PORT};
use nerve_windows_surface::platform::{aumid, paths};
use nerve_windows_surface::runtime::{Desktop, Runtime};

/// The label this surface attaches under. The hub treats it as a log tag.
const SURFACE: &str = "windows";

/// How long an autostarted copy waits before touching the shell.
///
/// At login every startup entry is competing for the same disk and the same
/// explorer, and a tray icon registered into that storm is the one most likely
/// to land in the overflow. Four seconds costs nothing and lands cleanly.
const LOGIN_SETTLE: Duration = Duration::from_secs(4);

fn main() {
    nerve_surface_core::log::init("nerve-windows");
    let autostarted = std::env::args().any(|arg| arg == "--autostart");
    if autostarted {
        std::thread::sleep(LOGIN_SETTLE);
    }

    let listener = match claim(LOCK_PORT) {
        Ok(Claim::Yield) => {
            if !autostarted {
                if let Err(error) = activate(LOCK_PORT) {
                    tracing::warn!(%error, "could not activate existing surface");
                }
            }
            tracing::info!("another tray surface already has this machine");
            eprintln!("nerve: another tray surface already has this machine");
            return;
        }
        Ok(Claim::Owned(listener)) => Some(listener),
        // Could not bind at all — a sandbox, most likely. Running unlocked is
        // worse than running locked, and better than not running.
        Err(error) => {
            tracing::warn!(%error, "surface lock failed");
            eprintln!("nerve: could not take the surface lock ({error}); carrying on");
            None
        }
    };

    // Registering the notification identity is idempotent and optional: without
    // it toasts do not appear, which is exactly what the default setting says
    // anyway.
    let icon = paths::install_dir().map(|dir| dir.join("nerve.ico"));
    aumid::register(icon.as_deref());

    let home = home_dir();
    let hub_installed = HubLocator::new(FileProbe, home.clone()).locate().is_some();

    let store = JobsStore::new();
    // Holding this stream open is what keeps the hub alive, so the reader
    // outlives any window and is never tied to one.
    let _reader = store.clone().spawn_reader(home, stream_path(SURFACE));

    let settings = nerve_windows_surface::settings::Settings::load(
        &nerve_windows_surface::settings::settings_path(),
    );
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(true)
            .with_inner_size([settings.panel_width, settings.panel_height])
            .with_min_inner_size([
                nerve_windows_surface::settings::WIDTH.0,
                nerve_windows_surface::settings::HEIGHT.0,
            ])
            .with_max_inner_size([
                nerve_windows_surface::settings::WIDTH.1,
                nerve_windows_surface::settings::HEIGHT.1,
            ])
            .with_always_on_top()
            // No taskbar button and no Alt-Tab entry: the flyout is a glance,
            // not a window to manage.
            .with_taskbar(false)
            // Shown on the first tray click. Built now so reopening is instant
            // and the font atlas is only ever built once.
            .with_visible(false),
        ..Default::default()
    };

    let desktop = std::rc::Rc::new(std::cell::RefCell::new(Desktop {
        open_requested: !autostarted,
        ..Default::default()
    }));
    let event_loop =
        match winit::event_loop::EventLoop::<eframe::UserEvent>::with_user_event().build() {
            Ok(event_loop) => event_loop,
            Err(error) => {
                tracing::error!(%error, "could not create event loop");
                return;
            }
        };
    let app_desktop = desktop.clone();
    let app = eframe::create_native(
        "Nerve",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, store, hub_installed, app_desktop)))),
        &event_loop,
    );
    let result = event_loop.run_app(&mut Runtime::new(app, desktop, listener));
    if let Err(error) = result {
        tracing::error!(%error, "event loop ended");
        eprintln!("nerve: event loop ended ({error})");
    }
}

/// `%USERPROFILE%` on Windows, `$HOME` elsewhere.
fn home_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default()
}
