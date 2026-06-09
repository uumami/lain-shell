//! `bebop run` -- dev binary: one window running $SHELL through the BEBOP core.
//! Reactive (ControlFlow::Wait), woken by PTY bytes via EventLoopProxy. Exercises
//! the GPU (wgpu+glyphon -> surface) and CPU (softbuffer) present paths in the
//! real crate. Input encoding is spike-grade -- a later plan replaces it. Not the
//! NAVI host; just a test vehicle until NAVI exists.
//!
//! Usage: bebop run [--cpu] [--smoke]

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use lain_types::ByteStream;
use terminal_core::{cell_size, CpuRenderer, GpuRenderer, LocalPty, ReaderPump, Renderer, Terminal};

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

const FONT_SIZE: f32 = 14.0;

#[derive(Debug, Clone, Copy)]
enum UserEvent {
    Pty,
    Timeout,
}

enum Backend {
    Gpu {
        surface: Box<wgpu::Surface<'static>>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        config: wgpu::SurfaceConfiguration,
        renderer: Box<GpuRenderer>,
    },
    Cpu {
        _context: softbuffer::Context<Arc<Window>>,
        surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
        renderer: Box<CpuRenderer>,
    },
}

struct App {
    use_cpu: bool,
    smoke: bool,
    proxy: EventLoopProxy<UserEvent>,
    window: Option<Arc<Window>>,
    backend: Option<Backend>,
    term: Option<Terminal>,
    pty: Option<LocalPty>,
    pump: Option<ReaderPump>,
    cell_w: u32,
    cell_h: u32,
    done: bool,
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = Arc::new(
            el.create_window(
                Window::default_attributes()
                    .with_title("bebop run")
                    .with_inner_size(LogicalSize::new(1000.0, 700.0)),
            )
            .expect("create window"),
        );
        let size = window.inner_size();
        let (cw, ch) = cell_size(FONT_SIZE);
        self.cell_w = cw.max(1);
        self.cell_h = ch.max(1);
        let cols = (size.width / self.cell_w).max(1) as usize;
        let lines = (size.height / self.cell_h).max(1) as usize;

        // PTY + core. Move the single reader into the pump; wake the loop per chunk.
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        let mut pty = LocalPty::spawn(&shell, cols as u16, lines as u16).expect("spawn shell");
        let reader = pty.take_reader();
        let wake_proxy = self.proxy.clone();
        let pump = ReaderPump::start_with_waker(reader, 64, 65536, move || {
            let _ = wake_proxy.send_event(UserEvent::Pty);
        });
        let term = Terminal::new(cols, lines);

