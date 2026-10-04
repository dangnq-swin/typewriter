//! The frame: egui runs on the window's surface, through the painter we
//! drive — eframe's wgpu integration, kept home so the renderer can own it.

use std::num::NonZeroU32;
use std::sync::Arc;

use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

/// True on a desktop, false on the web: on the desktop child viewports get
/// windows of their own, on the web they embed into this one.
const IS_DESKTOP: bool = cfg!(not(target_arch = "wasm32"));

/// The app a window drives: one update per frame, one goodbye when it closes.
pub trait FrameApp {
    /// Draws one frame and reacts to it.
    fn update(&mut self, ui: &mut egui::Ui);

    /// The window is closing for good: put things away.
    fn on_exit(&mut self);
}

/// egui on a window we drive ourselves: the painter, egui-winit's input
/// state, and the frame that ties them together.
pub struct Egui {
    ctx: egui::Context,
    painter: egui_wgpu::winit::Painter,
    winit: Option<egui_winit::State>,
    /// What egui's widgets asked for last frame: screenshots, cut, copy, paste.
    actions_requested: Vec<egui_winit::ActionRequested>,
}

impl Egui {
    /// Builds egui and its painter: the wgpu instance only — the device
    /// comes when a window opens.
    pub fn new(ctx: egui::Context, depth_stencil: Option<wgpu::TextureFormat>) -> Self {
        ctx.set_embed_viewports(!IS_DESKTOP);
        let painter = pollster::block_on(egui_wgpu::winit::Painter::new(
            ctx.clone(),
            egui_wgpu::WgpuConfiguration::default(),
            false, // no transparent backbuffer: the desk paints every pixel
            egui_wgpu::RendererOptions {
                depth_stencil_format: depth_stencil,
                ..egui_wgpu::RendererOptions::default()
            },
        ));
        Self {
            ctx,
            painter,
            winit: None,
            actions_requested: Vec::new(),
        }
    }

    /// The egui context the app runs on.
    pub fn ctx(&self) -> &egui::Context {
        &self.ctx
    }

    /// The window's device, once [`Self::open`] opened one on it.
    pub fn render_state(&self) -> Option<egui_wgpu::RenderState> {
        self.painter.render_state()
    }

    /// Creates the window from `builder` and opens egui on it: the surface,
    /// the device, and egui-winit's input state.
    pub fn open(
        &mut self,
        event_loop: &ActiveEventLoop,
        builder: &egui::ViewportBuilder,
    ) -> anyhow::Result<Arc<Window>> {
        let window = Arc::new(egui_winit::create_window(&self.ctx, event_loop, builder)?);
        pollster::block_on(
            self.painter
                .set_window(egui::ViewportId::ROOT, Some(window.clone())),
        )
        .map_err(|err| anyhow::anyhow!("the window's device failed to build: {err}"))?;
        self.winit = Some(egui_winit::State::new(
            self.ctx.clone(),
            egui::ViewportId::ROOT,
            event_loop,
            Some(window.scale_factor() as f32),
            event_loop.system_theme(),
            self.painter.max_texture_side(),
        ));
        Ok(window)
    }

    /// Screen readers find the app through this; their action requests come
    /// back through the loop's user events, see [`Self::accesskit_event`].
    pub fn init_accesskit<T: From<egui_winit::accesskit_winit::Event> + Send>(
        &mut self,
        event_loop: &ActiveEventLoop,
        window: &Window,
        proxy: winit::event_loop::EventLoopProxy<T>,
    ) {
        if let Some(winit) = &mut self.winit {
            winit.init_accesskit(event_loop, window, proxy);
        }
    }

    /// An accesskit event the loop's user events handed back. Returns
    /// whether to redraw at once.
    pub fn accesskit_event(&mut self, event: &egui_winit::accesskit_winit::Event) -> bool {
        match &event.window_event {
            egui_winit::accesskit_winit::WindowEvent::InitialTreeRequested => {
                self.ctx.enable_accesskit();
                true
            }
            egui_winit::accesskit_winit::WindowEvent::ActionRequested(request) => {
                if let Some(winit) = &mut self.winit {
                    winit.on_accesskit_action_request(request.clone());
                }
                true
            }
            egui_winit::accesskit_winit::WindowEvent::AccessibilityDeactivated => {
                self.ctx.disable_accesskit();
                false
            }
        }
    }

