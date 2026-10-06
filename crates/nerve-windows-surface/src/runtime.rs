//! Drive tray work even when Windows does not paint the hidden flyout.
use std::{
    cell::RefCell,
    net::TcpListener,
    rc::Rc,
    time::{Duration, Instant},
};

use eframe::{EframeWinitApplication, UserEvent};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow},
    window::WindowId,
};

use crate::flyout::anchor::Rect;

pub const POLL: Duration = Duration::from_millis(250);

#[derive(Default)]
pub struct Desktop {
    pub open_requested: bool,
    pub monitors: Vec<(Rect, f32)>,
}

pub type SharedDesktop = Rc<RefCell<Desktop>>;

pub struct Runtime<'a> {
    pub app: EframeWinitApplication<'a>,
    pub desktop: SharedDesktop,
    pub listener: Option<TcpListener>,
    window: Option<WindowId>,
    next_tick: Instant,
    wake: bool,
}

impl<'a> Runtime<'a> {
    pub fn new(
        app: EframeWinitApplication<'a>,
        desktop: SharedDesktop,
        listener: Option<TcpListener>,
    ) -> Self {
        Self {
            app,
            desktop,
            listener,
            window: None,
            next_tick: Instant::now(),
            wake: true,
        }
    }
}

impl ApplicationHandler<UserEvent> for Runtime<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.app.resumed(event_loop);
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        self.app.new_events(event_loop, cause);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        // Future repaint requests keep their deadline; repainting them now
        // would turn request_repaint_after into a busy loop.
        if matches!(&event, UserEvent::RequestRepaint { when, .. } if *when <= Instant::now()) {
            self.wake = true;
        }
        self.app.user_event(event_loop, event);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        self.window.get_or_insert(id);
        self.app.window_event(event_loop, id, event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.wake || Instant::now() >= self.next_tick {
            self.wake = false;
            self.next_tick = Instant::now() + POLL;
            {
                let mut desktop = self.desktop.borrow_mut();
                if let Some(listener) = &self.listener {
                    // Bound the batch so another local process cannot starve UI work.
                    for _ in 0..16 {
                        match listener.accept() {
                            Ok(_) => desktop.open_requested = true,
                            Err(_) => break,
                        }
                    }
                }
                desktop.monitors = event_loop
                    .available_monitors()
                    .map(|monitor| {
                        let p = monitor.position();
                        let s = monitor.size();
                        (
                            Rect::new(p.x, p.y, s.width as i32, s.height as i32),
                            monitor.scale_factor() as f32,
                        )
                    })
                    .collect();
            }
            if let Some(id) = self.window {
                // Call eframe directly: request_redraw relies on WM_PAINT,
                // which is not a reliable work queue for a hidden HWND.
                self.app
                    .window_event(event_loop, id, WindowEvent::RedrawRequested);
            }
        }
        self.app.about_to_wait(event_loop);
        if !event_loop.exiting() {
            let deadline = match event_loop.control_flow() {
                ControlFlow::WaitUntil(at) => at.min(self.next_tick),
                _ => self.next_tick,
            };
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
        }
    }

    fn device_event(&mut self, event_loop: &ActiveEventLoop, id: DeviceId, event: DeviceEvent) {
        self.app.device_event(event_loop, id, event);
    }
    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        self.app.suspended(event_loop);
    }
    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        self.app.exiting(event_loop);
    }
    fn memory_warning(&mut self, event_loop: &ActiveEventLoop) {
        self.app.memory_warning(event_loop);
    }
}
