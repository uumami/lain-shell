//! `bebop run` -- dev binary: one window running $SHELL through the BEBOP core.
//! Reactive (ControlFlow::Wait), woken by PTY bytes via EventLoopProxy. Exercises
//! the GPU (wgpu+glyphon -> surface) and CPU (softbuffer) present paths in the
//! real crate. Input goes through the neutral KeyInput/Terminal::on_input seam
//! (xterm key encoding); action bindings (copy/paste/scroll) are a later plan.
//! Not the NAVI host; just a test vehicle until NAVI exists.
//!
//! Usage: bebop run [--cpu] [--smoke] [--config path]

use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use lain_types::ByteStream;
use terminal_core::{
    cell_size, config_rejected_notice, load_from_path, ConfigReload, ConfigReloader,
    CpuRenderer, DegradedReason, GpuRenderer, InputOutcome, Key, KeyInput, LocalPty, Modifiers,
    NamedKey, Notice, NoticeAction, NoticeCode, ReaderPump, RenderBackendPreference, Renderer,
    Severity, TermConfig, TermStatus, Terminal,
};

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::Modifiers as WinitModifiers;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key as WinitKey, NamedKey as WinitNamed};
use winit::window::{Window, WindowId};

#[derive(Debug, Clone, Copy)]
enum UserEvent {
    Pty,
    ConfigChanged,
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
    mods: WinitModifiers,
    config: TermConfig,
    reloader: Option<ConfigReloader>,
    startup_notices: Vec<Notice>,
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
        let (cw, ch) = cell_size(self.config.font.size);
        self.cell_w = cw.max(1);
        self.cell_h = ch.max(1);
        let cols = (size.width / self.cell_w).max(1) as usize;
        let lines = (size.height / self.cell_h).max(1) as usize;

        // PTY + core. Move the single reader into the pump; wake the loop per chunk.
        let shell = self
            .config
            .shell
            .clone()
            .or_else(|| std::env::var("SHELL").ok())
            .unwrap_or_else(|| "/bin/sh".into());
        let mut pty = LocalPty::spawn(&shell, cols as u16, lines as u16).expect("spawn shell");
        let reader = pty.take_reader();
        let wake_proxy = self.proxy.clone();
        let pump = ReaderPump::start_with_waker(reader, 64, 65536, move || {
            let _ = wake_proxy.send_event(UserEvent::Pty);
        });
        let mut term = Terminal::new(cols, lines);
        for notice in self.startup_notices.drain(..) {
            record_notice(&mut term, notice);
        }

