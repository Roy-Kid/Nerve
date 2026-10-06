//! The running surface: tray icon, flyout, and what connects them to the hub.
//!
//! Everything that decides *what* is on screen is a pure function elsewhere.
//! This file is the part that needs a running desktop — it polls the store,
//! tells the shell when the drawn result would differ, and shows or hides one
//! window.
//!
//! The reader thread is what holds the hub's refcount (CLAUDE.md invariant 4),
//! and it is deliberately not tied to the window: hiding the flyout must never
//! drop the stream, or the hub would exit thirty seconds after the user looked
//! away.

use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

use eframe::{App as EframeApp, CreationContext, Frame};
use egui::{Context, ViewportCommand};
use nerve_surface_core::machine;
use nerve_surface_core::store::JobsStore;
use time::OffsetDateTime;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::actions::clipboard::SystemClipboard;
use crate::actions::shell::SystemOpener;
use crate::actions::{self};
use crate::flyout::anchor::{place_for_viewport, Rect};
use crate::flyout::view::{PanelAction, PanelState, SettingsView};
use crate::flyout::visibility::Visibility;
use crate::flyout::{fonts, theme as panel_theme, view as panel};
use crate::notify::policy::{AskPolicy, Settings as NotifySettings};
use crate::notify::toast::{Toaster, WindowsToaster};
use crate::platform::{autostart, theme as system_theme};
use crate::settings::{settings_path, Settings};
use crate::tray::dpi::icon_px;
use crate::tray::icon::render;
use crate::tray::signature::Signature;
use crate::tray::view as tray_view;

/// How often the surface re-derives what it should say.
///
/// Four times a second is below what a person reads as lag and far above what
/// the shell is asked to do, because the redraw gate turns all but a handful of
/// these into nothing at all.
const POLL: Duration = Duration::from_millis(250);

pub struct App {
    store: JobsStore,
    settings: Settings,
    preferences_path: std::path::PathBuf,
    hub_installed: bool,
    local_alias: Option<String>,

    tray: Option<TrayIcon>,
    drawn: Option<Signature>,
    tooltip: Option<String>,
    icon_px: u32,
    last_refresh: Option<Instant>,
    theme: crate::tray::icon::Theme,
    panel_theme: crate::tray::icon::Theme,
    desktop: crate::runtime::SharedDesktop,
    clicks: crate::tray::events::ClickSequence,

    panel: PanelState,
    visibility: Visibility,
    quitting: bool,
    opened_this_frame: bool,
    tray_events: Receiver<TrayIconEvent>,
    menu_events: Receiver<MenuEvent>,
    tray_rect: Option<Rect>,

    notifier: AskPolicy,
    toaster: WindowsToaster,
    toast_clicks: Receiver<String>,

    /// The last thing an action said, shown under the header briefly.
    note: Option<(String, Instant)>,

    menu: MenuIds,
}

/// The context-menu items whose clicks mean something.
///
/// Settings moved into the flyout's Settings section; the menu keeps only the
/// two things that are not preferences — open and quit.
struct MenuIds {
    open: tray_icon::menu::MenuId,
    quit: tray_icon::menu::MenuId,
}

