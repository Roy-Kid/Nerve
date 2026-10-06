#![cfg(feature = "legacy-ui")]
use egui::{Context, Event, FullOutput, PointerButton, Pos2, RawInput, Rect, Vec2};
use nerve_surface_core::store::JobsSnapshot;
use nerve_windows_surface::{
    flyout::{
        theme,
        view::{self, PanelAction, PanelState, SettingsView},
    },
    settings::GroupMode,
    tray::icon::Theme,
};

fn render(
    ctx: &Context,
    state: &mut PanelState,
    jobs: usize,
    events: Vec<Event>,
) -> (FullOutput, Option<PanelAction>) {
    let snapshot = JobsSnapshot {
        jobs: (0..jobs).map(|index| serde_json::from_value(serde_json::json!({"id":format!("job:{index}"), "name":format!("Project {index}")})).unwrap()).collect(),
        ..Default::default()
    };
    let mut action = None;
    let output = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 240.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                action = view::show(
                    ui,
                    &snapshot,
                    state,
                    view::Presentation {
                        group_mode: GroupMode::Machine,
                        settings: SettingsView::default(),
                        hub_installed: true,
                        theme: Theme::Light,
                        now: time::OffsetDateTime::now_utc(),
                    },
                );
            });
        },
    );
    (output, action)
}

fn text_position(output: &FullOutput, label: &str) -> Pos2 {
    for shape in &output.shapes {
        if let egui::epaint::Shape::Text(text) = &shape.shape {
            if text.galley.text() == label {
                let rect = Rect::from_min_size(text.pos, text.galley.size());
                assert!(
                    shape.clip_rect.contains(rect.center()),
                    "{label} was clipped"
                );
                assert!(rect.max.y <= 240.0, "{label} fell below the window");
                return rect.center();
            }
        }
    }
    panic!("missing visible control {label}");
}

