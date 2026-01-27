use super::*;
use crate::state::{Buffer, BufferId, CursorId};
use slotmap::Key;
use std::{fs, path::PathBuf};

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
                - reload : Reload the current file from disk, dropping unsaved changes\n\
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
            Some(arg0 @ "search") => {
                let needle = Some(cmd.get(arg0.len()..).unwrap().trim().to_string())
                    .filter(|n| !n.is_empty());
                Ok(Action::BeginSearch(needle))
            }
            Some("reload") => Ok(Action::Reload),
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
                .handle(
                    &mut state.clipboard,
                    &mut self.buffer,
                    self.cursor_id,
                    event,
                )
                .map(Resp::into_can_end),
        }
    }
}

impl Visual for Prompt {
    fn render(&mut self, state: &mut State, frame: &mut Rect) {
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
    fn handle(&mut self, _state: &mut State, event: Event) -> Result<Resp<()>, Event> {
        match event.to_action(|e| {
            e.to_cancel()
                .or_else(|| e.to_continue())
                .or_else(|| e.to_char().map(Action::Char))
        }) {
            Some(Action::Cancel) => Ok(Resp::end(None)),
            // A continue ends the show
            Some(Action::Continue) => Ok(Resp::end(None)),
            // All other events end the show and get passed to the parent
            _ => Ok(Resp::end(Some(event))),
        }
    }
}

impl Visual for Show {
    fn render(&mut self, state: &mut State, frame: &mut Rect) {
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
    fn handle(&mut self, _state: &mut State, event: Event) -> Result<Resp<()>, Event> {
        match event.to_action(|e| e.to_yes().or_else(|| e.to_no()).or_else(|| e.to_cancel())) {
            Some(Action::Yes) => Ok(Resp::end(Some(self.action.clone().into()))),
            Some(Action::No | Action::Cancel) => Ok(Resp::end(None)),
            // All other events get swallowed
            _ => Ok(Resp::handled(None)),
        }
    }
}

impl Visual for Confirm {
    fn render(&mut self, state: &mut State, frame: &mut Rect) {
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
                        .handle(
                            &mut state.clipboard,
                            &mut self.buffer,
                            self.cursor_id,
                            event,
                        )
                        .map(Resp::into_can_end);
                    // Score entries
                    let filter = self.buffer.text.to_string().to_lowercase();
                    if res.is_ok() {
                        self.options.apply_scoring(|b| {
                            let Some(buffer) = state.buffers.get(*b) else {
                                return None;
                            };
                            let name = buffer.name().as_deref().unwrap_or("").to_lowercase();
                            let parent = buffer
                                .path()
                                .and_then(|p| Some(p.parent()?.to_str()?.to_lowercase()));
                            if name.starts_with(&filter) {
                                Some(1)
                            } else if name.contains(&filter) {
                                Some(2)
                            } else if let Some(parent) = parent
                                && parent.contains(&filter)
                            {
                                Some(3)
                            } else {
                                None
                            }
                        });
                    }
                    res
                }
            },
        }
    }
}