    /// The surface follows the window: a resize reconfigures it.
    pub fn on_window_resized(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        // Zero means the window is minimized on Windows; there is nothing to
        // resize into.
        if let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        {
            self.painter
                .on_window_resized(egui::ViewportId::ROOT, width, height);
        }
    }

    /// Input the window took in, egui-style.
    pub fn on_window_event(
        &mut self,
        window: &Window,
        event: &winit::event::WindowEvent,
    ) -> egui_winit::EventResponse {
        self.winit.as_mut().map_or(
            egui_winit::EventResponse {
                consumed: false,
                repaint: false,
            },
            |winit| winit.on_window_event(window, event),
        )
    }

    /// Raw mouse motion while a drag runs past the window's edge.
    pub fn on_mouse_motion(&mut self, delta: (f64, f64)) -> bool {
        self.winit
            .as_mut()
            .is_some_and(|winit| winit.on_mouse_motion(delta))
    }

    /// Whether a pointer button is down, whatever window it was over.
    pub fn any_pointer_button_down(&self) -> bool {
        self.winit
            .as_ref()
            .is_some_and(egui_winit::State::is_any_pointer_button_down)
    }

    /// One frame: egui runs, the painter paints. Returns true when the
    /// window should close — egui asked and nothing cancelled it this frame.
    pub fn frame(
        &mut self,
        window: &Arc<Window>,
        event_loop: &ActiveEventLoop,
        info: &mut egui::ViewportInfo,
        app: &mut dyn FrameApp,
        clear_color: [f32; 4],
    ) -> bool {
        egui_winit::update_viewport_info(info, &self.ctx, window, false);
        let Some(winit) = &mut self.winit else {
            return false;
        };

        let mut raw_input = winit.take_egui_input(window);
        // The window's own state as egui will read it: fullscreen, focus,
        // close asks — egui-winit's own copy holds nothing native.
        raw_input
            .viewports
            .insert(egui::ViewportId::ROOT, info.clone());
        // Screenshot asks the loop took in while we slept.
        self.painter.handle_screenshots(&mut raw_input.events);
        let close_requested = raw_input.viewport().close_requested();

        let mut full_output = self.ctx.run_ui(raw_input, |ui| app.update(ui));

        info.events.clear(); // egui has read them

        winit.handle_platform_output_with_event_loop(
            window,
            event_loop,
            full_output.platform_output,
        );

        // egui asked to close; this frame may still cancel it with a dialog.
        let cancelled = full_output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .is_some_and(|out| out.commands.contains(&egui::ViewportCommand::CancelClose));

        if info.visible().unwrap_or(true) {
            let clipped = self
                .ctx
                .tessellate(full_output.shapes, full_output.pixels_per_point);
            let mut screenshots = Vec::new();
            self.actions_requested.retain(|action| match action {
                egui_winit::ActionRequested::Screenshot(data) => {
                    screenshots.push(data.clone());
                    false
                }
                _ => true,
            });
            self.painter.paint_and_update_textures(
                egui::ViewportId::ROOT,
                full_output.pixels_per_point,
                clear_color,
                &clipped,
                &mut full_output.textures_delta,
                screenshots,
                window,
            );
        }

        // Cut, copy and paste asks land in the next frame's input.
        for action in self.actions_requested.drain(..) {
            match action {
                egui_winit::ActionRequested::Screenshot(_) => {} // already painted
                egui_winit::ActionRequested::Cut => {
                    winit.egui_input_mut().events.push(egui::Event::Cut);
                }
                egui_winit::ActionRequested::Copy => {
                    winit.egui_input_mut().events.push(egui::Event::Copy);
                }
                egui_winit::ActionRequested::Paste => {
                    if let Some(text) = winit.clipboard_text() {
                        let text = text.replace("\r\n", "\n");
                        if !text.is_empty() {
                            winit.egui_input_mut().events.push(egui::Event::Paste(text));
                        }
                    }
                }
            }
        }

        // The app's viewport commands: fullscreen, inner size, focus, close.
        if let Some(out) = full_output.viewport_output.get(&egui::ViewportId::ROOT) {
            egui_winit::process_viewport_commands(
                &self.ctx,
                info,
                out.commands.clone(),
                window,
                &mut self.actions_requested,
            );
        }

        close_requested && !cancelled
    }
}
