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

use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use eframe::{App as EframeApp, CreationContext, Frame};
use egui::{Context, ViewportCommand};
use nerve_surface_core::machine;
use nerve_surface_core::store::JobsStore;
use time::OffsetDateTime;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::actions::clipboard::SystemClipboard;
use crate::actions::shell::SystemOpener;
use crate::actions::{self};
use crate::flyout::anchor::{Rect, place_scaled};
use crate::flyout::view::{PanelAction, PanelState};
use crate::flyout::visibility::Visibility;
use crate::flyout::{fonts, theme as panel_theme, view as panel};

use crate::notify::toast::{Toaster, WindowsToaster};
use crate::platform::{autostart, theme as system_theme};
use crate::settings::{Settings, settings_path};
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
    hub_installed: bool,
    local_alias: Option<String>,

    tray: Option<TrayIcon>,
    drawn: Option<Signature>,
    tooltip: Option<String>,
    icon_px: u32,
    theme: crate::tray::icon::Theme,

    panel: PanelState,
    visibility: Visibility,
    quitting: bool,
    tray_events: Receiver<TrayIconEvent>,
    menu_events: Receiver<MenuEvent>,
    tray_rect: Option<Rect>,

    toaster: WindowsToaster,
    toast_clicks: Receiver<String>,

    /// The last thing an action said, shown under the header briefly.
    note: Option<(String, Instant)>,

    menu: MenuIds,
}

/// The context-menu items whose clicks mean something.
struct MenuIds {
    open: tray_icon::menu::MenuId,
    autostart: CheckMenuItem,
    toasts: tray_icon::menu::MenuId,
    sound: tray_icon::menu::MenuId,
    quit: tray_icon::menu::MenuId,
}

