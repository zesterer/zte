use super::*;
use alacritty_terminal::{
    Term as Alacritty,
    event::VoidListener,
    term::{Config as AlacrittyConfig, test::TermSize},
    vte::ansi,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::mpsc::{self, Receiver, Sender},
    task,
};

enum Input {
    Resize([usize; 2]),
    Bytes(Vec<u8>),
}

pub struct Term {
    term: Alacritty<VoidListener>,
    old_term_size: Option<[usize; 2]>,
    ansi: ansi::Processor,
    in_tx: Sender<Input>,
    out_rx: Receiver<Vec<u8>>,
    cmd: task::JoinHandle<Option<()>>,
}

impl Term {
    pub fn new(path: Option<PathBuf>, state: &mut State) -> Result<Self, Error> {
        let (in_tx, mut in_rx) = mpsc::channel(4096);
        let (out_tx, out_rx) = mpsc::channel(4096);
        let wakeup = state.wakeup.clone();

        // Spawn TTY and attach shell process to it
        let (pty, pts) = pty_process::open().unwrap();
        let shell = std::env::var("SHELL").map_err(|_| Error::NoShellEnvVar)?;
        let cmd = pty_process::Command::new(shell);
        let cmd = if let Some(path) = path {
            cmd.current_dir(path)
        } else {
            cmd
        };
        cmd.spawn(pts).map_err(Error::ShellProcessFailed)?;

        let cmd = task::spawn(async move {
            let (mut pty_read, mut pty_write) = pty.into_split();
            loop {
                let mut bytes = [0; 1024];
                tokio::select! {
                    n = pty_read.read(&mut bytes) => {
                        out_tx.send(bytes[..n.ok()?].to_vec()).await.ok()?;
                        wakeup.notify_one();
                    },
                    input = in_rx.recv() => match input {
                        Some(Input::Resize(sz)) => pty_write.resize(pty_process::Size::new(sz[1] as u16, sz[0] as u16)).ok()?,
                        Some(Input::Bytes(mut bytes)) => pty_write.write_all(&mut bytes).await.ok()?,
                        None => break Some(()),
                    },
                }
            }
        });

        Ok(Self {
            term: Alacritty::new(
                AlacrittyConfig::default(),
                &TermSize::new(40, 15),
                VoidListener,
            ),
            old_term_size: None,
            ansi: Default::default(),
            in_tx,
            out_rx,
            cmd,
        })
    }

    pub fn should_close(&self) -> bool {
        self.cmd.is_finished()
    }

    pub fn close(mut self, _state: &mut State) {
        self.term.exit();
    }

    fn send_bytes(&self, bytes: impl AsRef<[u8]>) {
        let _ = self.in_tx.try_send(Input::Bytes(bytes.as_ref().to_vec()));
    }
}

impl Element for Term {
    fn handle(&mut self, _state: &mut State, event: Event) -> Result<Resp, Event> {
        use crate::action::RawEvent;
        use crossterm::event::{KeyCode, KeyEvent};

        match event.to_action(|e| e.to_char().map(Action::Char).or_else(|| e.to_move())) {
            Some(Action::Char(c)) => {
                self.send_bytes(c.encode_utf8(&mut [0; 4]));
                Ok(Resp::handled(None))
            }
            _ => match event {
                Event::Raw(RawEvent(TerminalEvent::Key(KeyEvent { code, .. }))) => {
                    match code {
                        KeyCode::Left => self.send_bytes("\x1B[D"),
                        KeyCode::Right => self.send_bytes("\x1B[C"),
                        KeyCode::Up => self.send_bytes("\x1B[A"),
                        KeyCode::Down => self.send_bytes("\x1B[B"),
                        KeyCode::Home => self.send_bytes("\x1B[H"),
                        KeyCode::End => self.send_bytes("\x1B[F"),
                        KeyCode::BackTab => self.send_bytes("\x1B[Z"),
                        _ => {}
                    }
                    Err(event)
                }
                event => Err(event),
            },
        }
    }
}

impl Visual for Term {
    fn render(&mut self, state: &mut State, frame: &mut Rect) {
        while let Ok(bytes) = self.out_rx.try_recv() {
            self.ansi.advance(&mut self.term, &bytes);
        }

        frame
            .with_border(
                if frame.has_focus() {
                    &state.theme.focus_border
                } else {
                    &state.theme.border
                },
                None,
            )
            .with(|frame| {
                // Resize terminal if needed
                let term_size = frame.size();
                if Some(term_size) != self.old_term_size {
                    self.old_term_size = Some(term_size);
                    self.term.resize(TermSize::new(term_size[0], term_size[1]));
                    let _ = self.in_tx.try_send(Input::Resize(frame.size()));
                }

                if frame.has_focus() {
                    frame.set_cursor(
                        [
                            self.term.grid().cursor.point.column.0 as isize,
                            self.term.grid().cursor.point.line.0 as isize,
                        ],
                        CursorStyle::BlinkingBlock,
                    );
                }

                // Draw terminal cells
                for cell in self.term.grid().display_iter() {
                    let map_color = |c| match c {
                        ansi::Color::Named(
                            ansi::NamedColor::Foreground
                            | ansi::NamedColor::Background
                            | ansi::NamedColor::Cursor,
                        ) => Color::Reset,
                        ansi::Color::Named(n) => Color::AnsiValue(n as u8),
                        ansi::Color::Spec(rgb) => Color::Rgb {
                            r: rgb.r,
                            g: rgb.g,
                            b: rgb.b,
                        },
                        ansi::Color::Indexed(i) => Color::AnsiValue(i),
                    };
                    frame
                        .with_bg(map_color(cell.bg))
                        .with_fg(map_color(cell.fg))
                        .text(
                            [cell.point.column.0 as isize, cell.point.line.0 as isize],
                            cell.cell.c.encode_utf8(&mut [0; 4]),
                        );
                }
            });
    }
}
