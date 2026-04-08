use std::sync::Arc;

use lain_core::renderer::pipeline::RenderPipeline;
use lain_core::terminal::{Terminal, TerminalEvent};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, Modifiers, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

struct App {
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
    terminal: Option<Terminal>,
    render_pipeline: Option<RenderPipeline>,
    event_proxy: winit::event_loop::EventLoopProxy<TerminalEvent>,
    modifiers: Modifiers,
}

struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
}

impl App {
    fn new(event_proxy: winit::event_loop::EventLoopProxy<TerminalEvent>) -> Self {
        Self {
            window: None,
            gpu: None,
            terminal: None,
            render_pipeline: None,
            event_proxy,
            modifiers: Modifiers::default(),
        }
    }
}

impl ApplicationHandler<TerminalEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = Window::default_attributes()
            .with_title("lain-shell")
            .with_inner_size(winit::dpi::LogicalSize::new(640u32, 384u32));
        let window = Arc::new(event_loop.create_window(attrs).expect("failed to create window"));

        // Initialize wgpu
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let surface = instance
            .create_surface(window.clone())
            .expect("failed to create surface");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("no suitable GPU adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("lain-shell"),
                ..Default::default()
            },
        ))
        .expect("failed to create device");

        let size = window.inner_size();
        let surface_caps = surface.get_capabilities(&adapter);
        let format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Create render pipeline
        let render_pipeline = RenderPipeline::new(&device, &queue, format);

        // Get cell metrics from renderer for grid size calculation
        let (cell_w, cell_h) = render_pipeline.cell_metrics();
        let cols = (size.width as f32 / cell_w).max(1.0) as u16;
        let rows = (size.height as f32 / cell_h).max(1.0) as u16;

        // Create terminal with renderer-derived cell metrics
        let terminal = Terminal::new(
            cols,
            rows,
            cell_w as u16,
            cell_h as u16,
            self.event_proxy.clone(),
        )
        .expect("failed to create terminal");

        self.gpu = Some(GpuState {
            surface,
            device,
            queue,
            config,
        });
        self.render_pipeline = Some(render_pipeline);
        self.terminal = Some(terminal);
        self.window = Some(window);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            WindowEvent::ModifiersChanged(new_modifiers) => {
                self.modifiers = new_modifiers;
            }

            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.config.width = size.width.max(1);
                    gpu.config.height = size.height.max(1);
                    gpu.surface.configure(&gpu.device, &gpu.config);
                }
                if let (Some(terminal), Some(pipeline)) =
                    (&self.terminal, &self.render_pipeline)
                {
                    let (cell_w, cell_h) = pipeline.cell_metrics();
                    let cols = (size.width as f32 / cell_w).max(1.0) as u16;
                    let rows = (size.height as f32 / cell_h).max(1.0) as u16;
                    terminal.resize(cols, rows, cell_w as u16, cell_h as u16);
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => {
                let Some(gpu) = &self.gpu else { return };
                let Some(terminal) = &self.terminal else { return };
                let Some(pipeline) = &mut self.render_pipeline else {
                    return;
                };

                let frame = match gpu.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(frame)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
                    wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                        gpu.surface.configure(&gpu.device, &gpu.config);
                        return;
                    }
                    wgpu::CurrentSurfaceTexture::Timeout
                    | wgpu::CurrentSurfaceTexture::Occluded
                    | wgpu::CurrentSurfaceTexture::Validation => {
                        return;
                    }
                };

                let view =
                    frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder =
                    gpu.device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("render"),
                        });

                pipeline.render_frame(
                    &gpu.device,
                    &gpu.queue,
                    &mut encoder,
                    &view,
                    gpu.config.width,
                    gpu.config.height,
                    &terminal.term,
                );

                gpu.queue.submit(std::iter::once(encoder.finish()));
                frame.present();
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                let Some(terminal) = &self.terminal else { return };

                match &event.logical_key {
                    Key::Character(c) => {
                        let text = c.as_str();
                        if self.modifiers.state().control_key() && text.len() == 1 {
                            let ch = text.bytes().next().unwrap();
                            if ch.is_ascii_lowercase() {
                                terminal.send_input(&[ch - b'a' + 1]);
                            } else if ch.is_ascii_uppercase() {
                                terminal.send_input(&[ch - b'A' + 1]);
                            }
                        } else {
                            terminal.send_input(text.as_bytes());
                        }
                    }
                    Key::Named(named) => {
                        let bytes: &[u8] = match named {
                            NamedKey::Enter => b"\r",
                            NamedKey::Backspace => b"\x7f",
                            NamedKey::Tab => b"\t",
                            NamedKey::Escape => b"\x1b",
                            NamedKey::ArrowUp => b"\x1b[A",
                            NamedKey::ArrowDown => b"\x1b[B",
                            NamedKey::ArrowRight => b"\x1b[C",
                            NamedKey::ArrowLeft => b"\x1b[D",
                            NamedKey::Home => b"\x1b[H",
                            NamedKey::End => b"\x1b[F",
                            NamedKey::PageUp => b"\x1b[5~",
                            NamedKey::PageDown => b"\x1b[6~",
                            NamedKey::Delete => b"\x1b[3~",
                            _ => return,
                        };
                        terminal.send_input(bytes);
                    }
                    _ => {}
                }
            }

            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: TerminalEvent) {
        match event {
            TerminalEvent::Wakeup => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            TerminalEvent::Exit => {
                event_loop.exit();
            }
        }
    }
}

fn main() {
    let event_loop = EventLoop::<TerminalEvent>::with_user_event()
        .build()
        .expect("failed to create event loop");
    let proxy = event_loop.create_proxy();
    let mut app = App::new(proxy);
    event_loop.run_app(&mut app).expect("event loop error");
}
