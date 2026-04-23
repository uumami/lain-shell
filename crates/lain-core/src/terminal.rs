use std::fmt;
use std::sync::Arc;
use std::thread::JoinHandle;

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{self, Term};
use alacritty_terminal::tty;

use crate::pty;

/// Dimensions for initializing the terminal grid.
struct TermDims {
    cols: usize,
    lines: usize,
}

impl Dimensions for TermDims {
    fn total_lines(&self) -> usize {
        self.lines
    }
    fn screen_lines(&self) -> usize {
        self.lines
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

/// Event listener that signals the winit event loop to request a redraw.
#[derive(Clone)]
pub struct JsonLessListener {
    proxy: winit::event_loop::EventLoopProxy<TerminalEvent>,
}

impl JsonLessListener {
    pub fn new(proxy: winit::event_loop::EventLoopProxy<TerminalEvent>) -> Self {
        Self { proxy }
    }
}

impl EventListener for JsonLessListener {
    fn send_event(&self, event: Event) {
        let terminal_event = match event {
            Event::Wakeup => TerminalEvent::Wakeup,
            Event::Exit | Event::ChildExit(_) => TerminalEvent::Exit,
            Event::ClipboardStore(_, text) => TerminalEvent::ClipboardStore(text),
            Event::ClipboardLoad(_, cb) => TerminalEvent::ClipboardLoad(cb),
            Event::PtyWrite(s) => TerminalEvent::PtyWrite(s),
            Event::Title(s) => TerminalEvent::Title(s),
            Event::TextAreaSizeRequest(cb) => TerminalEvent::TextAreaSizeRequest(cb),
            _ => return,
        };
        let _ = self.proxy.send_event(terminal_event);
    }
}

/// Events sent from the terminal I/O thread to the winit event loop.
pub enum TerminalEvent {
    Wakeup,
    Exit,
    ClipboardStore(String),
    ClipboardLoad(Arc<dyn Fn(&str) -> String + Sync + Send + 'static>),
    PtyWrite(String),
    Title(String),
    TextAreaSizeRequest(Arc<dyn Fn(WindowSize) -> String + Sync + Send + 'static>),
}

impl Clone for TerminalEvent {
    fn clone(&self) -> Self {
        match self {
            TerminalEvent::Wakeup => TerminalEvent::Wakeup,
            TerminalEvent::Exit => TerminalEvent::Exit,
            TerminalEvent::ClipboardStore(s) => TerminalEvent::ClipboardStore(s.clone()),
            TerminalEvent::ClipboardLoad(cb) => TerminalEvent::ClipboardLoad(cb.clone()),
            TerminalEvent::PtyWrite(s) => TerminalEvent::PtyWrite(s.clone()),
            TerminalEvent::Title(s) => TerminalEvent::Title(s.clone()),
            TerminalEvent::TextAreaSizeRequest(cb) => {
                TerminalEvent::TextAreaSizeRequest(cb.clone())
            }
        }
    }
}

impl fmt::Debug for TerminalEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TerminalEvent::Wakeup => write!(f, "Wakeup"),
            TerminalEvent::Exit => write!(f, "Exit"),
            TerminalEvent::ClipboardStore(s) => write!(f, "ClipboardStore({s:?})"),
            TerminalEvent::ClipboardLoad(_) => write!(f, "ClipboardLoad(<fn>)"),
            TerminalEvent::PtyWrite(s) => write!(f, "PtyWrite({s:?})"),
            TerminalEvent::Title(s) => write!(f, "Title({s:?})"),
            TerminalEvent::TextAreaSizeRequest(_) => write!(f, "TextAreaSizeRequest(<fn>)"),
        }
    }
}

/// Holds the terminal state and the channel to send input to the PTY.
pub struct Terminal {
    pub term: Arc<FairMutex<Term<JsonLessListener>>>,
    pub sender: EventLoopSender,
    _io_handle: JoinHandle<(
        EventLoop<tty::Pty, JsonLessListener>,
        alacritty_terminal::event_loop::State,
    )>,
}

impl Terminal {
    pub fn new(
        cols: u16,
        rows: u16,
        cell_width: u16,
        cell_height: u16,
        event_proxy: winit::event_loop::EventLoopProxy<TerminalEvent>,
    ) -> std::io::Result<Self> {
        let listener = JsonLessListener::new(event_proxy);

        let dims = TermDims {
            cols: cols as usize,
            lines: rows as usize,
        };
        let config = term::Config::default();
        let term = Term::new(config, &dims, listener.clone());
        let term = Arc::new(FairMutex::new(term));

        let pty = pty::create_pty(cols, rows, cell_width, cell_height)?;

        let event_loop = EventLoop::new(
            term.clone(),
            listener,
            pty,
            false, // drain_on_exit
            false, // ref_test
        )?;
        let sender = event_loop.channel();
        let io_handle = event_loop.spawn();

        Ok(Self {
            term,
            sender,
            _io_handle: io_handle,
        })
    }

    pub fn resize(&self, cols: u16, rows: u16, cell_width: u16, cell_height: u16) {
        let window_size = WindowSize {
            num_lines: rows,
            num_cols: cols,
            cell_width,
            cell_height,
        };

        let _ = self.sender.send(Msg::Resize(window_size));
        let dims = TermDims {
            cols: cols as usize,
            lines: rows as usize,
        };
        self.term.lock().resize(dims);
    }

    pub fn send_input(&self, bytes: &[u8]) {
        let _ = self.sender.send(Msg::Input(bytes.to_vec().into()));
    }
}
