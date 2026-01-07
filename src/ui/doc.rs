use super::*;
use crate::state::{Buffer, BufferId, Cursor, CursorId};
use std::collections::HashMap;

pub struct Doc {
    buffer: BufferId,
    // Remember the cursor we use for each buffer
    inputs: HashMap<BufferId, (CursorId, Input)>,
    finder: Option<Finder>,
}

impl Doc {
    pub fn new(state: &mut State, buffer: BufferId) -> Self {
        let mut this = Self {
            buffer,
            inputs: HashMap::default(),
            finder: None,
        };
        this.switch_buffer(state, buffer);
        this
    }

    pub fn close(self, state: &mut State) {
        for (buffer, (cursor, _)) in self.inputs {
            let Some(buffer) = state.buffers.get_mut(buffer) else {
                continue;
            };
            buffer.end_session(cursor);
        }
    }

    fn switch_buffer(&mut self, state: &mut State, buffer: BufferId) {
        state.set_most_recent(buffer);
        self.buffer = buffer;
        let Some(buffer) = state.buffers.get_mut(self.buffer) else {
            return;
        };
        // Start a new cursor session for this buffer if one doesn't exist
        let (cursor_id, input) = self
            .inputs
            .entry(self.buffer)
            .or_insert_with(|| (buffer.start_session(), Input::default()));
        input.refocus(buffer, *cursor_id);
    }
}

impl Element for Doc {
    fn handle(&mut self, state: &mut State, mut event: Event) -> Result<Resp, Event> {
        let (cursor_id, input) = &mut self.inputs.get_mut(&self.buffer).unwrap();

        if let Some(finder) = &mut self.finder {
            let resp = finder.handle(state, input, self.buffer, *cursor_id, event);
            event = match resp {
                Ok(resp) => {
                    if resp.is_end() {
                        self.finder = None;
                    }
                    return Ok(Resp::handled(resp.event));
                }
                Err(event) => event,
            }
        }

        let Some(buffer) = state.buffers.get_mut(self.buffer) else {
            return Err(event);
        };

        let mut open_path = buffer
            .path
            .to_owned()
            .map(|mut p| {
                p.pop();
                p
            })
            .unwrap_or_else(|| std::env::current_dir().expect("no working dir"));

        match event.to_action(|e| {
            e.to_open_switcher()
                .or_else(|| e.to_open_browser(&open_path))
                .or_else(|| e.to_open_finder(None))
                .or_else(|| e.to_move())
                .or_else(|| e.to_save())
                .or_else(|| e.to_path_search())
        }) {
            action @ Some(Action::OpenSwitcher)
            | action @ Some(Action::OpenOpener(_))
            | action @ Some(Action::OpenSaver(_)) => Ok(Resp::handled(action.map(Into::into))),
            ref action @ Some(Action::OpenFinder(ref query)) => {
                self.finder = Some(Finder::new(
                    buffer.cursors[*cursor_id],
                    query.clone(),
                    state,
                    input,
                    self.buffer,
                    *cursor_id,
                ));
                Ok(Resp::handled(None))
            }
            Some(Action::BeginSearch(needle)) => {
                let path = buffer
                    .path
                    .clone()
                    .unwrap_or_else(|| std::env::current_dir().expect("no cwd"));
                Ok(Resp::handled(Some(
                    Action::OpenSearcher(path, needle).into(),
                )))
            }
            Some(Action::SwitchBuffer(new_buffer)) => {
                self.switch_buffer(state, new_buffer);
                Ok(Resp::handled(None))
            }
            Some(Action::OpenFile(path, line_idx)) => match state.open(path) {
                Ok(buffer_id) => {
                    self.switch_buffer(state, buffer_id);
                    if let Some(buffer) = state.buffers.get_mut(self.buffer)
                        && let Some(line_idx) = line_idx
                    {
                        let (cursor_id, input) = &mut self.inputs.get_mut(&self.buffer).unwrap();
                        buffer.goto_cursor(*cursor_id, [0, line_idx as isize], true);
                        input.refocus(buffer, *cursor_id);
                    }
                    Ok(Resp::handled(None))
                }
                Err(err) => Ok(Resp::handled(Some(
                    Action::Show(Some(format!("Could not open file")), format!("{err}")).into(),
                ))),
            },
            Some(Action::CreateFile(path)) => match state.create(path) {
                Ok(buffer_id) => {
                    self.switch_buffer(state, buffer_id);
                    Ok(Resp::handled(None))
                }
                Err(err) => Ok(Resp::handled(Some(
                    Action::Show(Some(format!("Could not create file")), format!("{err}")).into(),
                ))),
            },
            Some(Action::Save) => Ok(Resp::handled(if buffer.diverged {
                Some(
                    Action::Confirm(
                        format!("File has diverged on disk. Are you sure you wish to save (y/n)?"),
                        Box::new(Action::Overwrite),
                    )
                    .into(),
                )
            } else if buffer.path.is_none() {
                Some(Action::OpenSaver(std::env::current_dir().expect("no cwd")).into())
            } else {
                buffer.save().err().map(|err| {
                    Action::Show(Some("Could not save file".to_string()), err.to_string()).into()
                })
            })),
            Some(Action::Overwrite) => Ok(Resp::handled(buffer.save().err().map(|err| {
                Action::Show(Some("Could not save file".to_string()), err.to_string()).into()
            }))),
            Some(Action::SaveFileAs(path)) => Ok(Resp::handled(if path.exists() {
                Some(
                    Action::Confirm(
                        format!("File already exists on disk. Are you sure you wish to overwrite it (y/n)?"),
                        Box::new(Action::OverwriteFileAs(path)),
                    )
                    .into(),
                )
            } else {
                buffer.save_as(path).err().map(|err| {
                    Action::Show(Some("Could not save file".to_string()), err.to_string()).into()
                })
            })),
            Some(Action::OverwriteFileAs(path)) => {
                Ok(Resp::handled(buffer.save_as(path).err().map(|err| {
                    Action::Show(Some("Could not save file".to_string()), err.to_string()).into()
                })))
            }
            Some(Action::Reload) => {
                buffer.reload();
                Ok(Resp::handled(None))
            }
            _ => {
                let Some(buffer) = state.buffers.get_mut(self.buffer) else {
                    return Err(event);
                };
                input.handle(&mut state.clipboard, buffer, *cursor_id, event)
            }
        }
    }
}

