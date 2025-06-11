use super::*;
use crate::{
    state::{Buffer, BufferId, CursorId},
    terminal::CursorStyle,
};
use std::{collections::HashMap, path::PathBuf};

#[derive(Clone)]
pub struct Doc {
    buffer: BufferId,
    // Remember the cursor we use for each buffer
    cursors: HashMap<BufferId, CursorId>,
    input: Input,
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
        let Some(buffer) = state.buffers.get_mut(self.buffer) else {
            return Err(event);
        };

        let open_path = buffer.dir.to_owned().unwrap_or(PathBuf::from("/"));

        match event.to_action(|e| e.to_open_switcher().or_else(|| e.to_open_opener(open_path))) {
            action @ Some(Action::OpenSwitcher) => Ok(Resp::handled(action.map(Into::into))),
            action @ Some(Action::OpenOpener(_)) => Ok(Resp::handled(action.map(Into::into))),
            Some(Action::SwitchBuffer(new_buffer)) => {
                self.switch_buffer(state, new_buffer);
                Ok(Resp::handled(None))
            }
            Some(Action::OpenFile(path)) => match Buffer::from_file(path) {
                Ok(buffer) => {
                    let buffer_id = state.buffers.insert(buffer);
                    self.switch_buffer(state, buffer_id);
                    Ok(Resp::handled(None))
                }
                Err(err) => Ok(Resp::handled(Some(
                    Action::Show(Some(format!("Could not open file")), format!("{err}")).into(),
                ))),
            },
            _ => {
                let Some(buffer) = state.buffers.get_mut(self.buffer) else {
                    return Err(event);
                };
                let cursor_id = self.cursors[&self.buffer];
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
        self.input.render(state, buffer, cursor_id, frame);
    }
}
