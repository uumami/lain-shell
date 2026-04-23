use std::sync::Arc;
use std::time::{Duration, Instant};

use lain_core::renderer::{CellGrid, CpuRenderer, GpuRenderer, Renderer};
use lain_core::terminal::{Terminal, TerminalEvent};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, Modifiers, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use alacritty_terminal::grid::Scroll;
use alacritty_terminal::index::{Column, Direction as Side, Line, Point};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::TermMode;
#[cfg(target_os = "linux")]
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};

/// Target 60fps: coalesce PTY wakeups behind this gate.
const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);
/// Cursor blink half-period.
const BLINK_INTERVAL: Duration = Duration::from_millis(500);

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    cell_grid: Option<CellGrid>,
    terminal: Option<Terminal>,
    event_proxy: winit::event_loop::EventLoopProxy<TerminalEvent>,
    modifiers: Modifiers,
    // Mouse state
    mouse_pos: (f64, f64),
    mouse_button_held: bool,
    active_selection: bool,
    // Frame pacing
    needs_redraw: bool,
    last_frame: Instant,
    // Cursor blink
    cursor_blink_visible: bool,
    cursor_focused: bool,
    next_blink_at: Instant,
}

impl App {
    fn new(event_proxy: winit::event_loop::EventLoopProxy<TerminalEvent>) -> Self {
        Self {
            window: None,
            renderer: None,
            cell_grid: None,
            terminal: None,
            event_proxy,
            modifiers: Modifiers::default(),
            mouse_pos: (0.0, 0.0),
            mouse_button_held: false,
            active_selection: false,
            needs_redraw: false,
            last_frame: Instant::now(),
            cursor_blink_visible: true,
            cursor_focused: true,
            next_blink_at: Instant::now() + BLINK_INTERVAL,
        }
    }
}

fn grid_dimensions(width: u32, height: u32, cell_w: f32, cell_h: f32) -> (u16, u16) {
    let cols = (width as f32 / cell_w).ceil().max(1.0) as u16;
    let rows = (height as f32 / cell_h).ceil().max(1.0) as u16;
    (cols, rows)
}