impl App {
    pub fn new(cc: &CreationContext<'_>, store: JobsStore, hub_installed: bool) -> Self {
        let mut settings = Settings::load(&settings_path());
        settings.autostart = autostart::is_enabled();
        let theme = system_theme::current();

        let mut fonts = egui::FontDefinitions::default();
        fonts::install_cjk(&mut fonts);
        cc.egui_ctx.set_fonts(fonts);
        cc.egui_ctx.set_visuals(panel_theme::visuals(theme));

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
        let (tray, menu) = build_tray(&settings);

        Self {
            store,
            settings,
            hub_installed,
            local_alias: machine::local_alias().map(str::to_string),
            tray,
            drawn: None,
            tooltip: None,
            icon_px: icon_px(cc.egui_ctx.pixels_per_point() as f64),
            theme,
            panel: PanelState::default(),
            visibility: Visibility::default(),
            quitting: false,
            tray_events,
            menu_events,
            tray_rect: None,
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
        self.store.configure_notifications(serde_json::json!({"enabled":self.settings.toasts_enabled,"sound":self.settings.toast_sound,
            "floor":match self.settings.toast_floor { nerve_surface_core::frame::AttentionLevel::Urgent => 4, nerve_surface_core::frame::AttentionLevel::Required => 3, _ => 2 }}));
        for event in self.store.drain_notifications() {
            let delivered = self.toaster.show(&crate::notify::policy::Toast {
                title: event.title,
                body: event.body,
                job_id: event.job_id,
                tag: event.id.clone(),
                sound: event.sound,
            });
            self.store.notification_result(event.id, delivered);
        }
    }

    fn show_panel(&mut self, ctx: &Context) {
        if let (Some(tray), Some(monitor)) = (self.tray_rect, monitor_rect(ctx)) {
            let (x, y) = place_scaled(
                tray,
                monitor,
                (self.settings.panel_width, self.settings.panel_height),
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
            PanelAction::CycleGroup => {
                self.settings.group_mode = self.settings.group_mode.next();
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
        let _ = self.settings.save(&settings_path());
    }

    /// Tray clicks, menu clicks and toast clicks, all of which arrive out of
    /// band from other threads.
    fn drain_events(&mut self, ctx: &Context) {
        while let Ok(event) = self.tray_events.try_recv() {
            let toggle = crate::tray::events::toggles_panel(&event);
            if let TrayIconEvent::Click { rect, .. } = event {
                self.tray_rect = Some(Rect::new(
                    rect.position.x as i32,
                    rect.position.y as i32,
                    rect.size.width as i32,
                    rect.size.height as i32,
                ));
                if toggle {
                    self.toggle_panel(ctx);
                }
            }
        }

        while let Ok(event) = self.menu_events.try_recv() {
            if event.id == self.menu.open {
                self.show_panel(ctx);
            } else if event.id == self.menu.autostart.id() {
                let wanted = !autostart::is_enabled();
                self.settings.autostart = autostart::set(wanted);
                self.menu.autostart.set_checked(self.settings.autostart);
                self.persist();
            } else if event.id == self.menu.toasts {
                self.settings.toasts_enabled = !self.settings.toasts_enabled;
                self.persist();
            } else if event.id == self.menu.sound {
                self.settings.toast_sound = !self.settings.toast_sound;
                self.persist();
            } else if event.id == self.menu.quit {
                self.persist();
                self.quitting = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }

        // A clicked banner means "take me there", the same as the row's Open.
        while let Ok(job_id) = self.toast_clicks.try_recv() {
            self.act(PanelAction::Open(job_id), ctx);
        }
    }
}

impl EframeApp for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut Frame) {
        let context = ui.ctx().clone();
        let ctx = &context;
        if self.visibility.visible
            && let Some(rect) = ctx.input(|input| input.viewport().inner_rect)
        {
            self.settings.set_panel_size(rect.width(), rect.height());
        }
        self.drain_events(ctx);
        if ctx.input(|input| input.viewport().close_requested()) && !self.quitting {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.hide_panel(ctx);
        }

        let theme = system_theme::current();
        if theme != self.theme {
            self.theme = theme;
            ctx.set_visuals(panel_theme::visuals(theme));
            self.drawn = None;
        }
        self.icon_px = icon_px(ctx.pixels_per_point() as f64);

        self.refresh_tray();
        self.notify();

        if self.visibility.visible {
            // Focus loss dismisses. The panel is a glance, not a window to
            // manage, so it never competes for the taskbar or Alt-Tab.
            let focused = ctx.input(|input| input.viewport().focused);
            if self.visibility.lost_focus(focused) {
                self.hide_panel(ctx);
            }
            if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
                self.hide_panel(ctx);
            }
        }

        let action = if self.visibility.visible {
            if let Some((note, at)) = &self.note
                && at.elapsed() < Duration::from_secs(3)
            {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(note).small());
            }
            let snapshot = self.store.snapshot();
            panel::show(
                ui,
                &snapshot,
                &mut self.panel,
                self.settings.group_mode,
                self.hub_installed,
                self.theme,
                OffsetDateTime::now_utc(),
            )
        } else {
            None
        };
        if let Some(action) = action {
            self.act(action, ctx);
        }

        // The flyout lays out only while it is open. Hidden, the timer still
        // refreshes the tray and Ask toasts, just not four times a second.
        let interval = if self.visibility.visible {
            POLL
        } else {
            Duration::from_secs(1)
        };
        ctx.request_repaint_after(interval);
    }
}

/// Build the tray icon and its context menu.
fn build_tray(settings: &Settings) -> (Option<TrayIcon>, MenuIds) {
    let open = MenuItem::new("Open Nerve", true, None);
    let start = CheckMenuItem::new("Start at login", true, settings.autostart, None);
    let toasts = CheckMenuItem::new("Ask notifications", true, settings.toasts_enabled, None);
    let sound = CheckMenuItem::new("Notification sound", true, settings.toast_sound, None);
    let quit = MenuItem::new("Quit", true, None);

    let ids = MenuIds {
        open: open.id().clone(),
        autostart: start.clone(),
        toasts: toasts.id().clone(),
        sound: sound.id().clone(),
        quit: quit.id().clone(),
    };

    let menu = Menu::new();
    let _ = menu.append_items(&[
        &open,
        &PredefinedMenuItem::separator(),
        &start,
        &toasts,
        &sound,
        &PredefinedMenuItem::separator(),
        &quit,
    ]);

    let tray = TrayIconBuilder::new()
        .with_tooltip("Nerve")
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .ok();
    (tray, ids)
}

/// The monitor the flyout will land on, in physical pixels.
fn monitor_rect(ctx: &Context) -> Option<Rect> {
    let size = ctx.input(|input| input.viewport().monitor_size)?;
    let scale = ctx.pixels_per_point();
    Some(Rect::new(
        0,
        0,
        (size.x * scale) as i32,
        (size.y * scale) as i32,
    ))
}