fn click(
    ctx: &Context,
    state: &mut PanelState,
    jobs: usize,
    position: Pos2,
) -> Option<PanelAction> {
    render(
        ctx,
        state,
        jobs,
        vec![
            Event::PointerMoved(position),
            Event::PointerButton {
                pos: position,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    render(
        ctx,
        state,
        jobs,
        vec![Event::PointerButton {
            pos: position,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    )
    .1
}

#[test]
fn settings_and_quit_remain_usable_with_zero_or_many_jobs_at_minimum_size() {
    for jobs in [0, 100] {
        let ctx = Context::default();
        theme::install(&ctx, Theme::Light);
        let mut state = PanelState::default();
        render(&ctx, &mut state, jobs, vec![]);
        render(&ctx, &mut state, jobs, vec![]);
        let settings = ctx
            .read_response(egui::Id::new("panel-settings"))
            .unwrap()
            .rect
            .center();
        click(&ctx, &mut state, jobs, settings);
        assert!(state.settings_open);
        render(&ctx, &mut state, jobs, vec![]);
        let (output, _) = render(&ctx, &mut state, jobs, vec![]);
        let toasts = text_position(&output, "Ask notifications");
        assert_eq!(
            click(&ctx, &mut state, jobs, toasts),
            Some(PanelAction::ToggleToasts)
        );
        render(&ctx, &mut state, jobs, vec![]);
        let quit = ctx
            .read_response(egui::Id::new("panel-quit"))
            .unwrap()
            .rect
            .center();
        assert_eq!(click(&ctx, &mut state, jobs, quit), Some(PanelAction::Quit));
    }
}

fn key(key: egui::Key) -> Vec<Event> {
    vec![Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }]
}

#[test]
fn keyboard_selects_expands_and_clamps_to_the_visible_list() {
    let ctx = Context::default();
    theme::install(&ctx, Theme::Light);
    let mut state = PanelState::default();
    render(&ctx, &mut state, 3, vec![]);
    render(&ctx, &mut state, 3, key(egui::Key::ArrowDown));
    let first = state.selected.clone().expect("first row selected");
    assert_eq!(state.expanded, Some(first.clone()));
    render(&ctx, &mut state, 3, key(egui::Key::Enter));
    assert_eq!(state.expanded, None);
    render(&ctx, &mut state, 3, key(egui::Key::Space));
    assert_eq!(state.expanded, Some(first.clone()));
    render(&ctx, &mut state, 3, key(egui::Key::ArrowDown));
    assert_ne!(state.selected, Some(first.clone()));
    render(&ctx, &mut state, 3, key(egui::Key::ArrowDown));
    let last = state.selected.clone();
    render(&ctx, &mut state, 3, key(egui::Key::ArrowDown));
    assert_eq!(state.selected, last);
    for _ in 0..4 {
        render(&ctx, &mut state, 3, key(egui::Key::ArrowUp));
    }
    assert_eq!(state.selected, Some(first));
    render(&ctx, &mut state, 0, key(egui::Key::ArrowDown));
    assert_eq!(state.selected, None);
    assert_eq!(state.expanded, None);
}

#[test]
fn settings_keyboard_does_not_change_job_selection() {
    let ctx = Context::default();
    let mut state = PanelState {
        settings_open: true,
        selected: Some("job:0".into()),
        ..Default::default()
    };
    render(&ctx, &mut state, 3, key(egui::Key::ArrowDown));
    render(&ctx, &mut state, 3, key(egui::Key::Enter));
    assert_eq!(state.selected.as_deref(), Some("job:0"));
    assert_eq!(state.expanded, None);
}

#[test]
fn keyboard_scrolls_selected_job_into_view() {
    let ctx = Context::default();
    theme::install(&ctx, Theme::Light);
    ctx.style_mut(|style| style.animation_time = 0.0);
    let mut state = PanelState::default();
    render(&ctx, &mut state, 100, vec![]);
    for _ in 0..25 {
        render(&ctx, &mut state, 100, key(egui::Key::ArrowDown));
    }
    let id = state.selected.as_ref().unwrap();
    let label = format!("Project {}", id.strip_prefix("job:").unwrap());
    render(&ctx, &mut state, 100, vec![]);
    let (output, _) = render(&ctx, &mut state, 100, vec![]);
    text_position(&output, &label);
}

#[test]
fn focused_header_button_keeps_enter_activation() {
    let ctx = Context::default();
    let mut state = PanelState::default();
    render(&ctx, &mut state, 3, vec![]);
    render(&ctx, &mut state, 3, vec![]);
    let quit = ctx.read_response(egui::Id::new("panel-quit")).unwrap();
    ctx.memory_mut(|memory| memory.request_focus(quit.id));
    assert_eq!(
        render(&ctx, &mut state, 3, key(egui::Key::Enter)).1,
        Some(PanelAction::Quit)
    );
    assert_eq!(state.selected, None);
}

#[test]
fn mouse_selection_can_continue_with_the_keyboard() {
    let ctx = Context::default();
    let mut state = PanelState::default();
    render(&ctx, &mut state, 3, vec![]);
    let (output, _) = render(&ctx, &mut state, 3, vec![]);
    let position = text_position(&output, "Project 0");
    click(&ctx, &mut state, 3, position);
    assert_eq!(state.selected.as_deref(), Some("job:0"));
    render(&ctx, &mut state, 3, key(egui::Key::ArrowDown));
    assert_ne!(state.selected.as_deref(), Some("job:0"));
}

#[test]
fn resize_grip_starts_native_resize_in_list_and_settings() {
    for settings_open in [false, true] {
        let ctx = Context::default();
        let mut state = PanelState {
            settings_open,
            ..Default::default()
        };
        render(&ctx, &mut state, 100, vec![]);
        render(&ctx, &mut state, 100, vec![]);
        let position = ctx
            .read_response(egui::Id::new("panel-resize"))
            .unwrap()
            .rect
            .center();
        let (output, _) = render(
            &ctx,
            &mut state,
            100,
            vec![
                Event::PointerMoved(position),
                Event::PointerButton {
                    pos: position,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        assert!(output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .iter()
            .any(|command| matches!(
                command,
                egui::ViewportCommand::BeginResize(egui::ResizeDirection::SouthEast)
            )));
    }
}

#[test]
fn header_blank_area_starts_window_drag_without_covering_buttons() {
    for settings_open in [false, true] {
        let ctx = Context::default();
        let mut state = PanelState {
            settings_open,
            ..Default::default()
        };
        render(&ctx, &mut state, 3, vec![]);
        render(&ctx, &mut state, 3, vec![]);
        let drag = ctx.read_response(egui::Id::new("panel-drag")).unwrap().rect;
        assert!(drag.width() > 20.0, "header needs a usable drag region");
        for id in ["panel-settings", "panel-quit"] {
            assert!(!drag.intersects(ctx.read_response(egui::Id::new(id)).unwrap().rect));
        }
        let (output, _) = render(
            &ctx,
            &mut state,
            3,
            vec![
                Event::PointerMoved(drag.center()),
                Event::PointerButton {
                    pos: drag.center(),
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        assert!(output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::StartDrag)));
    }
}