impl App {
    pub fn new(
        cc: &CreationContext<'_>,
        store: JobsStore,
        hub_installed: bool,
        desktop: crate::runtime::SharedDesktop,
    ) -> Self {
        let mut settings = Settings::load(&settings_path());
        settings.autostart = autostart::is_enabled();
        let theme = system_theme::current();
        let panel_theme = system_theme::apps();

        let mut fonts = egui::FontDefinitions::default();
        fonts::install_cjk(&mut fonts);
        cc.egui_ctx.set_fonts(fonts);
        panel_theme::install(&cc.egui_ctx, panel_theme);

        let (toaster, toast_clicks) = WindowsToaster::new(cc.egui_ctx.clone());
        // Native events must wake even a hidden viewport. Once handlers are set,
        // tray-icon no longer feeds its global receivers.
        let (tray_sender, tray_events) = channel();
        let wake = cc.egui_ctx.clone();
        TrayIconEvent::set_event_handler(Some(move |event| {
            let _ = tray_sender.send(event);
            wake.request_repaint();
        }));
        let (menu_sender, menu_events) = channel();
        let wake = cc.egui_ctx.clone();
        MenuEvent::set_event_handler(Some(move |event| {
            let _ = menu_sender.send(event);
            wake.request_repaint();
        }));
        let (tray, menu) = build_tray(theme);
        if tray.is_none() {
            desktop.borrow_mut().open_requested = true;
        }

        Self {
            store,
            settings,
            preferences_path: settings_path(),
            hub_installed,
            local_alias: machine::local_alias().map(str::to_string),
            tray,
            drawn: None,
            tooltip: None,
            icon_px: icon_px(cc.egui_ctx.pixels_per_point() as f64),
            last_refresh: None,
            theme,
            panel_theme,
            desktop,
            clicks: Default::default(),
            panel: PanelState::default(),
            visibility: Visibility::default(),
            quitting: false,
            opened_this_frame: false,
            tray_events,
            menu_events,
            tray_rect: None,
            notifier: AskPolicy::new(),
            toaster,
            toast_clicks,
            note: None,
            menu,
        }
    }

    /// Re-derive the tray's appearance and tell the shell only on a change.
    fn refresh_tray(&mut self) {
        let snapshot = self.store.snapshot();
        let view = tray_view::of(
            &snapshot,
            self.hub_installed,
            self.theme,
            self.icon_px,
            OffsetDateTime::now_utc(),
        );
        let Some(tray) = self.tray.as_mut() else {
            return;
        };
        if self.drawn != Some(view.signature) {
            let rgba = render(&view.icon);
            if let Ok(icon) = Icon::from_rgba(rgba, view.icon.size, view.icon.size) {
                // Cache only successful shell updates, so failures retry.
                if tray.set_icon(Some(icon)).is_ok() {
                    self.drawn = Some(view.signature);
                }
            }
        }
        // Text can change while the status bands stay exactly the same.
        if self.tooltip.as_ref() != Some(&view.tooltip)
            && tray.set_tooltip(Some(&view.tooltip)).is_ok()
        {
            self.tooltip = Some(view.tooltip);
        }
    }

    /// Ask the policy whether this frame is worth interrupting for.
    fn notify(&mut self) {
        let snapshot = self.store.snapshot();
        let settings = NotifySettings {
            enabled: self.settings.toasts_enabled
                && !snapshot.offline
                && snapshot.notify.may_interrupt("windows"),
            sound: self.settings.toast_sound,
            floor: self.settings.toast_floor,
        };
        for toast in self
            .notifier
            .evaluate(&snapshot.jobs, settings, Instant::now())
        {
            self.toaster.show(&toast);
        }
    }

