use super::*;
use crate::state::{Buffer, BufferId, Cursor, CursorId};
use std::ops::Range;

pub struct Doc {
    buffer: BufferId,
    pub cursor: CursorId,
    pub input: Input,
    finder: Option<Box<Finder>>,
}

impl Doc {
    pub fn new(state: &mut State, buffer: BufferId) -> Self {
        Self {
            buffer,
            cursor: state.buffers.get_mut(buffer).unwrap().start_session(),
            input: Input::default(),
            finder: None,
        }
    }

    pub fn close(self, state: &mut State) {
        let Some(buffer) = state.buffers.get_mut(self.buffer) else {
            return;
        };
        buffer.end_session(self.cursor);
    }
}

impl Element<()> for Doc {
    fn handle(&mut self, state: &mut State, mut event: Event) -> Result<Resp<()>, Event> {
        if let Some(finder) = &mut self.finder {
            let resp = finder.handle(state, &mut self.input, self.buffer, self.cursor, event);
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

        let open_path = buffer
            .path()
            .cloned()
            .map(|mut p| {
                p.pop();
                p
            })
            .unwrap_or_else(|| std::env::current_dir().expect("no working dir"));

        match event.to_action(|e| {
            e.to_file_op(&open_path, None)
                .or_else(|| e.to_move())
                .or_else(|| e.to_close_buffer())
        }) {
            action @ Some(Action::OpenSaver(_)) | action @ Some(Action::OpenMover(_)) => {
                Ok(Resp::handled(action.map(Into::into)))
            }
            Some(Action::OpenFinder(ref query)) => {
                self.finder = Some(
                    Finder::new(
                        buffer.cursors[self.cursor],
                        query.clone(),
                        state,
                        &mut self.input,
                        self.buffer,
                    )
                    .into(),
                );
                Ok(Resp::handled(None))
            }
            Some(Action::OpenFile(path, range)) => {
                Ok(Resp::handled(Some(Action::OpenFile(path, range).into())))
            }

            // Save
            Some(Action::SaveFile) => Ok(Resp::handled(if buffer.has_changes() {
                Some(
                    Action::Confirm(
                        format!("File has diverged on disk. Are you sure you wish to save (y/n)?"),
                        Box::new(Action::SaveFileForce),
                    )
                    .into(),
                )
            } else if buffer.path().is_none() {
                Some(Action::OpenSaver(std::env::current_dir().expect("no cwd")).into())
            } else {
                buffer.save().err().map(|err| {
                    Action::Show(Some("Could not save file".to_string()), err.to_string()).into()
                })
            })),
            Some(Action::SaveFileForce) => Ok(Resp::handled(buffer.save().err().map(|err| {
                Action::Show(Some("Could not save file".to_string()), err.to_string()).into()
            }))),

            // Save as
            Some(Action::SaveFileAs(path)) => Ok(Resp::handled(if path.exists() {
                Some(
                    Action::Confirm(
                        format!("File already exists on disk. Are you sure you wish to overwrite it (y/n)?"),
                        Box::new(Action::SaveFileAsForce(path)),
                    )
                    .into(),
                )
            } else {
                buffer.save_as(path).err().map(|err| {
                    Action::Show(Some("Could not save file".to_string()), err.to_string()).into()
                })
            })),
            Some(Action::SaveFileAsForce(path)) => {
                Ok(Resp::handled(buffer.save_as(path).err().map(|err| {
                    Action::Show(Some("Could not save file".to_string()), err.to_string()).into()
                })))
            }

            // Move
            Some(Action::MoveFile(path)) => Ok(Resp::handled(if path.exists() {
                Some(
                    Action::Confirm(
                        format!("File already exists on disk. Are you sure you wish to overwrite it (y/n)?"),
                        Box::new(Action::MoveFileForce(path)),
                    )
                    .into(),
                )
            } else {
                buffer.move_to(path).err().map(|err| {
                    Action::Show(Some("Could not save file".to_string()), err.to_string()).into()
                })
            })),
            Some(Action::MoveFileForce(path)) => {
                Ok(Resp::handled(buffer.move_to(path).err().map(|err| {
                    Action::Show(Some("Could not move file".to_string()), err.to_string()).into()
                })))
            }

            Some(Action::CloseFile) => {
                if buffer.has_changes() {
                    Ok(Resp::handled(Some(
                    Action::Confirm(
                        format!("File has unsaved changes. Are you sure you wish to lose your changes (y/n)?"),
                        Box::new(Action::CloseFileForce),
                    )
                    .into(),
                )))
                } else {
                    state.close(self.buffer);
                    Ok(Resp::end(None))
                }
            }
            Some(Action::CloseFileForce) => {
                state.close(self.buffer);
                Ok(Resp::end(None))
            }

            Some(Action::Reload) => {
                buffer.reload();
                Ok(Resp::handled(None))
            }
            _ => {
                let Some(buffer) = state.buffers.get_mut(self.buffer) else {
                    return Err(event);
                };
                self.input
                    .handle(&mut state.clipboard, buffer, self.cursor, event)
                    .map(Resp::into_can_end)
            }
        }
    }
}

impl Visual for Doc {
    fn render(&mut self, state: &mut State, frame: &mut Rect) {
        let Some(buffer) = state.buffers.get_mut(self.buffer) else {
            return;
        };

        if frame.has_focus() {
            frame.set_title(if let Some(path) = buffer.path() {
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
                self.input.render(
                    &state.theme,
                    buffer.name().as_deref(),
                    buffer,
                    self.cursor,
                    self.finder.as_deref(),
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
    needle: String,
    error: Option<String>,
    results: Vec<Range<usize>>,
}

impl Finder {
    fn new(
        old_cursor: Cursor,
        query: Option<String>,
        state: &mut State,
        input: &mut Input,
        buffer_id: BufferId,
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
            needle: String::new(),
            error: None,
            results: Vec::new(),
        };

        this.refocus_selected(&mut state.buffers[buffer_id], input, cursor_id);

        this
    }

    pub fn contains(&self, pos: usize) -> Option<bool> {
        let idx = self
            .results
            .binary_search_by_key(&pos, |r| r.start)
            .unwrap_or_else(|p| p.saturating_sub(1));

        self.results
            .get(idx)
            .filter(|range| (range.start..range.end).contains(&pos))
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

        let needle = self.buffer.text.to_string();
        // The needle has changed!
        if self.needle != needle {
            self.needle = needle;

            let haystack = buffer.text.slice(..);

            self.error = None;
            self.results = match regex::CompiledPattern::create_search(&self.needle) {
                Ok(regex) => regex
                    .find_nonoverlapping_matches(&haystack.to_string())
                    .collect(),
                Err(err) => {
                    self.error = Some(err);
                    Vec::new()
                }
            };

            // Select the first entry that comes after the current cursor position
            self.selected = (0..self.results.len())
                .rev()
                .find(|i| (self.results[*i].start..).contains(&self.old_cursor.pos))
                .unwrap_or(0);

            self.refocus_selected(buffer, input, cursor_id);
        }
    }

    fn refocus_selected(&mut self, buffer: &mut Buffer, input: &mut Input, cursor_id: CursorId) {
        if let Some(result) = self.results.get(self.selected) {
            buffer.cursors[cursor_id].select(result.clone());
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
    fn render(&mut self, state: &mut State, frame: &mut Rect) {
        let title = if let Some(err) = &self.error {
            format!("Error: {err}")
        } else if self.results.is_empty() {
            format!("No results found")
        } else {
            format!("{} of {} results", self.selected + 1, self.results.len())
        };
        self.input.render(
            &state.theme,
            Some(&title),
            &mut self.buffer,
            self.cursor_id,
            None,
            frame,
        );
    }
}
