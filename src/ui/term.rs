use super::*;
use crate::state::{Clipboard, TermId};
use alacritty_terminal::{
    Term as Alacritty,
    event::{Event as TermEvent, EventListener},
    grid::{Dimensions as _, Scroll},
    index::{Column, Line, Point, Side},
    selection::{Selection, SelectionType},
    term::{
        ClipboardType, Config as AlacrittyConfig, Osc52, TermMode, cell::Flags, test::TermSize,
    },
    vte::ansi,
};
use std::time::SystemTime;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::mpsc::{self, Receiver, Sender},
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

struct Listener(Sender<Output>);

impl EventListener for Listener {
    fn send_event(&self, event: TermEvent) {
        let _ = self.0.try_send(Output::Event(event));
        // self.1.notify_one();
    }
}

pub struct Term {
    term: Alacritty<Listener>,
    resize_req: Option<Option<[usize; 2]>>,
    pub title: Option<String>,
    ansi: ansi::Processor,
    in_tx: Sender<Input>,
    out_rx: Receiver<Output>,
    cmd: task::JoinHandle<()>,
    bell: bool,
    pub last_switch: Option<SystemTime>,
    pub open_count: usize,
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
                AlacrittyConfig {
                    kitty_keyboard: true,
                    osc52: Osc52::CopyPaste,
                    ..AlacrittyConfig::default()
                },
                &TermSize::new(40, 15),
                Listener(out_tx.clone()),
            ),
            title: None,
            resize_req: None,
            ansi: Default::default(),
            in_tx,
            out_rx,
            cmd,
            bell: false,
            last_switch: None,
            open_count: 0,
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

    pub fn pre_render(
        &mut self,
        clipboard: &mut Clipboard,
        needs_render: &mut bool,
        bell_rung: &mut bool,
    ) {
        if let Some(Some(sz)) = self.resize_req.take() {
            self.term.resize(TermSize::new(sz[0], sz[1]));
            let _ = self.in_tx.try_send(Input::Resize(sz));
        }
        while let Ok(out) = self.out_rx.try_recv() {
            *needs_render = true;
            match out {
                Output::Bytes(bytes) => self.ansi.advance(&mut self.term, &bytes),
                // Title changes
                Output::Event(TermEvent::Title(title)) => self.title = Some(title),
                Output::Event(TermEvent::ResetTitle) => self.title = None,
                // Proxy clipboard events to our internal clipboard
                Output::Event(TermEvent::ClipboardStore(ClipboardType::Clipboard, s)) => {
                    _ = clipboard.set(s)
                }
                Output::Event(TermEvent::ClipboardLoad(ClipboardType::Clipboard, fmt)) => {
                    if let Ok(s) = clipboard.get() {
                        let _ = self.in_tx.try_send(Input::Bytes(fmt(&s).into()));
                    }
                }
                // Pass bell events on to host
                Output::Event(TermEvent::Bell) => self.bell = true,
                Output::Event(_) => {}
            }
        }

        if self.bell {
            self.bell = false;
            *bell_rung = true;
        }
    }
}

pub struct TermWindow {
    pub term: TermId,
    scroller: Scroller,
    term_area: Area,
}

impl TermWindow {
    pub fn new(term: TermId) -> Self {
        Self {
            term,
            scroller: Scroller::default(),
            term_area: Area::default(),
        }
    }

    pub fn close(self, state: &mut State) {
        state.close_term_window(self.term);
    }

    pub fn should_close(&self, state: &mut State) -> bool {
        state
            .terms
            .get(self.term)
            .map_or(true, |t| t.should_close())
    }
}

impl Element for TermWindow {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp, Event> {
        let Some(term) = state.terms.get_mut(self.term) else {
            return Err(event);
        };

        let display_offset = term.term.grid().display_offset() as isize;
        // First, handle scroller events
        let old_focus =
            term.term.total_lines() as isize - term.term.screen_lines() as isize - display_offset;
        let mut focus = old_focus;
        let event = match self
            .scroller
            .handle(event, term.term.total_lines(), [&mut 0, &mut focus])
        {
            Ok(resp) => {
                term.term
                    .scroll_display(Scroll::Delta((old_focus - focus) as i32));
                return Ok(resp);
            }
            Err(event) => event,
        };