        let backend = if self.use_cpu {
            let context = softbuffer::Context::new(window.clone()).expect("softbuffer context");
            let surface =
                softbuffer::Surface::new(&context, window.clone()).expect("softbuffer surface");
            Backend::Cpu { _context: context, surface, renderer: Box::new(CpuRenderer::new(FONT_SIZE)) }
        } else {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
            let surface = instance.create_surface(window.clone()).expect("surface");
            let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            }))
            .expect("no wgpu adapter (try `bebop run --cpu`)");
            eprintln!("adapter: {:?}", adapter.get_info());
            let (device, queue) = pollster::block_on(adapter.request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("bebop"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            ))
            .expect("request device");
            let caps = surface.get_capabilities(&adapter);
            let format = caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(caps.formats[0]);
            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: wgpu::PresentMode::Fifo, // vsync default (design §4)
                desired_maximum_frame_latency: 2,
                alpha_mode: caps.alpha_modes[0],
                view_formats: vec![],
            };
            surface.configure(&device, &config);
            let renderer = Box::new(GpuRenderer::new(&device, &queue, format, FONT_SIZE));
            Backend::Gpu { surface: Box::new(surface), device, queue, config, renderer }
        };

        el.set_control_flow(ControlFlow::Wait);
        self.backend = Some(backend);
        self.term = Some(term);
        self.pty = Some(pty);
        self.pump = Some(pump);
        self.window = Some(window.clone());

        if self.smoke {
            // Deterministic command; success when its output text reaches the grid.
            let _ = self.pty.as_mut().unwrap().write(b"printf 'BEBOPSMOKE\\n'\r");
        }
        window.request_redraw();
    }

    fn user_event(&mut self, el: &ActiveEventLoop, ev: UserEvent) {
        match ev {
            UserEvent::Timeout => {
                if !self.done {
                    println!("SMOKE_TIMEOUT");
                }
                el.exit();
            }
            UserEvent::Pty => {
                let bytes = self.pump.as_ref().map(|p| p.drain()).unwrap_or_default();
                if bytes.is_empty() {
                    return;
                }
                if let Some(t) = self.term.as_mut() {
                    t.feed(&bytes);
                    let replies = t.take_pty_writes();
                    if !replies.is_empty() {
                        let _ = self.pty.as_mut().unwrap().write(&replies);
                    }
                }
                if let Some(w) = self.window.as_ref() {
                    w.request_redraw();
                }
                self.check_smoke(el);
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(new) => {
                self.resize(new.width, new.height);
                if let Some(w) = self.window.as_ref() {
                    w.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => self.draw(),
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                let bytes: Vec<u8> = match event.logical_key {
                    Key::Named(NamedKey::Enter) => vec![b'\r'],
                    Key::Named(NamedKey::Backspace) => vec![0x7f],
                    Key::Named(NamedKey::Tab) => vec![b'\t'],
                    Key::Named(NamedKey::Space) => vec![b' '],
                    Key::Character(s) => s.as_bytes().to_vec(),
                    _ => Vec::new(),
                };
                if !bytes.is_empty() {
                    if let Some(p) = self.pty.as_mut() {
                        let _ = p.write(&bytes);
                    }
                }
            }
            _ => {}
        }
    }
}

impl App {
    fn check_smoke(&mut self, el: &ActiveEventLoop) {
        if !self.smoke || self.done {
            return;
        }
        let snap = match self.term.as_ref() {
            Some(t) => t.snapshot(),
            None => return,
        };
        if (0..snap.lines).any(|l| snap.row(l).contains("BEBOPSMOKE")) {
            println!("SMOKE_OK");
            self.done = true;
            el.exit();
        }
    }

    fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        let cols = (w / self.cell_w).max(1) as usize;
        let lines = (h / self.cell_h).max(1) as usize;
        if let Some(t) = self.term.as_mut() {
            t.resize(cols, lines);
        }
        if let Some(p) = self.pty.as_mut() {
            let _ = p.resize(cols as u16, lines as u16);
        }
        match self.backend.as_mut() {
            Some(Backend::Gpu { surface, device, config, .. }) => {
                config.width = w;
                config.height = h;
                surface.configure(device, config);
            }
            Some(Backend::Cpu { surface, .. }) => {
                if let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) {
                    let _ = surface.resize(nw, nh);
                }
            }
            None => {}
        }
    }

    fn draw(&mut self) {
        let snap = match self.term.as_ref() {
            Some(t) => t.snapshot(),
            None => return,
        };
        match self.backend.as_mut() {
            Some(Backend::Gpu { surface, device, queue, config, renderer }) => {
                let frame = match surface.get_current_texture() {
                    Ok(f) => f,
                    Err(_) => {
                        surface.configure(device, config);
                        match surface.get_current_texture() {
                            Ok(f) => f,
                            Err(_) => return,
                        }
                    }
                };
                let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                renderer.render_to_view(device, queue, &snap, &view, config.width, config.height);
                frame.present();
            }
            Some(Backend::Cpu { surface, renderer, .. }) => {
                let pb = renderer.render(&snap);
                let (w, h) = (pb.width.max(1), pb.height.max(1));
                if let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) {
                    let _ = surface.resize(nw, nh);
                }
                if let Ok(mut buf) = surface.buffer_mut() {
                    let n = buf.len().min(pb.data.len());
                    buf[..n].copy_from_slice(&pb.data[..n]);
                    let _ = buf.present();
                }
            }
            None => {}
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|s| s.as_str()) != Some("run") {
        eprintln!("usage: bebop run [--cpu] [--smoke]");
        return;
    }
    let use_cpu = args.iter().any(|a| a == "--cpu");
    let smoke = args.iter().any(|a| a == "--smoke");

    let el = match EventLoop::<UserEvent>::with_user_event().build() {
        Ok(e) => e,
        Err(e) => {
            // No display (headless CI) -> not a failure for a windowed dev binary.
            eprintln!("SMOKE_SKIP: cannot build event loop ({e})");
            return;
        }
    };
    let proxy = el.create_proxy();
    if smoke {
        let p = proxy.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(12));
            let _ = p.send_event(UserEvent::Timeout);
        });
    }
    let mut app = App {
        use_cpu,
        smoke,
        proxy,
        window: None,
        backend: None,
        term: None,
        pty: None,
        pump: None,
        cell_w: 1,
        cell_h: 1,
        done: false,
    };
    el.run_app(&mut app).expect("run app");
}
