//! The window: a winit loop the app runs in, egui painted onto it directly.

use std::sync::Arc;
use std::time::Instant;

use anyhow::Context as _;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::Window;

use crate::ui::{Egui, FrameApp};

/// How the window opens: everything the loop owes the app before it runs.
pub struct Options {
    /// The window's title.
    pub title: &'static str,
    /// The app's id, for the desktop to know it by.
    pub app_id: &'static str,
    /// Opens the window fullscreen at once.
    pub fullscreen: bool,
    /// The windowed size, asked only on a windowed launch: applying the
    /// inner size after the desktop sizes for fullscreen shrinks the
    /// fullscreen window to it, so the desktop's own sizing is never
    /// second-guessed.
    pub windowed_size: Option<egui::Vec2>,
    /// What the frame clears to before egui paints over it.
    pub clear_color: [f32; 4],
    /// A depth attachment for the pass callbacks to depth-test into.
    pub depth_stencil: Option<wgpu::TextureFormat>,
}

/// Events the loop feeds itself: egui's repaint signal and accesskit's.
enum UserEvent {
    /// egui asked for a repaint at `when`, from a frame or a thread outside it.
    Repaint {
        when: Instant,
        cumulative_pass_nr: u64,
    },
    /// accesskit's action requests, through the same proxy.
    AccessKit(egui_winit::accesskit_winit::Event),
}

impl From<egui_winit::accesskit_winit::Event> for UserEvent {
    fn from(event: egui_winit::accesskit_winit::Event) -> Self {
        Self::AccessKit(event)
    }
}

/// Opens the window and runs it until egui's close ask stands. The app is
/// built by `creator` once the window's device exists.
pub fn run<Creator>(options: Options, creator: Creator) -> anyhow::Result<()>
where
    Creator: FnOnce(
        &egui::Context,
        Option<&egui_wgpu::RenderState>,
    ) -> anyhow::Result<Box<dyn FrameApp>>,
{
    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;

    let mut builder = egui::ViewportBuilder::default()
        .with_title(options.title)
        .with_app_id(options.app_id)
        .with_fullscreen(options.fullscreen);
    // The inner size goes only on a windowed launch, see [`Options`].
    if !options.fullscreen
        && let Some(size) = options.windowed_size
    {
        builder = builder.with_inner_size(size);
    }

    let egui_ctx = egui::Context::default();

    // egui's repaint signal reaches the loop through the proxy, from a frame
    // or from a thread outside it — the one-instance hand-over repaints so.
    let proxy = event_loop.create_proxy();
    egui_ctx.set_request_repaint_callback(move |info| {
        let when = Instant::now() + info.delay;
        let _ = proxy.send_event(UserEvent::Repaint {
            when,
            cumulative_pass_nr: info.current_cumulative_pass_nr,
        });
    });

    let depth_stencil = options.depth_stencil;
    let mut window_loop = Loop {
        options,
        builder,
        egui: Egui::new(egui_ctx, depth_stencil),
        window: None,
        app: None,
        creator: Some(creator),
        proxy: event_loop.create_proxy(),
        info: egui::ViewportInfo::default(),
        next_repaint: None,
        error: None,
    };

    event_loop.run_app(&mut window_loop)?;

    match window_loop.error.take() {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

struct Loop<Creator> {
    options: Options,
    builder: egui::ViewportBuilder,
    egui: Egui,
    window: Option<Arc<Window>>,
    app: Option<Box<dyn FrameApp>>,
    creator: Option<Creator>,
    /// Hands repaint and accesskit events back into the loop.
    proxy: winit::event_loop::EventLoopProxy<UserEvent>,
    /// The window's state as egui reads it: fullscreen, focus, close asks.
    info: egui::ViewportInfo,
    /// When egui's pending repaint request fires, at the earliest.
    next_repaint: Option<Instant>,
    /// The failure that stopped the window before the loop could return it.
    error: Option<anyhow::Error>,
}

impl<Creator> ApplicationHandler<UserEvent> for Loop<Creator>
where
    Creator: FnOnce(
        &egui::Context,
        Option<&egui_wgpu::RenderState>,
    ) -> anyhow::Result<Box<dyn FrameApp>>,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            // Back from a suspend: the window and its surface stand.
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return;
        }
        if let Err(err) = self.open(event_loop) {
            self.error = Some(err);
            event_loop.exit();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.clone() else {
            return;
        };
        match &event {
            WindowEvent::RedrawRequested => self.frame(event_loop),
            WindowEvent::CloseRequested => {
                // egui decides: the app may still cancel with a dialog.
                self.info.events.push(egui::ViewportEvent::Close);
            }
            WindowEvent::Resized(size) => self.egui.on_window_resized(*size),
            WindowEvent::Occluded(occluded) => self.info.occluded = Some(*occluded),
            _ => {}
        }

        let repaint = self.egui.on_window_event(&window, &event).repaint;
        if repaint {
            #[cfg(not(target_os = "windows"))]
            window.request_redraw();
            #[cfg(target_os = "windows")]
            {
                // A resize must paint synchronously inside the event, or the
                // compositor assumes the window cannot keep up and flickers.
                if matches!(event, WindowEvent::Resized(_)) {
                    self.frame(event_loop);
                } else {
                    window.request_redraw();
                }
            }
        }
        self.settle(event_loop);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Repaint {
                when,
                cumulative_pass_nr,
            } => {
                // A request from a frame that already ran is stale; the pass
                // number tells them apart: accept the current or the next.
                let current = self
                    .egui
                    .ctx()
                    .cumulative_pass_nr_for(egui::ViewportId::ROOT);
                if current == cumulative_pass_nr || current == cumulative_pass_nr + 1 {
                    self.next_repaint = Some(self.next_repaint.map_or(when, |due| due.min(when)));
                }
            }
            UserEvent::AccessKit(event) => {
                if self.egui.accesskit_event(&event)
                    && let Some(window) = &self.window
                {
                    window.request_redraw();
                }
            }
        }
        self.settle(event_loop);
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, _cause: StartCause) {
        self.settle(event_loop);
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        // egui tracks a drag that leaves the window through raw motion, but
        // only while the window is focused or a button is down.
        if let DeviceEvent::MouseMotion { delta } = &event
            && let Some(window) = self.window.clone()
        {
            let in_play = window.has_focus() || self.egui.any_pointer_button_down();
            if in_play && self.egui.on_mouse_motion(*delta) {
                window.request_redraw();
            }
        }
        self.settle(event_loop);
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {}

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        // A quit from outside the loop — the desktop's quit — saves too.
        if let Some(mut app) = self.app.take() {
            app.on_exit();
        }
    }
}

