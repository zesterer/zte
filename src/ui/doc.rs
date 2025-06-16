use super::*;
use crate::{
    state::{Buffer, BufferId, Cursor, CursorId},
    terminal::CursorStyle,
};
use std::{collections::HashMap, path::PathBuf};

pub struct Doc {
    buffer: BufferId,
    // Remember the cursor we use for each buffer
    cursors: HashMap<BufferId, CursorId>,
    input: Input,
    search: Option<Search>,
}

impl Doc {
    pub fn new(state: &mut State, buffer: BufferId) -> Self {
        Self {
            buffer,
            // TODO: Don't index directly
            cursors: [(buffer, state.buffers[buffer].start_session())]
                .into_iter()
                .collect(),
            input: Input::default(),
            search: None,
        }
    }

    pub fn close(self, state: &mut State) {
        for (buffer, cursor) in self.cursors {
            let Some(buffer) = state.buffers.get_mut(buffer) else {
                continue;
            };
            buffer.end_session(cursor);
        }
    }

    fn switch_buffer(&mut self, state: &mut State, buffer: BufferId) {
        self.buffer = buffer;
        let Some(buffer) = state.buffers.get_mut(self.buffer) else {
            return;
        };
        // Start a new cursor session for this buffer if one doesn't exist
        let cursor_id = *self
            .cursors
            .entry(self.buffer)
            .or_insert_with(|| buffer.start_session());
        self.input.refocus(buffer, cursor_id);
    }
}

impl Element for Doc {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp, Event> {
        let cursor_id = self.cursors[&self.buffer];

        if let Some(search) = &mut self.search {
            let resp = search.handle(state, &mut self.input, self.buffer, cursor_id, event)?;
            if resp.is_end() {
                self.search = None;
            }
            return Ok(Resp::handled(resp.event));
        }

        let Some(buffer) = state.buffers.get_mut(self.buffer) else {
            return Err(event);
        };

        let open_path = buffer.dir.to_owned().unwrap_or(PathBuf::from("/"));

        match event.to_action(|e| {
            e.to_open_switcher()
                .or_else(|| e.to_open_opener(open_path))
                .or_else(|| e.to_open_finder())
                .or_else(|| e.to_move())
                .or_else(|| e.to_save())
        }) {
            action @ Some(Action::OpenSwitcher) => Ok(Resp::handled(action.map(Into::into))),
            action @ Some(Action::OpenOpener(_)) => Ok(Resp::handled(action.map(Into::into))),
            action @ Some(Action::OpenFinder) => {
                self.search = Some(Search::new(buffer.cursors[cursor_id]));
                Ok(Resp::handled(None))
            }
            Some(Action::SwitchBuffer(new_buffer)) => {
                self.switch_buffer(state, new_buffer);
                Ok(Resp::handled(None))
            }
            Some(Action::OpenFile(path)) => match state.open_or_get(path) {
                Ok(buffer_id) => {
                    self.switch_buffer(state, buffer_id);
                    Ok(Resp::handled(None))
                }
                Err(err) => Ok(Resp::handled(Some(
                    Action::Show(Some(format!("Could not open file")), format!("{err}")).into(),
                ))),
            },
            Some(Action::Save) => {
                let event = buffer.save().err().map(|err| {
                    Action::Show(Some("Could not save file".to_string()), err.to_string()).into()
                });
                Ok(Resp::handled(event))
            }
            _ => {
                let Some(buffer) = state.buffers.get_mut(self.buffer) else {
                    return Err(event);
                };
                self.input.handle(buffer, cursor_id, event)
            }
        }
    }
}

impl Visual for Doc {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        let Some(buffer) = state.buffers.get(self.buffer) else {
            return;
        };
        let cursor_id = self.cursors[&self.buffer];

        let search_h = if self.search.is_some() { 3 } else { 0 };

        // Render input
        frame
            .rect(
                [0, 0],
                [frame.size()[0], frame.size()[1].saturating_sub(search_h)],
            )
            .with_focus(self.search.is_none())
            .with(|f| {
                self.input.render(
                    state,
                    buffer.name().as_deref(),
                    buffer,
                    cursor_id,
                    self.search.as_ref(),
                    f,
                )
            });

        // Render search
        if let Some(search) = &mut self.search {
            frame
                .rect(
                    [0, frame.size()[1].saturating_sub(search_h)],
                    [frame.size()[0], search_h],
                )
                .with_focus(true)
                .with(|f| search.render(state, f));
        }
    }
}

pub struct Search {
    old_cursor: Cursor,

    buffer: Buffer,
    cursor_id: CursorId,
    input: Input,

    selected: usize,
    needle: Vec<char>,
    results: Vec<usize>,
}

impl Search {
    fn new(old_cursor: Cursor) -> Self {
        let mut buffer = Buffer::default();
        Self {
            old_cursor,

            cursor_id: buffer.start_session(),
            buffer,
            input: Input::filter(),

            selected: 0,
            needle: Vec::new(),
            results: Vec::new(),
        }
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
            Some(Action::Move(dir, false, _)) => {
                match dir {
                    Dir::Up => {
                        self.selected = (self.selected + self.results.len().saturating_sub(1))
                            % self.results.len().max(1)
                    }
                    Dir::Down => self.selected = (self.selected + 1) % self.results.len().max(1),
                    _ => {}
                }
                Ok(Resp::handled(None))
            }
            _ => self
                .input
                .handle(&mut self.buffer, self.cursor_id, event)
                .map(Resp::into_can_end),
        };

        let needle = self.buffer.text.chars();
        if self.needle != needle {
            let haystack = buffer.text.chars();

            self.selected = 0;
            self.needle = needle.to_vec();
            self.results = (0..haystack.len().saturating_sub(needle.len()))
                .filter(|i| haystack[*i..].starts_with(needle))
                .collect();
        }

        if let Some(result) = self.results.get(self.selected) {
            buffer.cursors[cursor_id].select(*result..*result + self.needle.len());
            input.refocus(buffer, cursor_id);
        }

        res
    }
}

impl Visual for Search {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        let title = format!("{} of {} results", self.selected + 1, self.results.len());
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