impl Visual for Doc {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        let Some(buffer) = state.buffers.get(self.buffer) else {
            return;
        };
        let (cursor_id, input) = &mut self.inputs.get_mut(&self.buffer).unwrap();

        if frame.has_focus() {
            frame.set_title(if let Some(path) = &buffer.path {
                format!("{}: {}", env!("CARGO_PKG_NAME"), path.display())
            } else {
                format!("{}: Unsaved", env!("CARGO_PKG_NAME"))
            });
        }

        let finder_h = if self.finder.is_some() { 3 } else { 0 };

        // Render input
        frame
            .rect(
                [0, 0],
                [frame.size()[0], frame.size()[1].saturating_sub(finder_h)],
            )
            .with_focus(true /*self.finder.is_none()*/)
            .with(|f| {
                input.render(
                    state,
                    buffer.name().as_deref(),
                    buffer,
                    *cursor_id,
                    self.finder.as_ref(),
                    f,
                )
            });

        // Render finder
        if let Some(finder) = &mut self.finder {
            frame
                .rect(
                    [0, frame.size()[1].saturating_sub(finder_h)],
                    [frame.size()[0], finder_h],
                )
                .with_focus(true)
                .with(|f| finder.render(state, f));
        }
    }
}

pub struct Finder {
    old_cursor: Cursor,

    buffer: Buffer,
    cursor_id: CursorId,
    input: Input,

    selected: usize,
    needle: Vec<char>,
    results: Vec<usize>,
}

