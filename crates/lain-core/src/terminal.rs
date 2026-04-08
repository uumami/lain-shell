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
        match event {
            Event::Wakeup => {
                let _ = self.proxy.send_event(TerminalEvent::Wakeup);
            }
            Event::Exit | Event::ChildExit(_) => {
                let _ = self.proxy.send_event(TerminalEvent::Exit);
            }
            _ => {}
        }
    }
}

/// Events sent from the terminal I/O thread to the winit event loop.
#[derive(Debug, Clone)]
pub enum TerminalEvent {
    Wakeup,
    Exit,
}

/// Holds the terminal state and the channel to send input to the PTY.
pub struct Terminal {
    pub term: Arc<FairMutex<Term<JsonLessListener>>>,
    pub sender: EventLoopSender,
    _io_handle: JoinHandle<(EventLoop<tty::Pty, JsonLessListener>, alacritty_terminal::event_loop::State)>,
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