impl Visual for Switcher {
    fn render(&mut self, state: &mut State, frame: &mut Rect) {
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
    fn render(&mut self, state: &mut State, frame: &mut Rect) {
        let Some(buffer) = state.buffers.get(*self) else {
            return;
        };
        frame
            .with_theme(state.theme.option_file)
            .text([0, 0], buffer.name().as_deref().unwrap_or("<unknown>"));
        let path_x = (frame.size()[0] as isize / 3).max(32);
        frame.with_theme(state.theme.option_dir).text(
            [path_x, 0],
            &buffer
                .path()
                .and_then(|p| Some(format!("{}", p.parent()?.display())))
                .unwrap_or_else(|| format!("<anonymous #{}>", self.data().as_ffi())),
        );
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum FileBrowserMode {
    Opener,
    Save,
    Move,
}

pub struct FileBrowser {
    pub options: Options<FileOption>,
    // Filter
    pub buffer: Buffer,
    pub cursor_id: CursorId,
    pub input: Input,
    preview: Option<(Buffer, CursorId, Input)>,
    mode: FileBrowserMode,
}

impl FileBrowser {
    pub fn new(path: PathBuf, mode: FileBrowserMode) -> Self {
        let mut buffer = Buffer::default();
        let cursor_id = buffer.start_session();
        match path.display().to_string().as_str() {
            s @ "/" => buffer.enter(cursor_id, s.chars()),
            s => buffer.enter(
                cursor_id,
                s.chars().chain((!s.ends_with('/')).then_some('/')),
            ),
        }
        let mut this = Self {
            options: Options::new([]),
            cursor_id,
            buffer,
            input: Input::filter(),
            preview: None,
            mode,
        };
        this.update_completions();
        this
    }

    fn set_string(&mut self, s: &str) {
        self.buffer.reset();
        self.buffer.enter(self.cursor_id, s.chars());
        self.update_completions();
    }

    fn update_completions(&mut self) {
        let path_str = self.buffer.text.to_string();
        let (dir, file_name) = match path_str.rsplit_once('/') {
            Some(("", filter)) => ("/", filter),
            Some((dir, filter)) => (dir, filter),
            None => ("/", path_str.as_str()),
        };
        let filter = file_name.to_lowercase();
        match fs::read_dir(dir) {
            Ok(entries) => {
                let mut options = entries
                    .filter_map(|e| e.ok())
                    .filter_map(|entry| {
                        let metadata = fs::metadata(entry.path()).ok()?;
                        Some(FileOption {
                            path: entry.path(),
                            kind: if metadata.file_type().is_dir() {
                                FileKind::Dir
                            } else if metadata.file_type().is_file() {
                                FileKind::File
                            } else {
                                FileKind::Unknown
                            },
                            is_link: entry.file_type().ok()?.is_symlink(),
                        })
                    })
                    .collect::<Vec<_>>();
                if filter != ""
                    && options
                        .iter()
                        .all(|e| e.path.file_name().and_then(|e| e.to_str()) != Some(file_name))
                {
                    options.push(FileOption {
                        path: [dir, &file_name].into_iter().collect(),
                        kind: FileKind::New,
                        is_link: false,
                    });
                }
                if self.mode == FileBrowserMode::Opener {
                    options.push(FileOption {
                        path: [dir].into_iter().collect(),
                        kind: FileKind::Term,
                        is_link: false,
                    });
                }
                // TODO
                self.options.set_options(options, |e| {
                    let name = e.path.file_name()?.to_str()?.to_lowercase();
                    let modify_time = e
                        .path
                        .metadata()
                        .ok()
                        .and_then(|m| Some(m.modified().ok()?.elapsed().ok()?.as_secs()))
                        .unwrap_or(!0);
                    if e.kind == FileKind::Term {
                        Some((100000, 0, 0, name))
                    } else if filter == "" {
                        // When no filter is specified, simply order alphabetically
                        Some((0, 0, 0, name))
                    } else if matches!(e.kind, FileKind::New) {
                        // Special-case: the 'new file' entry always matches last
                        Some((1000, 0, 0, String::new()))
                    } else if name == filter {
                        Some((0, modify_time, name.chars().count(), String::new()))
                    } else if name.starts_with(&filter) {
                        Some((1, modify_time, name.chars().count(), String::new()))
                    } else if name.contains(&filter) {
                        Some((2, modify_time, name.chars().count(), String::new()))
                    } else {
                        None
                    }
                })
            }
            // TODO: Don't assume error is due to non-existent directory!
            Err(_) => self.options.set_options(
                if filter != "" {
                    vec![FileOption {
                        path: [dir, &file_name].into_iter().collect(),
                        kind: FileKind::New,
                        is_link: false,
                    }]
                } else {
                    Vec::new()
                },
                |_| Some(()),
            ),
        }
    }
}

impl Element<()> for FileBrowser {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp<()>, Event> {
        let path_str = self.buffer.text.to_string();
        let res = match event.to_action(|e| e
            .to_cancel()
            .or_else(|| e.to_char().map(Action::Char))
            .or_else(|| e.to_indent())) {
            Some(Action::Cancel) => Ok(Resp::end(None)),
            // Backspace removes the entire path segment!
            // Only works if we're at the end of the string
            Some(Action::Char('\x08')) if self.buffer.cursors.get(self.cursor_id).map_or(false, |c| c.selection().is_none() && c.pos == self.buffer.text.chars().len()) => {
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
            // Tab can be used to auto-complete directories
            Some(Action::Indent(true)) => if let Some(file) = self.options.selected() {
                let tail = if let FileKind::Dir = file.kind { "/" } else { "" };
                self.set_string(&format!("{}{tail}", file.path.display()));
                Ok(Resp::handled(None))
            } else {
                Err(event)
            },
            _ => match self.options.handle(state, event).map(Resp::into_ended) {
                // Selecting a directory enters the directory
                Ok(Some(file)) => match file.kind {
                    FileKind::Dir => {
                        self.set_string(&format!("{}/", file.path.display()));
                        Ok(Resp::handled(None))
                    },
                    FileKind::File | FileKind::New => match &self.mode {
                        FileBrowserMode::Opener => Ok(Resp::end(Some(Action::OpenFile(file.path, None).into()))),
                        FileBrowserMode::Save => Ok(Resp::end(Some(Action::SaveFileAs(file.path).into()))),
                        FileBrowserMode::Move => Ok(Resp::end(Some(Action::MoveFile(file.path).into()))),
                    },
                    FileKind::Term => Ok(Resp::end(Some(Action::NewTerm(Some(file.path)).into()))),
                    FileKind::Unknown => Ok(Resp::handled(None)),
                }
                Ok(None) => Ok(Resp::handled(None)),
                Err(event) => {
                    let res = match self
                        .input
                        .handle(&mut state.clipboard, &mut self.buffer, self.cursor_id, event)
                        .map(Resp::into_can_end)
                    {
                        Ok(x) => Ok(x),
                        Err(event) => if let Some((buffer, cursor_id, input)) = &mut self.preview {
                            input.handle(&mut state.clipboard, buffer, *cursor_id, event).map(Resp::into_can_end)
                        } else {
                            Err(event)
                        },
                    };
                    res
                }
            }
        };

        if self.buffer.text.to_string() != path_str {
            self.update_completions();
        }
        res
    }
}

#[derive(Copy, Clone, PartialEq)]
enum FileKind {
    Unknown,
    Dir,
    File,
    New,
    Term,
}

#[derive(Clone)]
pub struct FileOption {
    pub path: PathBuf,
    kind: FileKind,
    pub is_link: bool,
}

impl Visual for FileOption {
    fn render(&mut self, state: &mut State, frame: &mut Rect) {
        let name = match self.path.file_name().and_then(|n| n.to_str()) {
            Some(_) if matches!(self.kind, FileKind::Term) => format!("$"),
            Some(name) if matches!(self.kind, FileKind::Dir) => format!("{name}/"),
            Some(name) => format!("{name}"),
            None => format!("Unknown"),
        };
        let is_link = if self.is_link { " (symlink)" } else { "" };
        let desc = match self.kind {
            FileKind::Dir => format!("Directory{is_link}"),
            FileKind::Unknown => format!("Unknown{is_link}"),
            FileKind::File => format!("File{is_link}"),
            FileKind::New => format!("New file{is_link}"),
            FileKind::Term => format!("Open terminal"),
        };
        let theme = match self.kind {
            FileKind::Dir => state.theme.option_dir,
            FileKind::File | FileKind::Unknown => state.theme.option_file,
            FileKind::New => state.theme.option_new,
            FileKind::Term => state.theme.option_term,
        };
        frame.with_theme(theme).text([0, 0], &name);
        frame.with_theme(theme).with(|f| {
            f.text([f.size()[0] as isize * 2 / 4, 0], &desc);
        });
    }
}

impl Visual for FileBrowser {
    fn render(&mut self, state: &mut State, frame: &mut Rect) {
        self.preview = self.options.selected().and_then(|f| {
            self.preview
                .take()
                .filter(|(b, _, _)| b.is_same_path(&f.path))
                .or_else(|| {
                    let mut buffer = Buffer::open(f.path.clone()).ok()?;
                    let cursor_id = buffer.start_session();
                    Some((buffer, cursor_id, Input::default()))
                })
        });

        let path_input_sz = 3;
        let options_sz = self
            .options
            .requested_height()
            .max(1)
            .min(frame.size()[1] / 2);
        let preview_sz = frame.size()[1].saturating_sub(options_sz + path_input_sz);

        if let Some((buffer, cursor_id, input)) = &mut self.preview {
            frame.rect([0, 0], [frame.size()[0], preview_sz]).with(|f| {
                input.render(state, buffer.name().as_deref(), buffer, *cursor_id, None, f)
            });
        }

        frame
            .rect([0, preview_sz], [frame.size()[0], options_sz])
            .with(|f| self.options.render(state, f));
        frame
            .rect(
                [0, preview_sz + options_sz],
                [frame.size()[0], path_input_sz],
            )
            .with(|f| {
                let title = match &self.mode {
                    FileBrowserMode::Opener => "Open file",
                    FileBrowserMode::Save => "Save file",
                    FileBrowserMode::Move => "Move file",
                };
                self.input
                    .render(state, Some(title), &self.buffer, self.cursor_id, None, f)
            });
    }
}