impl<Creator> Loop<Creator>
where
    Creator: FnOnce(
        &egui::Context,
        Option<&egui_wgpu::RenderState>,
    ) -> anyhow::Result<Box<dyn FrameApp>>,
{
    /// Opens the window, hands the app its device, and asks for the first
    /// frame.
    fn open(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        let window = self.egui.open(event_loop, &self.builder)?;
        self.window = Some(window.clone());
        // accesskit's action requests come back through this loop's events.
        self.egui
            .init_accesskit(event_loop, &window, self.proxy.clone());
        egui_winit::update_viewport_info(&mut self.info, self.egui.ctx(), &window, true);
        let creator = self.creator.take().context("the window opened twice")?;
        let app = creator(self.egui.ctx(), self.egui.render_state().as_ref())?;
        self.app = Some(app);
        window.request_redraw();
        Ok(())
    }

    /// One frame on the window; on egui's standing close ask, the app says
    /// its goodbyes and the loop ends.
    fn frame(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let Some(app) = self.app.as_mut() else {
            return;
        };
        let close = self.egui.frame(
            &window,
            event_loop,
            &mut self.info,
            app.as_mut(),
            self.options.clear_color,
        );
        if close {
            // Once: a quit from outside the loop must not repeat the goodbye.
            if let Some(mut app) = self.app.take() {
                app.on_exit();
            }
            event_loop.exit();
        }
    }

    /// Turns due repaint requests into redraws and keeps the loop sleeping
    /// until the next one — polling would busy-loop a whole core.
    fn settle(&mut self, event_loop: &ActiveEventLoop) {
        if self.next_repaint.is_some_and(|due| due <= Instant::now()) {
            self.next_repaint = None;
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
        event_loop.set_control_flow(
            self.next_repaint
                .map_or(ControlFlow::Wait, ControlFlow::WaitUntil),
        );
    }
}