impl ApplicationHandler<TerminalEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let mut attrs = Window::default_attributes()
            .with_title("λ_ lain-shell")
            .with_inner_size(winit::dpi::LogicalSize::new(640u32, 384u32));
        // Set the Wayland app_id (and X11 WM_CLASS) so the compositor can match
        // this window against lain-shell.desktop and display the registered icon.
        #[cfg(target_os = "linux")]
        {
            use winit::platform::wayland::WindowAttributesExtWayland;
            attrs = attrs.with_name("lain-shell", "lain-shell");
        }
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("failed to create window"),
        );

        let scale_factor = window.scale_factor() as f32;

        // Create cell grid first — its FontSystem is needed for font metric measurement.
        let mut cell_grid = CellGrid::new();

        set_platform_window_icon(&window);

        // Backend selection
        let backend = std::env::var("LAIN_RENDERER").unwrap_or_default();
        let renderer = match backend.as_str() {
            "cpu" => {
                log::info!("Using CPU renderer (explicit)");
                Renderer::Cpu(CpuRenderer::new(
                    window.clone(),
                    scale_factor,
                    &mut cell_grid.font_system,
                ))
            }
            "gpu" => {
                let instance = wgpu::Instance::new(
                    wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
                );
                let gpu = GpuRenderer::new(
                    instance,
                    window.clone(),
                    scale_factor,
                    &mut cell_grid.font_system,
                )
                .expect("LAIN_RENDERER=gpu but no GPU adapter available");
                log::info!("Using GPU renderer (explicit)");
                Renderer::Gpu(gpu)
            }
            _ => {
                // Auto-detect: try GPU first, fall back to CPU
                let instance = wgpu::Instance::new(
                    wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
                );
                match GpuRenderer::new(
                    instance,
                    window.clone(),
                    scale_factor,
                    &mut cell_grid.font_system,
                ) {
                    Some(gpu) => {
                        log::info!("Using GPU renderer (auto-detected)");
                        Renderer::Gpu(gpu)
                    }
                    None => {
                        log::info!("No GPU adapter found, using CPU renderer");
                        Renderer::Cpu(CpuRenderer::new(
                            window.clone(),
                            scale_factor,
                            &mut cell_grid.font_system,
                        ))
                    }
                }
            }
        };

        // Get cell metrics from renderer for grid size calculation
        let (cell_w, cell_h) = renderer.cell_metrics();
        let size = window.inner_size();
        let (cols, rows) = grid_dimensions(size.width, size.height, cell_w, cell_h);

        cell_grid.resize(cols, rows);

        // Create terminal with renderer-derived cell metrics
        let terminal = Terminal::new(
            cols,
            rows,
            cell_w as u16,
            cell_h as u16,
            self.event_proxy.clone(),
        )
        .expect("failed to create terminal");

        self.renderer = Some(renderer);
        self.cell_grid = Some(cell_grid);
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

            WindowEvent::Focused(focused) => {
                self.cursor_focused = focused;
                self.cursor_blink_visible = true;
                if focused {
                    // Restart blink timer on regaining focus.
                    self.next_blink_at = Instant::now() + BLINK_INTERVAL;
                    event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_blink_at));
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let sf = scale_factor as f32;
                if let (Some(renderer), Some(cell_grid)) = (&mut self.renderer, &mut self.cell_grid)
                {
                    // Update font metrics in-place — never recreate the surface.
                    renderer.update_scale(sf, &mut cell_grid.font_system);
                    let (cell_w, cell_h) = renderer.cell_metrics();
                    if let Some(window) = &self.window {
                        let size = window.inner_size();
                        let (cols, rows) =
                            grid_dimensions(size.width, size.height, cell_w, cell_h);
                        cell_grid.resize(cols, rows);
                        if let Some(terminal) = &self.terminal {
                            terminal.resize(cols, rows, cell_w as u16, cell_h as u16);
                        }
                    }
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }

            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
                if let (Some(cell_grid), Some(terminal), Some(renderer)) =
                    (&mut self.cell_grid, &self.terminal, &self.renderer)
                {
                    let (cell_w, cell_h) = renderer.cell_metrics();
                    let (cols, rows) = grid_dimensions(size.width, size.height, cell_w, cell_h);
                    cell_grid.resize(cols, rows);
                    terminal.resize(cols, rows, cell_w as u16, cell_h as u16);
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => {
                let Some(terminal) = &self.terminal else {
                    return;
                };
                let Some(cell_grid) = &mut self.cell_grid else {
                    return;
                };
                let Some(renderer) = &mut self.renderer else {
                    return;
                };

                cell_grid.extract(&terminal.term);
                // Apply cursor blink state
                cell_grid.cursor.visible = self.cursor_blink_visible;

                let font_system = &mut cell_grid.font_system as *mut cosmic_text::FontSystem;
                // Safety: font_system is a separate field from cells/dirty_rows/cursor,
                // so there is no actual aliasing between the &CellGrid and &mut FontSystem.
                let needs_redraw = unsafe { renderer.render_frame(cell_grid, &mut *font_system) };

                self.last_frame = Instant::now();
                self.needs_redraw = false;

                if needs_redraw {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }

                // Schedule next cursor blink tick (only if focused and deadline not already past).
                if self.cursor_focused {
                    event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_blink_at));
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                // Keystroke fast-path: bypass frame gate, render immediately.
                // Reset cursor to visible and restart blink timer.
                self.cursor_blink_visible = true;
                self.next_blink_at = Instant::now() + BLINK_INTERVAL;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }

                if event.state != ElementState::Pressed {
                    return;
                }
                let Some(terminal) = &self.terminal else {
                    return;
                };

                match &event.logical_key {
                    Key::Character(c) => {
                        let text = c.as_str();
                        let ctrl = self.modifiers.state().control_key();
                        let shift = self.modifiers.state().shift_key();

                        // Ctrl+Shift+C → copy selection to CLIPBOARD (do not forward to PTY)
                        if ctrl && shift && text.to_ascii_lowercase() == "c" {
                            let selected = terminal.term.lock().selection_to_string();
                            if let Some(text) = selected {
                                set_clipboard(&text);
                            }
                            return;
                        }

                        // Ctrl+Shift+V → paste CLIPBOARD into PTY (do not forward raw keystroke)
                        if ctrl && shift && text.to_ascii_lowercase() == "v" {
                            if let Some(text) = get_clipboard() {
                                terminal.send_input(text.as_bytes());
                            }
                            return;
                        }

                        // Ctrl+key → control character
                        if ctrl && text.len() == 1 {
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
                        // PageUp / PageDown → scroll viewport, do NOT forward to PTY
                        if *named == NamedKey::PageUp || *named == NamedKey::PageDown {
                            let Some(terminal) = &self.terminal else {
                                return;
                            };
                            let scroll = if *named == NamedKey::PageUp {
                                Scroll::PageUp
                            } else {
                                Scroll::PageDown
                            };
                            terminal.term.lock().scroll_display(scroll);
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                            return;
                        }

                        let bytes: &[u8] = match named {
                            NamedKey::Enter => b"\r",
                            NamedKey::Backspace => b"\x7f",
                            NamedKey::Tab => b"\t",
                            NamedKey::Escape => b"\x1b",
                            NamedKey::Space => b" ",
                            NamedKey::ArrowUp => b"\x1b[A",
                            NamedKey::ArrowDown => b"\x1b[B",
                            NamedKey::ArrowRight => b"\x1b[C",
                            NamedKey::ArrowLeft => b"\x1b[D",
                            NamedKey::Home => b"\x1b[H",
                            NamedKey::End => b"\x1b[F",
                            NamedKey::Delete => b"\x1b[3~",
                            _ => return,
                        };
                        terminal.send_input(bytes);
                    }
                    _ => {}
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let Some(terminal) = &self.terminal else {
                    return;
                };
                let Some(renderer) = &self.renderer else {
                    return;
                };

                // Positive = scroll up (toward history), negative = scroll down
                let lines: f32 = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 20.0,
                };

                let mode = *terminal.term.lock().mode();
                if mode.intersects(TermMode::MOUSE_MODE) {
                    // Forward to PTY as mouse button 64 (up) or 65 (down)
                    let (cell_w, cell_h) = renderer.cell_metrics();
                    let col = (self.mouse_pos.0 / cell_w as f64) as usize;
                    let row = (self.mouse_pos.1 / cell_h as f64) as usize;
                    let sgr = mode.contains(TermMode::SGR_MOUSE);
                    let ticks = (lines.abs().ceil() as usize).max(1);
                    let btn = if lines > 0.0 { 64u8 } else { 65u8 };
                    for _ in 0..ticks {
                        let seq = encode_mouse_event(btn, col, row, true, sgr);
                        terminal.send_input(&seq);
                    }
                } else {
                    // Scroll the viewport
                    let ticks = (lines.abs().ceil() as i32).max(1);
                    let delta_lines = if lines > 0.0 { ticks * 3 } else { -(ticks * 3) };
                    terminal
                        .term
                        .lock()
                        .scroll_display(Scroll::Delta(delta_lines));
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_pos = (position.x, position.y);

                if !self.mouse_button_held {
                    return;
                }

                let Some(terminal) = &self.terminal else {
                    return;
                };
                let Some(renderer) = &self.renderer else {
                    return;
                };
                let Some(cell_grid) = &self.cell_grid else {
                    return;
                };
                let (cell_w, cell_h) = renderer.cell_metrics();
                let mode = *terminal.term.lock().mode();

                if mode.contains(TermMode::MOUSE_MOTION) {
                    // Forward motion to PTY
                    let col = (position.x / cell_w as f64) as usize;
                    let row = (position.y / cell_h as f64) as usize;
                    let sgr = mode.contains(TermMode::SGR_MOUSE);
                    // Button 32 = left button motion
                    let seq = encode_mouse_event(32, col, row, true, sgr);
                    terminal.send_input(&seq);
                } else if self.active_selection {
                    // Update selection
                    let col = (position.x / cell_w as f64) as usize;
                    let display_row = (position.y / cell_h as f64) as usize;
                    let point = display_to_term_point(col, display_row, cell_grid.display_offset);
                    let mut term = terminal.term.lock();
                    if let Some(sel) = term.selection.as_mut() {
                        sel.update(point, Side::Right);
                    }
                    drop(term);
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                let Some(terminal) = &self.terminal else {
                    return;
                };
                let Some(renderer) = &self.renderer else {
                    return;
                };
                let Some(cell_grid) = &self.cell_grid else {
                    return;
                };

                let (cell_w, cell_h) = renderer.cell_metrics();
                let col = (self.mouse_pos.0 / cell_w as f64) as usize;
                let row_display = (self.mouse_pos.1 / cell_h as f64) as usize;
                let pressed = state == ElementState::Pressed;

                let mode = *terminal.term.lock().mode();

                if mode.contains(TermMode::MOUSE_REPORT_CLICK) {
                    // Forward to PTY
                    let btn = match button {
                        MouseButton::Left => 0u8,
                        MouseButton::Middle => 1u8,
                        MouseButton::Right => 2u8,
                        _ => return,
                    };
                    let sgr = mode.contains(TermMode::SGR_MOUSE);
                    let encoded = if sgr {
                        encode_mouse_event(btn, col, row_display, pressed, true)
                    } else {
                        let b = if pressed { btn } else { 3u8 };
                        encode_mouse_event(b, col, row_display, pressed, false)
                    };
                    terminal.send_input(&encoded);
                    self.mouse_button_held = pressed;
                } else if button == MouseButton::Left {
                    if pressed {
                        // Begin selection
                        let point =
                            display_to_term_point(col, row_display, cell_grid.display_offset);
                        terminal.term.lock().selection =
                            Some(Selection::new(SelectionType::Simple, point, Side::Left));
                        self.mouse_button_held = true;
                        self.active_selection = true;
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    } else {
                        // Finalize selection → write to PRIMARY
                        self.mouse_button_held = false;
                        if self.active_selection {
                            let selected = terminal.term.lock().selection_to_string();
                            if let Some(text) = selected {
                                set_primary(&text);
                            }
                            self.active_selection = false;
                        }
                    }
                } else if button == MouseButton::Middle && pressed {
                    // Middle-click paste from PRIMARY
                    if let Some(text) = get_primary() {
                        terminal.send_input(text.as_bytes());
                    }
                }
            }

            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: TerminalEvent) {
        match event {
            TerminalEvent::Wakeup => {
                // Frame gate: coalesce wakeups, render at most once per FRAME_INTERVAL
                self.needs_redraw = true;
                let now = Instant::now();
                if now.duration_since(self.last_frame) >= FRAME_INTERVAL {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                } else {
                    event_loop
                        .set_control_flow(ControlFlow::WaitUntil(self.last_frame + FRAME_INTERVAL));
                }
            }
            TerminalEvent::Exit => {
                event_loop.exit();
            }
            TerminalEvent::PtyWrite(s) => {
                if let Some(terminal) = &self.terminal {
                    terminal.send_input(s.as_bytes());
                }
            }
            TerminalEvent::Title(title) => {
                if let Some(window) = &self.window {
                    window.set_title(&title);
                }
            }
            TerminalEvent::TextAreaSizeRequest(cb) => {
                if let (Some(terminal), Some(renderer), Some(cell_grid)) =
                    (&self.terminal, &self.renderer, &self.cell_grid)
                {
                    let (cell_w, cell_h) = renderer.cell_metrics();
                    let window_size = alacritty_terminal::event::WindowSize {
                        num_lines: cell_grid.rows,
                        num_cols: cell_grid.cols,
                        cell_width: cell_w as u16,
                        cell_height: cell_h as u16,
                    };
                    let response = cb(window_size);
                    terminal.send_input(response.as_bytes());
                }
            }
            TerminalEvent::ClipboardStore(text) => {
                set_clipboard(&text);
            }
            TerminalEvent::ClipboardLoad(cb) => {
                if let Some(terminal) = &self.terminal {
                    let text = get_clipboard().unwrap_or_default();
                    let response = cb(&text);
                    terminal.send_input(response.as_bytes());
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Timer fired (WaitUntil elapsed with no OS event).
        // Handles both the frame gate and the cursor blink tick.
        let now = Instant::now();

        // Frame gate: pending redraw and the interval has elapsed.
        if self.needs_redraw && now.duration_since(self.last_frame) >= FRAME_INTERVAL {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            // Don't toggle blink here — fall through so we reschedule correctly.
        }

        // Cursor blink: only toggle when the dedicated blink deadline is reached.
        // This prevents rapid blinking when about_to_wait fires early for the frame gate.
        if self.cursor_focused && now >= self.next_blink_at {
            self.cursor_blink_visible = !self.cursor_blink_visible;
            self.next_blink_at = now + BLINK_INTERVAL;
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }

        // Reschedule for whichever deadline comes soonest.
        if self.cursor_focused || self.needs_redraw {
            let next = if self.cursor_focused && self.needs_redraw {
                self.next_blink_at.min(self.last_frame + FRAME_INTERVAL)
            } else if self.cursor_focused {
                self.next_blink_at
            } else {
                self.last_frame + FRAME_INTERVAL
            };
            event_loop.set_control_flow(ControlFlow::WaitUntil(next));
        }
    }
}

// ── Clipboard helpers ────────────────────────────────────────────────────────

fn set_clipboard(text: &str) {
    if let Ok(mut cb) = arboard::Clipboard::new() {
        #[cfg(target_os = "linux")]
        {
            let _ = cb.set().text(text.to_owned());
        }
        #[cfg(not(target_os = "linux"))]
        let _ = cb.set_text(text.to_owned());
    }
}

fn get_clipboard() -> Option<String> {
    arboard::Clipboard::new().ok()?.get_text().ok()
}

#[cfg(target_os = "linux")]
fn set_primary(text: &str) {
    use arboard::{LinuxClipboardKind, SetExtLinux};
    if let Ok(mut cb) = arboard::Clipboard::new() {
        let _ = cb.set().clipboard(LinuxClipboardKind::Primary).text(text.to_owned());
    }
}

#[cfg(not(target_os = "linux"))]
fn set_primary(text: &str) {
    // PRIMARY selection not supported on this platform; fall back to CLIPBOARD
    set_clipboard(text);
}

#[cfg(target_os = "linux")]
fn get_primary() -> Option<String> {
    use arboard::{GetExtLinux, LinuxClipboardKind};
    arboard::Clipboard::new()
        .ok()?
        .get()
        .clipboard(LinuxClipboardKind::Primary)
        .text()
        .ok()
}

#[cfg(not(target_os = "linux"))]
fn get_primary() -> Option<String> {
    get_clipboard()
}

// ── Mouse encoding ───────────────────────────────────────────────────────────

/// Encode a mouse event as X10 or SGR escape sequence.
/// `button`: 0=left, 1=middle, 2=right, 3=release(X10), 32=left-motion,
///           64=wheel-up, 65=wheel-down
/// Coordinates are 0-based; the encoding adds 1 for 1-based terminal coords.
fn encode_mouse_event(button: u8, col: usize, row: usize, pressed: bool, sgr: bool) -> Vec<u8> {
    if sgr {
        let suffix = if pressed { 'M' } else { 'm' };
        format!("\x1b[<{};{};{}{}", button, col + 1, row + 1, suffix).into_bytes()
    } else {
        // X10: ESC [ M Cb Cx Cy — each value offset by 32 to land in printable range
        let b = button.saturating_add(32);
        let x = ((col + 1 + 32) as u8).min(255);
        let y = ((row + 1 + 32) as u8).min(255);
        vec![0x1b, b'[', b'M', b, x, y]
    }
}

// ── Coordinate helpers ───────────────────────────────────────────────────────

/// Convert a display-coordinate (col, row) to an alacritty_terminal Point,
/// accounting for the current scroll offset.
fn display_to_term_point(col: usize, display_row: usize, display_offset: usize) -> Point {
    let line = Line(display_row as i32 - display_offset as i32);
    let column = Column(col);
    Point::new(line, column)
}

// ── Window icon: pixel art, PNG encoding, freedesktop registration ───────────

/// 32×32 source art for the λ_ icon.
/// The lambda is intentionally asymmetric so it reads as `λ` rather than a chevron,
/// with a separate underscore on the lower-right.
/// '#' = terminal green (0, 230, 115, 255); '.' = transparent (0, 0, 0, 0).
#[rustfmt::skip]
const ART_32: &[&[u8]] = &[
    b"....###.........................",
    b"....###.........................",
    b"....###.........................",
    b".....###........................",
    b".....###........................",
    b".....###........................",
    b"......###.......................",
    b"......###.......................",
    b"......###.......................",
    b".....####.......................",
    b".....#####......................",
    b".....#####......................",
    b"....######......................",
    b"....###.###.....................",
    b"....###.###.....................",
    b"....##..###.....................",
    b"...###..###.....................",
    b"...###...###....................",
    b"...###...###....................",
    b"..###....###....................",
    b"..###.....###...................",
    b"..###.....###...................",
    b"..###.....###...................",
    b".###......###...................",
    b".###.......###..................",
    b".###.......###..................",
    b"###........###..................",
    b"###.........###.................",
    b"................................",
    b"..................##############",
    b"..................##############",
    b"..................##############",
];

/// 16×16 hand-crafted art for the λ_ icon.
/// Tuned for GNOME's tiny app-grid rendering, where asymmetry matters more than detail.
#[rustfmt::skip]
const ART_16: &[&[u8]] = &[
    b"..##............",
    b"..##............",
    b"...#............",
    b"...#............",
    b"...##...........",
    b"..###...........",
    b"..###...........",
    b"..#.##..........",
    b"..#.##..........",
    b".##..#..........",
    b".##..#..........",
    b".#...##.........",
    b"##...##.........",
    b"##....#.........",
    b".........#######",
    b".........#######",
];

/// Rasterise a `&[&[u8]]` art definition into RGBA bytes.
/// '#' → green (0, 230, 115, 255); anything else → transparent.
fn art_to_rgba(art: &[&[u8]], canvas_size: u32) -> Vec<u8> {
    let mut rgba = vec![0u8; (canvas_size * canvas_size * 4) as usize];
    for (row, line) in art.iter().enumerate() {
        let py = row as u32;
        if py >= canvas_size {
            break;
        }
        for (col, &byte) in line.iter().enumerate() {
            if byte == b'#' {
                let px = col as u32;
                if px >= canvas_size {
                    continue;
                }
                let i = (py * canvas_size + px) as usize * 4;
                rgba[i] = 0;
                rgba[i + 1] = 230;
                rgba[i + 2] = 115;
                rgba[i + 3] = 255;
            }
        }
    }
    rgba
}

/// Nearest-neighbor upscale: each source pixel becomes a `scale`×`scale` block.
fn scale_rgba(src: &[u8], src_size: u32, scale: u32) -> Vec<u8> {
    let dst_size = src_size * scale;
    let mut out = vec![0u8; (dst_size * dst_size * 4) as usize];
    for y in 0..dst_size {
        for x in 0..dst_size {
            let si = ((y / scale) * src_size + (x / scale)) as usize * 4;
            let di = (y * dst_size + x) as usize * 4;
            out[di..di + 4].copy_from_slice(&src[si..si + 4]);
        }
    }
    out
}

/// Nearest-neighbor resize to an arbitrary square size.
/// Handles non-integer scale factors (e.g. 32→48).
fn resize_rgba(src: &[u8], src_size: u32, dst_size: u32) -> Vec<u8> {
    let mut out = vec![0u8; (dst_size * dst_size * 4) as usize];
    for y in 0..dst_size {
        let sy = y * src_size / dst_size;
        for x in 0..dst_size {
            let sx = x * src_size / dst_size;
            let si = (sy * src_size + sx) as usize * 4;
            let di = (y * dst_size + x) as usize * 4;
            out[di..di + 4].copy_from_slice(&src[si..si + 4]);
        }
    }
    out
}

/// Build 32×32 RGBA bytes from the canonical pixel art.
fn make_icon_rgba() -> Vec<u8> {
    art_to_rgba(ART_32, 32)
}

/// Build 16×16 RGBA bytes from the hand-crafted small pixel art.
fn make_icon_rgba_16() -> Vec<u8> {
    art_to_rgba(ART_16, 16)
}

/// Wrap the icon in a winit `Icon` for the in-process window decoration.
/// Uses a 128×128 upscale so non-X11 backends still get a single large source.
fn make_window_icon() -> Option<winit::window::Icon> {
    let rgba_128 = scale_rgba(&make_icon_rgba(), 32, 4);
    winit::window::Icon::from_rgba(rgba_128, 128, 128).ok()
}

/// Apply the best available per-window icon path for the current backend.
///
/// On X11 we bypass winit's single-size icon API and write `_NET_WM_ICON`
/// ourselves with four sizes. On Wayland and other backends we keep the
/// existing single-icon path.
fn set_platform_window_icon(window: &Window) {
    #[cfg(target_os = "linux")]
    {
        match try_set_x11_window_icon(window) {
            Ok(SetWindowIcon::AppliedX11) => {
                log::info!("Applied multi-size _NET_WM_ICON via direct X11 property write");
                return;
            }
            Ok(SetWindowIcon::NotX11) => {
                log::info!("Window backend is not X11; using winit window icon fallback");
            }
            Err(err) => {
                log::warn!("Direct X11 _NET_WM_ICON write failed: {err}");
            }
        }
    }

    window.set_window_icon(make_window_icon());
    log::info!("Applied winit single-size window icon fallback");
}

#[cfg(target_os = "linux")]
enum SetWindowIcon {
    AppliedX11,
    NotX11,
}

#[cfg(target_os = "linux")]
fn try_set_x11_window_icon(window: &Window) -> Result<SetWindowIcon, Box<dyn std::error::Error>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, PropMode};
    use x11rb::rust_connection::RustConnection;
    use x11rb::wrapper::ConnectionExt as _;

    match window.display_handle()?.as_raw() {
        RawDisplayHandle::Xlib(_) => {}
        _ => return Ok(SetWindowIcon::NotX11),
    }

    let window_id = match window.window_handle()?.as_raw() {
        RawWindowHandle::Xlib(handle) => handle.window as u32,
        _ => return Ok(SetWindowIcon::NotX11),
    };

    let (conn, _) = RustConnection::connect(None)?;
    let icon_atom = conn.intern_atom(false, b"_NET_WM_ICON")?.reply()?.atom;
    let payload = make_x11_window_icon_cardinals();
    conn.change_property32(
        PropMode::REPLACE,
        window_id,
        icon_atom,
        AtomEnum::CARDINAL,
        &payload,
    )?
    .check()?;
    conn.flush()?;
    Ok(SetWindowIcon::AppliedX11)
}

/// Build the `_NET_WM_ICON` cardinals array as `[w, h, pixels...]` blocks.
fn make_x11_window_icon_cardinals() -> Vec<u32> {
    let base_rgba = make_icon_rgba();
    let rgba_16 = make_icon_rgba_16();
    let rgba_48 = resize_rgba(&base_rgba, 32, 48);
    let rgba_128 = scale_rgba(&base_rgba, 32, 4);

    let mut cardinals = Vec::new();
    append_x11_icon_block(&mut cardinals, 128, &rgba_128);
    append_x11_icon_block(&mut cardinals, 48, &rgba_48);
    append_x11_icon_block(&mut cardinals, 32, &base_rgba);
    append_x11_icon_block(&mut cardinals, 16, &rgba_16);
    cardinals
}

fn append_x11_icon_block(out: &mut Vec<u32>, size: u32, rgba: &[u8]) {
    out.push(size);
    out.push(size);
    for pixel in rgba.chunks_exact(4) {
        let [r, g, b, a] = [pixel[0], pixel[1], pixel[2], pixel[3]];
        out.push((u32::from(a) << 24) | (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b));
    }
}

/// Minimal self-contained PNG encoder — no external crate required.
/// Produces a valid 32-bit RGBA PNG using uncompressed deflate (stored blocks).
fn encode_png(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    fn crc32(data: &[u8]) -> u32 {
        let mut table = [0u32; 256];
        for (i, entry) in table.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                if c & 1 != 0 {
                    c = 0xEDB8_8320 ^ (c >> 1);
                } else {
                    c >>= 1;
                }
            }
            *entry = c;
        }
        let mut crc = 0xFFFF_FFFFu32;
        for &b in data {
            crc = table[((crc ^ u32::from(b)) & 0xFF) as usize] ^ (crc >> 8);
        }
        crc ^ 0xFFFF_FFFF
    }

    fn adler32(data: &[u8]) -> u32 {
        let (mut s1, mut s2) = (1u32, 0u32);
        for &b in data {
            s1 = (s1 + u32::from(b)) % 65521;
            s2 = (s2 + s1) % 65521;
        }
        (s2 << 16) | s1
    }

    fn chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(tag);
        out.extend_from_slice(data);
        let mut crc_buf = Vec::with_capacity(tag.len() + data.len());
        crc_buf.extend_from_slice(tag);
        crc_buf.extend_from_slice(data);
        out.extend_from_slice(&crc32(&crc_buf).to_be_bytes());
    }

    let mut out = Vec::new();

    // PNG signature
    out.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    // IHDR: width, height, 8-bit RGBA
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // bit_depth=8, color_type=RGBA, compress=0, filter=0, interlace=0
    chunk(&mut out, b"IHDR", &ihdr);

    // Filter-byte-prefixed scanlines (filter type 0 = None)
    let row_stride = (w * 4) as usize;
    let mut scanlines = Vec::with_capacity(h as usize * (1 + row_stride));
    for row in 0..h as usize {
        scanlines.push(0u8); // filter byte
        scanlines.extend_from_slice(&rgba[row * row_stride..(row + 1) * row_stride]);
    }

    // IDAT: zlib header + stored deflate block(s) + Adler-32
    //   CMF=0x78 (deflate, 32KB window), FLG=0x01 — (0x78*256+0x01) % 31 == 0
    let adler = adler32(&scanlines);
    let mut idat = Vec::new();
    idat.extend_from_slice(&[0x78, 0x01]);

    let mut offset = 0;
    while offset < scanlines.len() {
        let end = (offset + 65535).min(scanlines.len());
        let is_last = end == scanlines.len();
        let block = &scanlines[offset..end];
        let len = block.len() as u16;
        idat.push(if is_last { 0x01 } else { 0x00 }); // BFINAL | BTYPE=00
        idat.extend_from_slice(&len.to_le_bytes());
        idat.extend_from_slice(&(!len).to_le_bytes()); // NLEN = ones-complement of LEN
        idat.extend_from_slice(block);
        offset = end;
    }
    idat.extend_from_slice(&adler.to_be_bytes());

    chunk(&mut out, b"IDAT", &idat);
    chunk(&mut out, b"IEND", b"");
    out
}

