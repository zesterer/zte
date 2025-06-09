use super::*;
use crate::{
    state::{BufferId, CursorId},
    terminal::CursorStyle,
};
use std::collections::HashMap;

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

    pub fn title(&self, state: &State) -> Option<String> {
        let Some(buffer) = state.buffers.get(self.buffer) else {
            return None;
        };
        Some(buffer.path.as_ref()?.display().to_string())
    }

    pub fn close(self, state: &mut State) {
        for (buffer, cursor) in self.cursors {
            let Some(buffer) = state.buffers.get_mut(buffer) else {
                continue;
            };
            buffer.end_session(cursor);
        }
    }
}

impl Element for Doc {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp, Event> {
        let Some(buffer) = state.buffers.get_mut(self.buffer) else {
            return Err(event);
        };

        match event.to_action(|e| e.to_open_switcher()) {
            action @ Some(Action::OpenSwitcher) => Ok(Resp::handled(action.map(Into::into))),
            Some(Action::SwitchBuffer(new_buffer)) => {
                self.buffer = new_buffer;
                let Some(buffer) = state.buffers.get_mut(self.buffer) else {
                    return Err(event);
                };
                // Start a new cursor session for this buffer if one doesn't exist
                let cursor_id = *self
                    .cursors
                    .entry(self.buffer)
                    .or_insert_with(|| buffer.start_session());
                self.input.refocus(buffer, cursor_id);
                Ok(Resp::handled(None))
            }
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