    fn show_panel(&mut self, ctx: &Context) {
        self.opened_this_frame = true;
        let tray_rect = self.tray_rect.or_else(|| {
            self.tray.as_ref().and_then(|tray| tray.rect()).map(|rect| {
                Rect::new(
                    rect.position.x as i32,
                    rect.position.y as i32,
                    rect.size.width as i32,
                    rect.size.height as i32,
                )
            })
        });
        let target = tray_rect.and_then(|tray| {
            self.desktop
                .borrow()
                .monitors
                .iter()
                .copied()
                .find(|(monitor, _)| {
                    tray.x >= monitor.x
                        && tray.x < monitor.right()
                        && tray.y >= monitor.y
                        && tray.y < monitor.bottom()
                })
                .map(|(monitor, scale)| (tray, monitor, scale))
        });
        if let Some((tray, monitor, scale)) = target {
            let (x, y) = place_for_viewport(
                tray,
                monitor,
                (self.settings.panel_width, self.settings.panel_height),
                scale,
                ctx.pixels_per_point(),
            );
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(egui::pos2(x, y)));
        }
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(egui::vec2(
            self.settings.panel_width,
            self.settings.panel_height,
        )));
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
        self.visibility.show();
    }

    fn hide_panel(&mut self, ctx: &Context) {
        ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        self.visibility.hide(Instant::now());
        self.persist();
        self.panel.expanded = None;
    }

    fn toggle_panel(&mut self, ctx: &Context) {
        if self.visibility.visible {
            self.hide_panel(ctx);
            return;
        }
        // The click that closed the panel must not immediately reopen it.
        if !self.visibility.can_reopen(Instant::now()) {
            return;
        }
        self.show_panel(ctx);
    }

    fn act(&mut self, action: PanelAction, ctx: &Context) {
        match action {
            PanelAction::Quit => self.quit(ctx),
            PanelAction::CycleGroup => {
                self.settings.group_mode = self.settings.group_mode.next();
                self.panel.selected = None;
                self.persist();
            }
            PanelAction::ToggleAutostart => {
                let wanted = !autostart::is_enabled();
                self.settings.autostart = autostart::set(wanted);
                self.persist();
            }
            PanelAction::ToggleToasts => {
                self.settings.toasts_enabled = !self.settings.toasts_enabled;
                self.persist();
            }
            PanelAction::ToggleToastSound => {
                self.settings.toast_sound = !self.settings.toast_sound;
                self.persist();
            }
            PanelAction::Open(id) => {
                let snapshot = self.store.snapshot();
                if let Some(job) = snapshot.jobs.iter().find(|job| job.id == id) {
                    let outcome = actions::perform(
                        job,
                        self.local_alias.as_deref(),
                        &SystemOpener,
                        &mut SystemClipboard,
                    );
                    self.note = Some((outcome.message, Instant::now()));
                    // Taking the user somewhere means getting out of the way.
                    if outcome.opened {
                        self.hide_panel(ctx);
                    }
                }
            }
            PanelAction::Copy(id) => {
                let snapshot = self.store.snapshot();
                if let Some(job) = snapshot.jobs.iter().find(|job| job.id == id) {
                    let outcome = actions::copy(job, &mut SystemClipboard);
                    self.note = Some((outcome.message, Instant::now()));
                }
            }
        }
    }

    /// Not `save`: eframe's `App` trait owns that name for its own storage.
    fn persist(&self) {
        if let Err(error) = self.settings.save(&self.preferences_path) {
            tracing::warn!(%error, "could not save settings");
        }
    }

    fn quit(&mut self, ctx: &Context) {
        self.persist();
        self.quitting = true;
        self.tray.take();
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }

    /// Tray clicks, menu clicks and toast clicks, all of which arrive out of
    /// band from other threads.
    fn drain_events(&mut self, ctx: &Context) {
        let requested = std::mem::take(&mut self.desktop.borrow_mut().open_requested);
        if requested {
            self.show_panel(ctx);
        }
        while let Ok(event) = self.tray_events.try_recv() {
            let intent = self.clicks.accept(&event);
            let rect = match &event {
                TrayIconEvent::Click { rect, .. } | TrayIconEvent::DoubleClick { rect, .. } => {
                    Some(*rect)
                }
                _ => None,
            };
            if let Some(rect) = rect {
                self.tray_rect = Some(Rect::new(
                    rect.position.x as i32,
                    rect.position.y as i32,
                    rect.size.width as i32,
                    rect.size.height as i32,
                ));
            }
            match intent {
                Some(crate::tray::events::PanelIntent::Open) => self.show_panel(ctx),
                Some(crate::tray::events::PanelIntent::Toggle) => self.toggle_panel(ctx),
                None => {}
            }
        }

        while let Ok(event) = self.menu_events.try_recv() {
            if event.id == self.menu.open {
                self.show_panel(ctx);
            } else if event.id == self.menu.quit {
                self.quit(ctx);
                return;
            }
        }

        // A clicked banner means "take me there", the same as the row's Open.
        while let Ok(job_id) = self.toast_clicks.try_recv() {
            self.act(PanelAction::Open(job_id), ctx);
        }
    }
}