impl Finder {
    fn new(
        old_cursor: Cursor,
        query: Option<String>,
        state: &mut State,
        input: &mut Input,
        buffer_id: BufferId,
        cursor_id: CursorId,
    ) -> Self {
        let mut buffer = Buffer::default();
        let cursor_id = buffer.start_session();

        // Insert default query
        buffer.insert(0, query.iter().flat_map(|s| s.chars()));

        let mut this = Self {
            old_cursor,

            cursor_id,
            buffer,
            input: Input::filter(),

            selected: 0,
            needle: Vec::new(),
            results: Vec::new(),
        };

        this.update(state, input, buffer_id, cursor_id);
        this.refocus_selected(&mut state.buffers[buffer_id], input, cursor_id);

        this
    }

    pub fn contains(&self, pos: usize) -> Option<bool> {
        let idx = self
            .results
            .binary_search(&pos)
            .unwrap_or_else(|p| p.saturating_sub(1));

        self.results
            .get(idx)
            .filter(|start| (**start..**start + self.needle.len()).contains(&pos))
            .map(|_| idx == self.selected)
    }

    fn update(
        &mut self,
        state: &mut State,
        input: &mut Input,
        buffer_id: BufferId,
        cursor_id: CursorId,
    ) {
        let buffer = &mut state.buffers[buffer_id];

        let needle = self.buffer.text.chars();
        if self.needle != needle {
            // The needle has changed!
            let haystack = buffer.text.chars();

            self.needle = needle.to_vec();
            self.results = (0..haystack.len().saturating_sub(needle.len()))
                .filter(|i| haystack[*i..].starts_with(needle))
                .collect();

            // Select the first entry that comes after the current cursor position
            self.selected = (0..self.results.len())
                .find(|i| self.results[*i] >= self.old_cursor.pos)
                .unwrap_or(0);
        }
    }

    fn refocus_selected(&mut self, buffer: &mut Buffer, input: &mut Input, cursor_id: CursorId) {
        if let Some(result) = self.results.get(self.selected) {
            buffer.cursors[cursor_id].select(*result..*result + self.needle.len());
            input.refocus(buffer, cursor_id);
        }
    }

    fn handle(
        &mut self,
        state: &mut State,
        input: &mut Input,
        buffer_id: BufferId,
        cursor_id: CursorId,
        event: Event,
    ) -> Result<Resp<()>, Event> {
        let buffer = &mut state.buffers[buffer_id];

        let res = match event
            .to_action(|e| e.to_cancel().or_else(|| e.to_go()).or_else(|| e.to_move()))
        {
            Some(Action::Cancel) => {
                buffer.cursors[cursor_id] = self.old_cursor;
                input.refocus(buffer, cursor_id);
                return Ok(Resp::end(None));
            }
            Some(Action::Go) => return Ok(Resp::end(None)),
            Some(Action::Move(dir @ (Dir::Up | Dir::Down), Dist::Char, false, false)) => {
                match dir {
                    Dir::Up => {
                        self.selected = (self.selected + self.results.len().saturating_sub(1))
                            % self.results.len().max(1)
                    }
                    Dir::Down => self.selected = (self.selected + 1) % self.results.len().max(1),
                    _ => {}
                }
                self.refocus_selected(buffer, input, cursor_id);
                Ok(Resp::handled(None))
            }
            _ => {
                let resp = self
                    .input
                    .handle(
                        &mut state.clipboard,
                        &mut self.buffer,
                        self.cursor_id,
                        event,
                    )
                    .map(Resp::into_can_end)?;
                self.refocus_selected(buffer, input, cursor_id);
                Ok(resp)
            }
        };

        self.update(state, input, buffer_id, cursor_id);

        res
    }
}

impl Visual for Finder {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        let title = if self.results.is_empty() {
            format!("No results found")
        } else {
            format!("{} of {} results", self.selected + 1, self.results.len())
        };
        self.input.render(
            state,
            Some(&title),
            &self.buffer,
            self.cursor_id,
            None,
            frame,
        );
    }
}
