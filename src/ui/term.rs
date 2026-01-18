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
    cmd: task::JoinHandle<()>,
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
            (async || loop {
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
            })().await;
            // If the command exits, wake up to update the UI
            wakeup.notify_one();
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
        match event {
            Event::Raw(ref ev) => {
                if let Some(s) = ev.to_esc_seq() {
                    self.send_bytes(s);
                    Ok(Resp::handled(None))
                } else {
                    // Ok(Resp::handled(Some(Action::Show(
                    //     Some(format!("Failed to handle event")),
                    //     format!("{ev:?}"),
                    // ).into())))
                    Err(event)
                }
            }
            event => Err(event),
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

use crate::action::RawEvent;
impl RawEvent {
    fn to_esc_seq(&self) -> Option<String> {
        use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

        match &self.0 {
            TerminalEvent::Key(KeyEvent {
                code,
                modifiers,
                kind,
                state,
            }) => {
                // Base on `https://www.leonerd.org.uk/hacks/fixterms/`

                match kind {
                    KeyEventKind::Press => {}
                    _ => return None,
                }

                if state.contains(KeyEventState::KEYPAD) {
                    return None;
                }
                if state.contains(KeyEventState::CAPS_LOCK) {
                    return None;
                }
                if state.contains(KeyEventState::NUM_LOCK) {
                    return None;
                }

                enum Class {
                    Unicode(char),
                    ModifiedC0(&'static str),
                    Special(u8),
                    ReallySpecial(char),
                }

                let class = match code {
                    KeyCode::Enter => Class::Unicode('\r'),
                    KeyCode::Tab => Class::Unicode('\t'),
                    KeyCode::BackTab => Class::ModifiedC0("\x1B[Z"),
                    KeyCode::Backspace => Class::Unicode('\x7F'),
                    KeyCode::Left => Class::ReallySpecial('D'),
                    KeyCode::Right => Class::ReallySpecial('C'),
                    KeyCode::Up => Class::ReallySpecial('A'),
                    KeyCode::Down => Class::ReallySpecial('B'),
                    KeyCode::Home => Class::ReallySpecial('H'),
                    KeyCode::End => Class::ReallySpecial('F'),
                    KeyCode::F(1) => Class::ReallySpecial('P'),
                    KeyCode::F(2) => Class::ReallySpecial('Q'),
                    KeyCode::F(3) => Class::ReallySpecial('R'),
                    KeyCode::F(4) => Class::ReallySpecial('S'),
                    KeyCode::Insert => Class::Special(2),
                    KeyCode::Delete => Class::Special(3),
                    KeyCode::PageUp => Class::Special(5),
                    KeyCode::PageDown => Class::Special(6),
                    KeyCode::F(5) => Class::Special(15),
                    KeyCode::F(6) => Class::Special(17),
                    KeyCode::F(7) => Class::Special(18),
                    KeyCode::F(8) => Class::Special(19),
                    KeyCode::F(9) => Class::Special(20),
                    KeyCode::F(10) => Class::Special(21),
                    KeyCode::F(11) => Class::Special(23),
                    KeyCode::F(12) => Class::Special(24),
                    KeyCode::F(_) => return None, // Should be unreachable
                    // KeyCode::Esc => Class::Unicode('\x1B'),
                    KeyCode::Char(c) => Class::Unicode(*c),
                    // Not handled
                    KeyCode::Null
                    | KeyCode::Esc
                    | KeyCode::CapsLock
                    | KeyCode::ScrollLock
                    | KeyCode::NumLock
                    | KeyCode::PrintScreen
                    | KeyCode::Pause
                    | KeyCode::Menu
                    | KeyCode::KeypadBegin
                    | KeyCode::Media(_)
                    | KeyCode::Modifier(_) => return None,
                    // _ => return None,
                };

                let mut modifiers = *modifiers;

                // Shift is removed for unicode
                if matches!(&class, Class::Unicode(_)) {
                    modifiers.remove(KeyModifiers::SHIFT);
                }

                let modifiers = 1
                    + (0 | (modifiers.contains(KeyModifiers::SHIFT) as u32) << 0
                        | (modifiers.contains(KeyModifiers::ALT) as u32) << 1
                        | (modifiers.contains(KeyModifiers::CONTROL) as u32) << 2);

                Some(match class {
                    Class::Unicode(c) if modifiers == 1 => format!("{c}"),
                    // Special cases
                    Class::Unicode(c @ ('i' | 'm' | '[' | '@')) if modifiers == 5 => {
                        format!("\x1B[{};{modifiers}{c}~", c as u8)
                    }
                    Class::Unicode(c @ 'a'..='z') if modifiers == 5 => {
                        format!("{}", (c as u8 & 0x1F) as char)
                    }
                    Class::Unicode(c) => format!("\x1B[{};{modifiers}{c}", c as u8),

                    Class::ModifiedC0(s) => format!("{s}"),

                    Class::Special(c) if modifiers == 1 => format!("\x1B[{c}~"),
                    Class::Special(c) => format!("\x1B[;{modifiers}{c}~"),

                    Class::ReallySpecial(c) if modifiers == 1 => format!("\x1B[{c}"),
                    Class::ReallySpecial(c) => format!("\x1B[1;{modifiers}{c}"),
                })
            }
            _ => None,
        }
    }
}