        let pos_to_point = |pos| {
            self.term_area.contains(pos).map(|pos| {
                Point::new(
                    Line(pos[1] as i32 - display_offset as i32),
                    Column(pos[0].max(0) as usize),
                )
            })
        };

        match event.to_action(|e| e.to_move().or_else(|| e.to_edit())) {
            Some(Action::Move(dir, dist @ (Dist::Doc | Dist::Page), false, false))
                if !term.term.mode().contains(TermMode::ALT_SCREEN) =>
            {
                let dir = match dir {
                    Dir::Up => 1,
                    Dir::Down => -1,
                    _ => 0,
                };
                let dist = match dist {
                    Dist::Doc => term.term.total_lines(),
                    Dist::Page => term.term.screen_lines(),
                    Dist::Char => 1,
                };
                term.term.scroll_display(Scroll::Delta(dir * dist as i32));
                Ok(Resp::handled(None))
            }
            Some(Action::Copy)
                if term
                    .term
                    .selection
                    .as_ref()
                    .map_or(false, |s| !s.is_empty()) =>
            {
                if let Some(s) = term.term.selection_to_string() {
                    let _ = state.clipboard.set(s);
                }
                Ok(Resp::handled(None))
            }
            Some(Action::Paste) => {
                if let Ok(s) = state.clipboard.get() {
                    let s = if term.term.mode().contains(TermMode::BRACKETED_PASTE) {
                        format!("\x1B[200~{s}\x1B[201~")
                    } else {
                        s
                    };
                    term.send_bytes(s);
                }
                Ok(Resp::handled(None))
            }
            Some(Action::Mouse(MouseAction::Click, pos, Modifiers::NONE, _drag_id)) => {
                if let Some(point) = pos_to_point(pos) {
                    term.term.selection =
                        Some(Selection::new(SelectionType::Simple, point, Side::Left));
                    Ok(Resp::handled(None))
                } else {
                    Err(event)
                }
            }
            Some(Action::Mouse(MouseAction::Drag, pos, Modifiers::NONE, _drag_id)) => {
                if let Some(point) = pos_to_point(pos)
                    && let Some(sel) = &mut term.term.selection
                {
                    sel.update(point, Side::Left);
                    Ok(Resp::handled(None))
                } else {
                    Err(event)
                }
            }
            _ => {
                use terminput::{Encoding, KittyFlags};

                if let Event::Raw(ref ev) = event
                    && let Ok(ev) = terminput_crossterm::to_terminput(ev.0.clone())
                    && let mut bytes = [0; 16]
                    && let mode = term.term.mode()
                    && let Ok(n) = ev.encode(
                        &mut bytes,
                        Encoding::Kitty(
                            KittyFlags::empty()
                                | if mode.contains(TermMode::DISAMBIGUATE_ESC_CODES) {
                                    KittyFlags::DISAMBIGUATE_ESCAPE_CODES
                                } else {
                                    KittyFlags::empty()
                                }
                                | if mode.contains(TermMode::REPORT_EVENT_TYPES) {
                                    KittyFlags::REPORT_EVENT_TYPES
                                } else {
                                    KittyFlags::empty()
                                }
                                | if mode.contains(TermMode::REPORT_ALTERNATE_KEYS) {
                                    KittyFlags::REPORT_ALTERNATE_KEYS
                                } else {
                                    KittyFlags::empty()
                                }
                                | if mode.contains(TermMode::REPORT_ALL_KEYS_AS_ESC) {
                                    KittyFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                                } else {
                                    KittyFlags::empty()
                                },
                        ),
                    )
                {
                    // Ensure the cursor is on-screen. TODO: Better way of differentiating this than `ALT_SCREEN`
                    if !term.term.mode().contains(TermMode::ALT_SCREEN) {
                        term.term.scroll_to_point(term.term.grid().cursor.point);
                    }
                    term.send_bytes(&bytes[..n]);
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

impl Visual for TermWindow {
    fn render(&mut self, state: &mut State, frame: &mut Rect) {
        let Some(term) = state.terms.get_mut(self.term) else {
            return;
        };

        let display_offset = term.term.grid().display_offset() as isize;

        if frame.has_focus()
            && let Some(title) = &term.title
        {
            frame.set_title(format!("{}: {title}", env!("CARGO_PKG_NAME")));
        }

        frame
            .with_border(
                if frame.has_focus() {
                    &state.theme.focus_border
                } else {
                    &state.theme.border
                },
                term.title.as_deref(),
            )
            .with(|frame| {
                self.term_area = frame.area();

                // Resize terminal if needed
                let term_size = frame.size().map(|e| e.max(1));
                // If we have focus, our request takes priority
                if frame.has_focus() || term.resize_req.is_none() {
                    // Only actually perform a resize if the terminal needs it
                    if term_size[0] != term.term.columns()
                        || term_size[1] != term.term.screen_lines()
                    {
                        term.resize_req = Some(Some(term_size));
                        state.needs_render = true;
                    } else {
                        term.resize_req = Some(None);
                    }
                }

                if frame.has_focus() {
                    let style = match (
                        term.term.cursor_style().shape,
                        term.term.cursor_style().blinking,
                    ) {
                        (ansi::CursorShape::Beam, true) => Some(CursorStyle::BlinkingBar),
                        (ansi::CursorShape::Beam, false) => Some(CursorStyle::SteadyBar),
                        (ansi::CursorShape::Underline, true) => {
                            Some(CursorStyle::BlinkingUnderScore)
                        }
                        (ansi::CursorShape::Underline, false) => {
                            Some(CursorStyle::SteadyUnderScore)
                        }
                        (ansi::CursorShape::Block | ansi::CursorShape::HollowBlock, true) => {
                            Some(CursorStyle::BlinkingBlock)
                        }
                        (ansi::CursorShape::Block | ansi::CursorShape::HollowBlock, false) => {
                            Some(CursorStyle::SteadyBlock)
                        }
                        (ansi::CursorShape::Hidden, _) => None,
                    };
                    if let Some(style) = style {
                        frame.set_cursor(
                            [
                                term.term.grid().cursor.point.column.0 as isize,
                                term.term.grid().cursor.point.line.0 as isize + display_offset,
                            ],
                            style,
                        );
                    } else {
                        frame.hide_cursor();
                    }
                }

                // Draw terminal cells
                for cell in term.term.grid().display_iter() {
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
                    let mut attr = Attributes::none();
                    if cell.flags.contains(Flags::INVERSE) {
                        attr.set(Attribute::Reverse);
                    }
                    if cell.flags.contains(Flags::BOLD) {
                        attr.set(Attribute::Bold);
                    }
                    if cell.flags.contains(Flags::ITALIC) {
                        attr.set(Attribute::Italic);
                    }
                    if cell.flags.contains(Flags::UNDERLINE) {
                        attr.set(Attribute::Underlined);
                    }
                    if cell.flags.contains(Flags::DIM) {
                        attr.set(Attribute::Dim);
                    }
                    if cell.flags.contains(Flags::STRIKEOUT) {
                        attr.set(Attribute::CrossedOut);
                    }
                    if cell.flags.contains(Flags::DOUBLE_UNDERLINE) {
                        attr.set(Attribute::DoubleUnderlined);
                    }
                    if cell.flags.contains(Flags::UNDERCURL) {
                        attr.set(Attribute::Undercurled);
                    }
                    if cell.flags.contains(Flags::DOTTED_UNDERLINE) {
                        attr.set(Attribute::Underdotted);
                    }
                    if cell.flags.contains(Flags::DASHED_UNDERLINE) {
                        attr.set(Attribute::Underdashed);
                    }
                    frame
                        .with_bg(map_color(cell.bg))
                        .with_fg(map_color(cell.fg))
                        .with_theme(
                            if let Some(sel) = &term.term.selection
                                && let Some(range) = sel.to_range(&term.term)
                                && range.contains(cell.point)
                            {
                                Some(state.theme.select)
                            } else {
                                None
                            },
                        )
                        .with_attr(attr)
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
            term.term.total_lines() as isize
                - term.term.screen_lines() as isize
                - term.term.grid().display_offset() as isize,
        ];
        self.scroller.render(frame, term.term.total_lines(), focus);
    }
}
