#![cfg(feature = "legacy-ui")]
use nerve_windows_surface::tray::events::toggles_panel;
use nerve_windows_surface::tray::events::{ClickSequence, PanelIntent};
use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};

fn click(state: MouseButtonState) -> TrayIconEvent {
    TrayIconEvent::Click {
        id: "nerve".into(),
        position: Default::default(),
        rect: Default::default(),
        button: MouseButton::Left,
        button_state: state,
    }
}

#[test]
fn windows_double_click_finishes_open_and_the_next_click_still_works() {
    let mut sequence = ClickSequence::default();
    assert_eq!(sequence.accept(&click(MouseButtonState::Down)), None);
    assert_eq!(
        sequence.accept(&click(MouseButtonState::Up)),
        Some(PanelIntent::Toggle)
    );
    assert_eq!(
        sequence.accept(&TrayIconEvent::DoubleClick {
            id: "nerve".into(),
            position: Default::default(),
            rect: Default::default(),
            button: MouseButton::Left,
        }),
        Some(PanelIntent::Open)
    );
    assert_eq!(sequence.accept(&click(MouseButtonState::Up)), None);
    assert_eq!(sequence.accept(&click(MouseButtonState::Down)), None);
    assert_eq!(
        sequence.accept(&click(MouseButtonState::Up)),
        Some(PanelIntent::Toggle)
    );
}

#[test]
fn one_mouse_click_toggles_once_and_right_click_only_opens_the_menu() {
    for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
        for button_state in [MouseButtonState::Down, MouseButtonState::Up] {
            let event = TrayIconEvent::Click {
                id: "nerve".into(),
                position: Default::default(),
                rect: Default::default(),
                button,
                button_state,
            };
            assert_eq!(
                toggles_panel(&event),
                button == MouseButton::Left && button_state == MouseButtonState::Up
            );
        }
    }
}
