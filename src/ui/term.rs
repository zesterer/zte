use super::*;
use alacritty_terminal::{
    Term as Alacritty,
    event::{Event as TermEvent, EventListener},
    grid::{Dimensions as _, Scroll},
    term::{ClipboardType, Config as AlacrittyConfig, TermMode, test::TermSize},
    vte::ansi,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{
        Notify,
        mpsc::{self, Receiver, Sender},
    },
    task,
};

enum Input {
    Resize([usize; 2]),
    Bytes(Vec<u8>),
}

enum Output {
    Bytes(Vec<u8>),
    Event(TermEvent),
}

struct Listener(Sender<Output>, Arc<Notify>);

impl EventListener for Listener {
    fn send_event(&self, event: TermEvent) {
        let _ = self.0.try_send(Output::Event(event));
        self.1.notify_one();
    }
}

pub struct Term {
    term: Alacritty<Listener>,
    old_term_size: Option<[usize; 2]>,
    title: Option<String>,
    ansi: ansi::Processor,
    in_tx: Sender<Input>,
    out_rx: Receiver<Output>,
    cmd: task::JoinHandle<()>,
    scroller: Scroller,
    bell: bool,
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

        let cmd = task::spawn({
            let wakeup = wakeup.clone();
            let out_tx = out_tx.clone();
            async move {
                let (mut pty_read, mut pty_write) = pty.into_split();
                (async || loop {
                    let mut bytes = [0; 1024];
                    tokio::select! {
                        n = pty_read.read(&mut bytes) => {
                            out_tx.send(Output::Bytes(bytes[..n.ok()?].to_vec())).await.ok()?;
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
            }
        });

        Ok(Self {
            term: Alacritty::new(
                AlacrittyConfig::default(),
                &TermSize::new(40, 15),
                Listener(out_tx.clone(), wakeup.clone()),
            ),
            title: None,
            old_term_size: None,
            ansi: Default::default(),
            in_tx,
            out_rx,
            cmd,
            scroller: Scroller::default(),
            bell: false,
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
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp, Event> {
        // First, handle scroller events
        let old_focus = [
            0,
            self.term.total_lines() as isize
                - self.term.screen_lines() as isize
                - self.term.grid().display_offset() as isize,
        ];
        let mut focus = old_focus;
        let event = match self
            .scroller
            .handle(event, self.term.total_lines(), &mut focus)
        {
            Ok(resp) => {
                self.term
                    .scroll_display(Scroll::Delta((old_focus[1] - focus[1]) as i32));
                return Ok(resp);
            }
            Err(event) => event,
        };

        match event.to_action(|e| e.to_move().or_else(|| e.to_edit())) {
            Some(Action::Move(dir, dist @ (Dist::Doc | Dist::Page), false, false))
                if !self.term.mode().contains(TermMode::ALT_SCREEN) =>
            {
                let dir = match dir {
                    Dir::Up => 1,
                    Dir::Down => -1,
                    _ => 0,
                };
                let dist = match dist {
                    Dist::Doc => self.term.total_lines(),
                    Dist::Page => self.term.screen_lines(),
                    Dist::Char => 1,
                };
                self.term.scroll_display(Scroll::Delta(dir * dist as i32));
                Ok(Resp::handled(None))
            }
            Some(Action::Paste) => {
                if let Ok(s) = state.clipboard.get() {
                    let s = if self.term.mode().contains(TermMode::BRACKETED_PASTE) {
                        format!("\x1B[200~{s}\x1B[201~")
                    } else {
                        s
                    };
                    self.send_bytes(s);
                }
                Ok(Resp::handled(None))
            }
            _ => {
                if let Event::Raw(ref ev) = event
                    && let Some(s) = ev.to_esc_seq()
                {
                    // Ensure the cursor is on-screen. TODO: Better way of differentiating this than `ALT_SCREEN`
                    if !self.term.mode().contains(TermMode::ALT_SCREEN) {
                        self.term.scroll_to_point(self.term.grid().cursor.point);
                    }
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
        }
    }
}

impl Visual for Term {
    fn render(&mut self, state: &mut State, frame: &mut Rect) {
        while let Ok(out) = self.out_rx.try_recv() {
            match out {
                Output::Bytes(bytes) => self.ansi.advance(&mut self.term, &bytes),
                // Title changes
                Output::Event(TermEvent::Title(title)) => self.title = Some(title),
                Output::Event(TermEvent::ResetTitle) => self.title = None,
                // Proxy clipboard events to our internal clipboard
                Output::Event(TermEvent::ClipboardStore(ClipboardType::Clipboard, s)) => {
                    _ = state.clipboard.set(s)
                }
                Output::Event(TermEvent::ClipboardLoad(ClipboardType::Clipboard, fmt)) => {
                    if let Ok(s) = state.clipboard.get() {
                        let _ = self.in_tx.try_send(Input::Bytes(fmt(&s).into()));
                    }
                }
                // Pass bell events on to host
                Output::Event(TermEvent::Bell) => self.bell = true,
                Output::Event(_) => {}
            }
        }

        let display_offset = self.term.grid().display_offset() as isize;

        if frame.has_focus()
            && let Some(title) = &self.title
        {
            frame.set_title(format!("{}: {title}", env!("CARGO_PKG_NAME")));
        }

        if self.bell {
            self.bell = false;
            frame.ring_bell();
        }

        frame
            .with_border(
                if frame.has_focus() {
                    &state.theme.focus_border
                } else {
                    &state.theme.border
                },
                self.title.as_deref(),
            )
            .with(|frame| {
                // Resize terminal if needed
                let term_size = frame.size().map(|e| e.max(1));
                if Some(term_size) != self.old_term_size {
                    self.old_term_size = Some(term_size);
                    self.term.resize(TermSize::new(term_size[0], term_size[1]));
                    let _ = self.in_tx.try_send(Input::Resize(frame.size()));
                }

                if frame.has_focus() {
                    frame.set_cursor(
                        [
                            self.term.grid().cursor.point.column.0 as isize,
                            self.term.grid().cursor.point.line.0 as isize + display_offset,
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
                            [
                                cell.point.column.0 as isize,
                                cell.point.line.0 as isize + display_offset,
                            ],
                            cell.cell.c.encode_utf8(&mut [0; 4]),
                        );
                }
            });

        let focus = [
            0,
            self.term.total_lines() as isize
                - self.term.screen_lines() as isize
                - self.term.grid().display_offset() as isize,
        ];
        self.scroller.render(frame, self.term.total_lines(), focus);
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