impl EframeApp for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        if self.visibility.visible {
            if let Some(rect) = ctx.input(|input| input.viewport().inner_rect) {
                self.settings.set_panel_size(rect.width(), rect.height());
            }
        }
        self.drain_events(ctx);
        if self.quitting {
            return;
        }
        if ctx.input(|input| input.viewport().close_requested()) && !self.quitting {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.hide_panel(ctx);
        }

        if self.last_refresh.is_none_or(|at| at.elapsed() >= POLL) {
            self.last_refresh = Some(Instant::now());
            let theme = system_theme::current();
            if theme != self.theme {
                self.theme = theme;
                self.drawn = None;
            }
            let theme = system_theme::apps();
            if theme != self.panel_theme {
                self.panel_theme = theme;
                ctx.set_visuals(panel_theme::visuals(theme));
            }
            self.icon_px = icon_px(ctx.pixels_per_point() as f64);

            self.refresh_tray();
            self.notify();
        }

        if !self.visibility.visible {
            return;
        }

        if self.visibility.visible {
            // Focus loss dismisses. The panel is a glance, not a window to
            // manage, so it never competes for the taskbar or Alt-Tab.
            let focused = ctx.input(|input| input.viewport().focused);
            if !std::mem::take(&mut self.opened_this_frame) && self.visibility.lost_focus(focused) {
                self.hide_panel(ctx);
            }
            if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
                self.hide_panel(ctx);
            }
        }

        let mut action = None;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().inner_margin(8))
            .show(ctx, |ui| {
                panel_theme::surface(ui, |ui| {
                    if let Some((note, at)) = &self.note {
                        if at.elapsed() < Duration::from_secs(3) {
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new(note).small());
                        }
                    }
                    let snapshot = self.store.snapshot();
                    let settings_view = SettingsView {
                        autostart: self.settings.autostart,
                        toasts_enabled: self.settings.toasts_enabled,
                        toast_sound: self.settings.toast_sound,
                    };
                    action = panel::show(
                        ui,
                        &snapshot,
                        &mut self.panel,
                        panel::Presentation {
                            group_mode: self.settings.group_mode,
                            settings: settings_view,
                            hub_installed: self.hub_installed,
                            theme: self.panel_theme,
                            now: OffsetDateTime::now_utc(),
                        },
                    );
                });
            });
        if let Some(action) = action {
            self.act(action, ctx);
        }

        // Polling rather than waking on frames: the redraw gate makes a poll
        // that changes nothing free, and the reader owns the socket either way.
        ctx.request_repaint_after(POLL);
    }
}

#[cfg(all(test, windows))]
#[path = "native_smoke.rs"]
mod native_smoke;

/// Build the tray icon and its context menu.
fn build_tray(theme: crate::tray::icon::Theme) -> (Option<TrayIcon>, MenuIds) {
    let open = MenuItem::new("Open Nerve", true, None);
    let quit = MenuItem::new("Quit", true, None);

    let ids = MenuIds {
        open: open.id().clone(),
        quit: quit.id().clone(),
    };

    let menu = Menu::new();
    let _ = menu.append_items(&[&open, &PredefinedMenuItem::separator(), &quit]);

    let spec = crate::tray::icon::IconSpec {
        size: 32,
        bands: Vec::new(),
        offline: false,
        theme,
    };
    let mut builder = TrayIconBuilder::new()
        .with_tooltip("Nerve")
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false);
    if let Ok(icon) = Icon::from_rgba(render(&spec), spec.size, spec.size) {
        builder = builder.with_icon(icon);
    }
    let tray = builder
        .build()
        .map_err(|error| {
            tracing::error!(%error, "could not create tray icon");
        })
        .ok();
    (tray, ids)
}
