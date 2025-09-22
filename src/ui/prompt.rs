use super::*;
use crate::state::{Buffer, BufferId, CursorId};
use std::{cmp::Reverse, fs, path::PathBuf};

pub struct Prompt {
    buffer: Buffer,
    cursor_id: CursorId,
    input: Input,
}

impl Prompt {
    pub fn new(init: &str) -> Self {
        let mut buffer = Buffer::default();
        let cursor_id = buffer.start_session();
        buffer.enter(cursor_id, init.chars());
        Self {
            buffer,
            cursor_id,
            input: Input::prompt(),
        }
    }

    pub fn parse_action(&self) -> Result<Action, String> {
        let cmd = self.buffer.text.to_string();
        let mut args = cmd.as_str().split_whitespace();

        match args.next() {
            // The root sees 'cancel' as an initiator for quitting
            Some("q" | "quit") => Ok(Action::Cancel),
            Some("version") => Ok(Action::Show(
                Some(format!("Version")),
                format!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
            )),
            Some("?" | "help") => Ok(Action::Show(
                Some(format!("Help")),
                format!(
                    "Temporary help info:\n\
                - quit\n\
                - version\n\
                - pane_move_left\n\
                - pane_move_right\n\
                - help"
                ),
            )),
            Some("pane_move_left") => Ok(Action::PaneMove(Dir::Left)),
            Some("pane_move_right") => Ok(Action::PaneMove(Dir::Right)),
            Some("goto_line") => {
                // Subtract 1 due to zero indexing
                let line = args
                    .next()
                    .ok_or_else(|| "Expected argument".to_string())?
                    .parse::<isize>()
                    .map_err(|_| "Expected integer".to_string())?
                    - 1;
                Ok(Action::GotoLine(line))
            }
            Some(cmd) => Err(format!("Unknown command `{cmd}`")),
            None => Err(format!("No command entered")),
        }
    }

    pub fn requested_height(&self) -> usize {
        3
    }
}

impl Element<()> for Prompt {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp<()>, Event> {
        match event.to_action(|e| {
            e.to_go()
                .or_else(|| e.to_cancel().or_else(|| e.to_command_start()))
        }) {
            Some(Action::Cancel) => Ok(Resp::end(None)),
            Some(Action::Go) => match self.parse_action() {
                Ok(action) => {
                    self.buffer.reset();
                    Ok(Resp::end(Some(action.into())))
                }
                Err(err) => Ok(Resp::handled(Some(
                    Action::Show(Some(format!("Error")), err).into(),
                ))),
            },
            _ => self
                .input
                .handle(&mut self.buffer, self.cursor_id, event)
                .map(Resp::into_can_end),
        }
    }
}

impl Visual for Prompt {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        frame.with(|f| {
            self.input
                .render(state, None, &self.buffer, self.cursor_id, None, f)
        });
    }
}

pub struct Show {
    pub title: Option<String>,
    pub label: Label,
}

impl Show {
    pub fn requested_height(&self) -> usize {
        self.label.requested_height() + 2
    }
}

impl Element<()> for Show {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp<()>, Event> {
        match event.to_action(|e| {
            e.to_cancel()
                .or_else(|| e.to_continue())
                .or_else(|| e.to_char().map(Action::Char))
        }) {
            // Shows cannot be cancelled, so pass the cancel along to the parent task
            Some(Action::Cancel) => Ok(Resp::end(Some(Action::Cancel.into()))),
            // A continue ends the show
            Some(Action::Continue) => Ok(Resp::end(None)),
            // All other events end the show and get passed to the parent
            _ => Ok(Resp::end(Some(event))),
        }
    }
}

impl Visual for Show {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        let mut frame = frame.with_border(
            if frame.has_focus() {
                &state.theme.focus_border
            } else {
                &state.theme.border
            },
            self.title.as_deref(),
        );

        let lines = self.label.lines().count();
        self.label.render(
            state,
            &mut frame.rect(
                [0, frame.size()[1].saturating_sub(3 + lines)],
                [frame.size()[0], lines],
            ),
        );
    }
}

pub struct Confirm {
    pub label: Label,
    pub action: Action,
}

impl Confirm {
    pub fn requested_height(&self) -> usize {
        self.label.requested_height() + 2
    }
}

impl Element<()> for Confirm {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp<()>, Event> {
        match event.to_action(|e| e.to_yes().or_else(|| e.to_no()).or_else(|| e.to_cancel())) {
            Some(Action::Yes) => Ok(Resp::end(Some(self.action.clone().into()))),
            Some(Action::No | Action::Cancel) => Ok(Resp::end(None)),
            // All other events get swallowed
            _ => Ok(Resp::handled(None)),
        }
    }
}