        let backend = self.create_backend(window.clone(), size, &mut term);

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
            UserEvent::ConfigChanged => {
                self.reload_config();
                if let Some(w) = self.window.as_ref() {
                    w.request_redraw();
                }
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
            WindowEvent::ModifiersChanged(m) => {
                self.mods = m;
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                let key = match &event.logical_key {
                    // Space arrives as a named key but is a printable char to the PTY.
                    WinitKey::Named(WinitNamed::Space) => Some(Key::Char(' ')),
                    WinitKey::Named(n) => translate_named(*n).map(Key::Named),
                    WinitKey::Character(s) => s.chars().next().map(Key::Char),
                    _ => None,
                };
                if let Some(key) = key {
                    let st = self.mods.state();
                    let ki = KeyInput {
                        key,
                        mods: Modifiers {
                            shift: st.shift_key(),
                            alt: st.alt_key(),
                            ctrl: st.control_key(),
                            logo: st.super_key(),
                        },
                    };
                    if let Some(t) = self.term.as_ref() {
                        let outcome = t.on_input(&ki);
                        if let InputOutcome::Bytes(bytes) = outcome {
                            if let Some(p) = self.pty.as_mut() {
                                let _ = p.write(&bytes);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

impl App {
    fn create_backend(
        &self,
        window: Arc<Window>,
        size: winit::dpi::PhysicalSize<u32>,
        term: &mut Terminal,
    ) -> Backend {
        if self.use_cpu || self.config.render_backend == RenderBackendPreference::Cpu {
            return create_cpu_backend(window, self.config.font.size);
        }

        match try_create_gpu_backend(
            window.clone(),
            size,
            self.config.font.size,
            self.config.vsync,
        ) {
            Ok(backend) => backend,
            Err(err) => {
                record_notice(term, gpu_fallback_notice(err));
                term.set_status(TermStatus::Degraded { reason: DegradedReason::GpuUnavailable });
                create_cpu_backend(window, self.config.font.size)
            }
        }
    }

    fn reload_config(&mut self) {
        let reload = self.reloader.as_mut().map(ConfigReloader::reload_if_changed);
        match reload {
            Some(ConfigReload::Applied(cfg)) => self.apply_config(cfg),
            Some(ConfigReload::Rejected(notice)) => {
                if let Some(t) = self.term.as_mut() {
                    record_notice(t, notice);
                }
            }
            Some(ConfigReload::Unchanged) | None => {}
        }
    }

    fn apply_config(&mut self, cfg: TermConfig) {
        let old_font_size = self.config.font.size;
        self.config = cfg;
        if (self.config.font.size - old_font_size).abs() > f32::EPSILON {
            self.rebuild_renderer_for_font();
        }
    }

    fn rebuild_renderer_for_font(&mut self) {
        let font_size = self.config.font.size;
        let (cw, ch) = cell_size(font_size);
        self.cell_w = cw.max(1);
        self.cell_h = ch.max(1);
        match self.backend.as_mut() {
            Some(Backend::Cpu { renderer, .. }) => {
                **renderer = CpuRenderer::new(font_size);
            }
            Some(Backend::Gpu { device, queue, config, renderer, .. }) => {
                **renderer = GpuRenderer::new(device, queue, config.format, font_size);
            }
            None => {}
        }
        if let Some(w) = self.window.as_ref() {
            let size = w.inner_size();
            self.resize(size.width, size.height);
        }
    }

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

fn create_cpu_backend(window: Arc<Window>, font_size: f32) -> Backend {
    let context = softbuffer::Context::new(window.clone()).expect("softbuffer context");
    let surface = softbuffer::Surface::new(&context, window).expect("softbuffer surface");
    Backend::Cpu { _context: context, surface, renderer: Box::new(CpuRenderer::new(font_size)) }
}

fn try_create_gpu_backend(
    window: Arc<Window>,
    size: winit::dpi::PhysicalSize<u32>,
    font_size: f32,
    vsync: bool,
) -> Result<Backend, String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
    let surface = instance.create_surface(window).map_err(|e| format!("create surface: {e}"))?;
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::default(),
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    }))
    .ok_or_else(|| "no wgpu adapter".to_string())?;
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
    .map_err(|e| format!("request device: {e}"))?;
    let caps = surface.get_capabilities(&adapter);
    let format = caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(caps.formats[0]);
    let present_mode = if vsync {
        wgpu::PresentMode::Fifo
    } else if caps.present_modes.contains(&wgpu::PresentMode::Immediate) {
        wgpu::PresentMode::Immediate
    } else {
        wgpu::PresentMode::Fifo
    };
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode,
        desired_maximum_frame_latency: 2,
        alpha_mode: caps.alpha_modes[0],
        view_formats: vec![],
    };
    surface.configure(&device, &config);
    let renderer = Box::new(GpuRenderer::new(&device, &queue, format, font_size));
    Ok(Backend::Gpu { surface: Box::new(surface), device, queue, config, renderer })
}

fn gpu_fallback_notice(err: String) -> Notice {
    Notice {
        severity: Severity::Warn,
        code: NoticeCode::GpuFallback,
        message: format!("running on CPU renderer; GPU unavailable: {err}"),
        action: Some(NoticeAction::Dismiss),
    }
}

fn record_notice(term: &mut Terminal, notice: Notice) {
    eprintln!("BEBOP_NOTICE {:?}: {}", notice.code, notice.message);
    term.push_notice(notice);
}

/// Map the subset of winit named keys this cycle encodes into the neutral
/// `NamedKey`. Unmapped keys return `None` (the press is ignored for now).
fn translate_named(n: WinitNamed) -> Option<NamedKey> {
    Some(match n {
        WinitNamed::Enter => NamedKey::Enter,
        WinitNamed::Tab => NamedKey::Tab,
        WinitNamed::Backspace => NamedKey::Backspace,
        WinitNamed::Escape => NamedKey::Escape,
        WinitNamed::ArrowUp => NamedKey::Up,
        WinitNamed::ArrowDown => NamedKey::Down,
        WinitNamed::ArrowLeft => NamedKey::Left,
        WinitNamed::ArrowRight => NamedKey::Right,
        WinitNamed::Home => NamedKey::Home,
        WinitNamed::End => NamedKey::End,
        WinitNamed::PageUp => NamedKey::PageUp,
        WinitNamed::PageDown => NamedKey::PageDown,
        WinitNamed::Insert => NamedKey::Insert,
        WinitNamed::Delete => NamedKey::Delete,
        WinitNamed::F1 => NamedKey::F(1),
        WinitNamed::F2 => NamedKey::F(2),
        WinitNamed::F3 => NamedKey::F(3),
        WinitNamed::F4 => NamedKey::F(4),
        WinitNamed::F5 => NamedKey::F(5),
        WinitNamed::F6 => NamedKey::F(6),
        WinitNamed::F7 => NamedKey::F(7),
        WinitNamed::F8 => NamedKey::F(8),
        WinitNamed::F9 => NamedKey::F(9),
        WinitNamed::F10 => NamedKey::F(10),
        WinitNamed::F11 => NamedKey::F(11),
        WinitNamed::F12 => NamedKey::F(12),
        _ => return None,
    })
}

fn config_path(args: &[String]) -> Result<Option<PathBuf>, String> {
    let mut idx = 2;
    while idx < args.len() {
        if args[idx] == "--config" {
            let path = args
                .get(idx + 1)
                .ok_or_else(|| "--config requires a path".to_string())?;
            return Ok(Some(PathBuf::from(path)));
        }
        idx += 1;
    }
    Ok(None)
}

fn load_startup_config(path: Option<PathBuf>) -> (TermConfig, Option<ConfigReloader>, Vec<Notice>) {
    let default = TermConfig::default();
    match path {
        Some(path) => match load_from_path(&path) {
            Ok(cfg) => (cfg.clone(), Some(ConfigReloader::new(path, cfg)), Vec::new()),
            Err(err) => {
                let notice = config_rejected_notice(err);
                (default.clone(), Some(ConfigReloader::new(path, default)), vec![notice])
            }
        },
        None => (default, None, Vec::new()),
    }
}

fn start_config_watcher(path: PathBuf, proxy: EventLoopProxy<UserEvent>) {
    std::thread::spawn(move || {
        let mut last = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        loop {
            std::thread::sleep(Duration::from_millis(500));
            let now = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
            if now.is_some() && now != last {
                last = now;
                if proxy.send_event(UserEvent::ConfigChanged).is_err() {
                    break;
                }
            }
        }
    });
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|s| s.as_str()) != Some("run") {
        eprintln!("usage: bebop run [--cpu] [--smoke] [--config path]");
        return;
    }
    let use_cpu = args.iter().any(|a| a == "--cpu");
    let smoke = args.iter().any(|a| a == "--smoke");
    let config_path = match config_path(&args) {
        Ok(path) => path,
        Err(err) => {
            eprintln!("{err}");
            return;
        }
    };
    let (config, reloader, startup_notices) = load_startup_config(config_path.clone());

    let el = match EventLoop::<UserEvent>::with_user_event().build() {
        Ok(e) => e,
        Err(e) => {
            // No display (headless CI) -> not a failure for a windowed dev binary.
            eprintln!("SMOKE_SKIP: cannot build event loop ({e})");
            return;
        }
    };
    let proxy = el.create_proxy();
    if let Some(path) = config_path {
        start_config_watcher(path, proxy.clone());
    }
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
        mods: WinitModifiers::default(),
        config,
        reloader,
        startup_notices,
    };
    el.run_app(&mut app).expect("run app");
}
