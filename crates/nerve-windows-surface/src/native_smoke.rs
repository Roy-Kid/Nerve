//! Explicit desktop test: real winit/wgpu window and App, isolated from the hub.
use super::*;
use crate::platform::instance::{activate, claim, Claim};
use crate::runtime::{Desktop, Runtime};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::mpsc::Sender,
};
use winit::platform::windows::EventLoopBuilderExtWindows;

struct Probe {
    app: App,
    step: u8,
    at: Instant,
    port: u16,
    tray: Sender<TrayIconEvent>,
    menu: Sender<MenuEvent>,
    completed: Rc<Cell<bool>>,
    screenshots: usize,
}

impl EframeApp for Probe {
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        self.app.clear_color(visuals)
    }
    fn update(&mut self, ctx: &Context, frame: &mut Frame) {
        if (4..=9).contains(&self.step) {
            let theme = if self.step <= 5 {
                crate::tray::icon::Theme::Dark
            } else {
                crate::tray::icon::Theme::Light
            };
            self.app.panel_theme = theme;
            self.app.last_refresh = Some(Instant::now());
            ctx.set_visuals(panel_theme::visuals(theme));
        }
        self.app.update(ctx, frame);
        for event in ctx.input(|input| input.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = event {
                let mut pixels =
                    tiny_skia::Pixmap::new(image.width() as u32, image.height() as u32).unwrap();
                for (target, pixel) in pixels.data_mut().chunks_exact_mut(4).zip(&image.pixels) {
                    target.copy_from_slice(&pixel.to_array());
                }
                let path =
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(match self.screenshots {
                        0 => "../../target/windows-panel-smoke.png",
                        1 => "../../target/windows-panel-dark.png",
                        2 => "../../target/windows-panel-settings.png",
                        _ => "../../target/windows-panel-empty.png",
                    });
                pixels.save_png(path).unwrap();
                self.screenshots += 1;
            }
        }
        if matches!(self.step, 0 | 3 | 10) && self.at.elapsed() < Duration::from_millis(600) {
            return;
        }
        if (([2, 5, 7, 9].contains(&self.step)
            && self.screenshots
                < match self.step {
                    2 => 1,
                    5 => 2,
                    7 => 3,
                    _ => 4,
                })
            || (self.step == 4 && !self.app.visibility.visible))
            && self.at.elapsed() < Duration::from_secs(2)
        {
            return;
        }
        match self.step {
            0 => {
                assert!(!self.app.visibility.visible, "startup must remain hidden");
                use tray_icon::{MouseButton, MouseButtonState};
                for state in [MouseButtonState::Down, MouseButtonState::Up] {
                    self.tray
                        .send(TrayIconEvent::Click {
                            id: "test".into(),
                            position: Default::default(),
                            rect: Default::default(),
                            button: MouseButton::Left,
                            button_state: state,
                        })
                        .unwrap();
                }
                self.tray
                    .send(TrayIconEvent::DoubleClick {
                        id: "test".into(),
                        position: Default::default(),
                        rect: Default::default(),
                        button: MouseButton::Left,
                    })
                    .unwrap();
                self.tray
                    .send(TrayIconEvent::Click {
                        id: "test".into(),
                        position: Default::default(),
                        rect: Default::default(),
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                    })
                    .unwrap();
                ctx.request_repaint();
            }
            1 => {
                assert!(self.app.visibility.visible, "double click must stay open");
                ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
            }
            2 => {
                assert_eq!(self.screenshots, 1, "native renderer must produce a frame");
                self.app.hide_panel(ctx);
            }
            3 => {
                assert!(!self.app.visibility.visible);
                activate(self.port).unwrap();
            }
            4 => {
                assert!(
                    self.app.visibility.visible,
                    "second launch must wake a hidden panel"
                );
                ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
            }
            5 => {
                assert_eq!(self.screenshots, 2);
                self.app.panel.settings_open = true;
                self.app.show_panel(ctx);
            }
            6 => {
                ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
            }
            7 => {
                assert_eq!(self.screenshots, 3);
                self.app.panel.settings_open = false;
                self.app.store.set_jobs(Vec::new());
                self.app.show_panel(ctx);
            }
            8 => {
                ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
            }
            9 => {
                assert_eq!(self.screenshots, 4);
                self.app.hide_panel(ctx);
            }
            10 => {
                assert!(!self.app.visibility.visible);
                self.menu
                    .send(MenuEvent {
                        id: self.app.menu.quit.clone(),
                    })
                    .unwrap();
                ctx.request_repaint();
            }
            11 => {
                assert!(self.app.quitting, "Quit must be processed while hidden");
                assert!(self.app.tray.is_none(), "Quit removes the tray icon");
                self.completed.set(true);
            }
            _ => {}
        }
        self.step += 1;
        self.at = Instant::now();
    }
}

#[test]
#[ignore = "opens a real Windows flyout; run explicitly on an interactive desktop"]
fn hidden_window_double_click_activation_and_quit() {
    let mut builder = winit::event_loop::EventLoop::<eframe::UserEvent>::with_user_event();
    builder.with_any_thread(true);
    let event_loop = builder.build().unwrap();
    let Claim::Owned(listener) = claim(0).unwrap() else {
        panic!("lock");
    };
    let port = listener.local_addr().unwrap().port();
    let desktop = Rc::new(RefCell::new(Desktop::default()));
    let completed = Rc::new(Cell::new(false));
    let done = completed.clone();
    let shared = desktop.clone();
    let preferences =
        std::env::temp_dir().join(format!("nerve-native-smoke-{}.json", std::process::id()));
    let output = preferences.clone();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_visible(false)
            .with_decorations(false)
            .with_transparent(true)
            .with_inner_size([340.0, 360.0]),
        ..Default::default()
    };
    let app = eframe::create_native(
        "Nerve regression test",
        options,
        Box::new(move |cc| {
            let store = JobsStore::new();
            store.set_jobs(serde_json::from_value(serde_json::json!([
            {"id":"codex:1", "name":"Windows 界面修复", "alias":"local", "current":{"type":"tool", "summary":"Reviewing tray interactions"}},
            {"id":"claude:2", "name":"API integration", "alias":"local", "attention":{"level":"required", "title":"Ready for review"}}
        ])).unwrap());
            let mut app = App::new(cc, store, true, shared);
            app.preferences_path = output;
            app.settings = Settings::default();
            let (tray, events) = channel();
            app.tray_events = events;
            let (menu, events) = channel();
            app.menu_events = events;
            Ok(Box::new(Probe {
                app,
                step: 0,
                at: Instant::now(),
                port,
                tray,
                menu,
                completed: done,
                screenshots: 0,
            }))
        }),
        &event_loop,
    );
    event_loop
        .run_app(&mut Runtime::new(app, desktop, Some(listener)))
        .unwrap();
    let _ = std::fs::remove_file(preferences);
    assert!(completed.get(), "native lifecycle did not finish");
}