impl Visual for Confirm {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        let mut frame = frame.with_border(
            if frame.has_focus() {
                &state.theme.focus_border
            } else {
                &state.theme.border
            },
            Some("Question"),
        );

        let lines = self.label.lines().count();
        self.label.render(
            state,
            &mut frame.rect(
                [0, frame.size()[1].saturating_sub(3 + lines)],
                [frame.size()[0], lines],
            ),
        );
    }
}

pub struct Switcher {
    pub options: Options<BufferId>,
    // Filter
    pub buffer: Buffer,
    pub cursor_id: CursorId,
    pub input: Input,
}

impl Switcher {
    pub fn new(buffers: impl IntoIterator<Item = BufferId>) -> Self {
        let mut buffer = Buffer::default();
        Self {
            options: Options::new(buffers),
            cursor_id: buffer.start_session(),
            buffer,
            input: Input::filter(),
        }
    }

    pub fn requested_height(&self) -> usize {
        self.options.requested_height() + 3
    }
}

impl Element<()> for Switcher {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp<()>, Event> {
        match event.to_action(|e| e.to_cancel()) {
            Some(Action::Cancel) => Ok(Resp::end(None)),
            _ => match self.options.handle(state, event).map(Resp::into_ended) {
                Ok(Some(buffer_id)) => Ok(Resp::end(Some(Action::SwitchBuffer(buffer_id).into()))),
                Ok(None) => Ok(Resp::handled(None)),
                Err(event) => {
                    let res = self
                        .input
                        .handle(&mut self.buffer, self.cursor_id, event)
                        .map(Resp::into_can_end);
                    // Score entries
                    let filter = self.buffer.text.to_string();
                    self.options.apply_scoring(|b| {
                        let Some(buffer) = state.buffers.get(*b) else {
                            return None;
                        };
                        let name = buffer.name()?;
                        if name.starts_with(&filter) {
                            Some(1)
                        } else if name.contains(&filter) {
                            Some(2)
                        } else {
                            None
                        }
                    });
                    res
                }
            },
        }
    }
}

impl Visual for Switcher {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        frame
            .rect([0, 0], [frame.size()[0], frame.size()[1].saturating_sub(3)])
            .with(|f| self.options.render(state, f));
        frame
            .rect([0, frame.size()[1].saturating_sub(3)], [frame.size()[0], 3])
            .with(|f| {
                self.input
                    .render(state, None, &self.buffer, self.cursor_id, None, f)
            });
    }
}

impl Visual for BufferId {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        let Some(buffer) = state.buffers.get(*self) else {
            return;
        };
        frame.text([0, 0], buffer.name().as_deref().unwrap_or("<unknown>"));
    }
}

pub struct Opener {
    pub options: Options<FileOption>,
    // Filter
    pub buffer: Buffer,
    pub cursor_id: CursorId,
    pub input: Input,
}

impl Opener {
    pub fn new(path: PathBuf) -> Self {
        let mut buffer = Buffer::default();
        let cursor_id = buffer.start_session();
        match path.display().to_string().as_str() {
            s @ "/" => buffer.enter(cursor_id, s.chars()),
            s => buffer.enter(cursor_id, s.chars().chain(['/'])),
        }
        let mut this = Self {
            options: Options::new([]),
            cursor_id,
            buffer,
            input: Input::filter(),
        };
        this.update_completions();
        this
    }

    pub fn requested_height(&self) -> usize {
        self.options.requested_height() + 3
    }

    fn set_string(&mut self, s: &str) {
        self.buffer.reset();
        self.buffer.enter(self.cursor_id, s.chars());
        self.update_completions();
    }