/// Register a `.desktop` file and icon PNG so the desktop environment can show
/// the app icon in shell surfaces such as the taskbar and Alt+Tab switcher.
/// Compositor-specific titlebar rendering remains environment-dependent.
///
/// Writes to:
///   ~/.local/share/icons/hicolor/32x32/apps/lain-shell.png
///   ~/.local/share/applications/lain-shell.desktop
///
/// Files are regenerated whenever their content would differ from the current
/// expected content (e.g. after a binary move or icon change). The check is
/// a cheap string comparison and runs on every launch.
fn try_register_desktop_icon() {
    let Some(home_os) = std::env::var_os("HOME") else {
        return;
    };
    let home = std::path::PathBuf::from(home_os);

    // --- Icon PNGs at all four hicolor sizes --------------------------------
    // 48×48 is a standard freedesktop size; 32→48 uses the general nearest-
    // neighbor resize (non-integer factor). 128×128 uses the integer 4× upscale.
    let base_rgba = make_icon_rgba();
    let rgba_16 = make_icon_rgba_16();
    let rgba_48 = resize_rgba(&base_rgba, 32, 48);
    let rgba_128 = scale_rgba(&base_rgba, 32, 4);

    let icon_sizes: [(u32, &[u8], &str); 4] = [
        (16, &rgba_16, "16x16"),
        (32, &base_rgba, "32x32"),
        (48, &rgba_48, "48x48"),
        (128, &rgba_128, "128x128"),
    ];

    let mut any_icon_written = false;
    for (size, rgba, dir_name) in icon_sizes {
        let icon_dir = home
            .join(".local/share/icons/hicolor")
            .join(dir_name)
            .join("apps");
        let icon_path = icon_dir.join("lain-shell.png");
        let png = encode_png(rgba, size, size);

        let needs_write = std::fs::read(&icon_path)
            .map(|existing| existing != png)
            .unwrap_or(true);

        if needs_write {
            if std::fs::create_dir_all(&icon_dir).is_ok() {
                if std::fs::write(&icon_path, &png).is_ok() {
                    any_icon_written = true;
                }
            }
        }
    }

    if any_icon_written {
        // Refresh icon cache — not installed everywhere; ignore failures.
        let _ = std::process::Command::new("gtk-update-icon-cache")
            .args(["--force", "--ignore-theme-index"])
            .arg(home.join(".local/share/icons/hicolor"))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }

    // --- .desktop file ----------------------------------------------------
    let desktop_dir = home.join(".local/share/applications");
    let desktop_path = desktop_dir.join("lain-shell.desktop");

    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "lain-shell".to_owned());

    // StartupWMClass must match our WM_CLASS so GNOME Shell can associate the
    // running window with this desktop entry and display the correct icon.
    let desktop_content = format!(
        "[Desktop Entry]\nType=Application\nName=lain-shell\nExec={exe}\nIcon=lain-shell\nTerminal=false\nCategories=System;TerminalEmulator;\nStartupWMClass=lain-shell\n"
    );

    let needs_desktop_write = std::fs::read_to_string(&desktop_path)
        .map(|existing| existing != desktop_content)
        .unwrap_or(true);

    if needs_desktop_write {
        if std::fs::create_dir_all(&desktop_dir).is_ok() {
            if std::fs::write(&desktop_path, desktop_content.as_bytes()).is_ok() {
                // Rebuild the XDG desktop database so the shell discovers the entry.
                let _ = std::process::Command::new("update-desktop-database")
                    .arg(&desktop_dir)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            }
        }
    }
}

fn main() {
    env_logger::init();
    try_register_desktop_icon();
    let event_loop = EventLoop::<TerminalEvent>::with_user_event()
        .build()
        .expect("failed to create event loop");
    let proxy = event_loop.create_proxy();
    let mut app = App::new(proxy);
    event_loop.run_app(&mut app).expect("event loop error");
}