    fn update_completions(&mut self) {
        let path_str = self.buffer.text.to_string();
        let (dir, filter) = match path_str.rsplit_once('/') {
            Some(("", filter)) => ("/", filter),
            Some((dir, filter)) => (dir, filter),
            None => ("/", path_str.as_str()),
        };
        let filter = filter.to_lowercase();
        match fs::read_dir(dir) {
            Ok(entries) => {
                let options = entries
                    .filter_map(|e| e.ok())
                    .filter_map(|entry| {
                        Some(FileOption {
                            path: entry.path(),
                            kind: if entry.file_type().ok()?.is_dir() {
                                FileKind::Dir
                            } else if entry.file_type().ok()?.is_file() {
                                FileKind::File
                            } else {
                                FileKind::Unknown
                            },
                            is_link: entry.file_type().ok()?.is_symlink(),
                        })
                    })
                    .chain(if filter != "" {
                        Some(FileOption {
                            path: [dir, &filter].into_iter().collect(),
                            kind: FileKind::New,
                            is_link: false,
                        })
                    } else {
                        None
                    });
                // TODO
                self.options.set_options(options, |e| {
                    let name = e.path.file_name()?.to_str()?.to_lowercase();
                    let modify_time = e
                        .path
                        .metadata()
                        .ok()
                        .and_then(|m| Some(m.modified().ok()?.elapsed().ok()?.as_secs()))
                        .unwrap_or(!0);
                    if matches!(e.kind, FileKind::New) {
                        // Special-case: the 'new file' entry always matches last
                        Some((1000, 0, 0))
                    } else if name == filter {
                        Some((0, modify_time, name.chars().count()))
                    } else if name.starts_with(&filter) {
                        Some((1, modify_time, name.chars().count()))
                    } else if name.contains(&filter) {
                        Some((2, modify_time, name.chars().count()))
                    } else {
                        None
                    }
                })
            }
            Err(err) => self.options.set_options::<_, ()>(Vec::new(), |_| None),
        }
    }
}

impl Element<()> for Opener {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp<()>, Event> {
        let path_str = self.buffer.text.to_string();
        match event.to_action(|e| e.to_cancel().or_else(|| e.to_char().map(Action::Char))) {
            Some(Action::Cancel) => Ok(Resp::end(None)),
            // Backspace removes the entire path segment!
            // Only works if we're at the end of the string
            Some(Action::Char('\x08')) if path_str.ends_with("/") && self.buffer.cursors.get(self.cursor_id).map_or(false, |c| c.selection().is_none() && c.pos == self.buffer.text.chars().len()) => {
                if path_str != "/" {
                    self.set_string(
                        path_str
                            .trim_end_matches("/")
                            .trim_end_matches(|c| c != '/'),
                    );
                }
                Ok(Resp::handled(None))
            }
            Some(Action::Char('/')) if path_str.ends_with("~") && std::env::home_dir().is_some() /*let Some(home_dir) = std::env::home_dir()*/ => {
                self.set_string(&format!("{}/", std::env::home_dir().unwrap().display()));
                Ok(Resp::handled(None))
            }
            _ => match self.options.handle(state, event).map(Resp::into_ended) {
                // Selecting a directory enters the directory
                Ok(Some(file)) if matches!(file.kind, FileKind::Dir) => {
                    self.set_string(&format!("{}/", file.path.display()));
                    Ok(Resp::handled(None))
                }
                Ok(Some(file)) => Ok(Resp::end(Some(Action::OpenFile(file.path).into()))),
                Ok(None) => Ok(Resp::handled(None)),
                Err(event) => {
                    let res = self
                        .input
                        .handle(&mut self.buffer, self.cursor_id, event)
                        .map(Resp::into_can_end);
                    self.update_completions();
                    res
                }
            },
        }
    }
}

#[derive(Copy, Clone)]
enum FileKind {
    Unknown,
    Dir,
    File,
    New,
}

#[derive(Clone)]
pub struct FileOption {
    pub path: PathBuf,
    pub kind: FileKind,
    pub is_link: bool,
}

impl Visual for FileOption {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        let name = match self.path.file_name().and_then(|n| n.to_str()) {
            Some(name) if matches!(self.kind, FileKind::Dir) => format!("{name}/"),
            Some(name) => format!("{name}"),
            None => format!("Unknown"),
        };
        let desc = match self.kind {
            FileKind::Dir => "Directory",
            FileKind::Unknown => "Unknown filesystem item",
            FileKind::File => "File",
            FileKind::New => "Create new file",
        };
        frame
            .with_fg(match self.kind {
                FileKind::Dir => state.theme.option_dir,
                FileKind::File | FileKind::Unknown => state.theme.option_file,
                FileKind::New => state.theme.option_new,
            })
            .text([0, 0], &name);
        frame.with_fg(state.theme.margin_line_num).with(|f| {
            f.text([f.size()[0] as isize / 2, 0], &desc);
        });
    }
}

impl Visual for Opener {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        frame
            .rect([0, 0], [frame.size()[0], frame.size()[1].saturating_sub(3)])
            .with(|f| self.options.render(state, f));
        frame
            .rect([0, frame.size()[1].saturating_sub(3)], [frame.size()[0], 3])
            .with(|f| {
                self.input
                    .render(state, None, &self.buffer, self.cursor_id, None, f)
            });
    }
}
