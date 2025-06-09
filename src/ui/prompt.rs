use super::*;
use crate::state::{Buffer, BufferId, CursorId};

pub struct Prompt {
    buffer: Buffer,
    cursor_id: CursorId,
    input: Input,
}

impl Prompt {
    pub fn new() -> Self {
        let mut buffer = Buffer::default();
        Self {
            cursor_id: buffer.start_session(),
            buffer,
            input: Input::prompt(),
        }
    }

    pub fn get_action(&self) -> Option<Action> {
        match self.buffer.text.to_string().as_str() {
            // The root sees 'cancel' as an initiator for quitting
            "q" | "quit" => Some(Action::Cancel),
            "version" => Some(Action::Show(
                Some(format!("Version")),
                format!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
            )),
            "?" | "help" => Some(Action::Show(
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
            "pane_move_left" => Some(Action::PaneMove(Dir::Left)),
            "pane_move_right" => Some(Action::PaneMove(Dir::Right)),
            _ => None,
        }
    }

    pub fn requested_height(&self) -> usize {
        3
    }
}

impl Element<()> for Prompt {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp<()>, Event> {
        match event.to_action(|e| e.to_go().or_else(|| e.to_cancel())) {
            Some(Action::Cancel) => Ok(Resp::end(None)),
            Some(Action::Go) => {
                if let Some(action) = self.get_action() {
                    self.buffer.clear();
                    Ok(Resp::end(Some(action.into())))
                } else {
                    Ok(Resp::handled(Some(
                        Action::Show(
                            Some(format!("Error")),
                            format!("unknown command `{}`", self.buffer.text.to_string()),
                        )
                        .into(),
                    )))
                }
            }
            _ => self
                .input
                .handle(&mut self.buffer, self.cursor_id, event)
                .map(Resp::into_can_end),
        }
    }
}

impl Visual for Prompt {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        frame.with(|f| self.input.render(state, &self.buffer, self.cursor_id, f));
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
            input: Input::prompt(),
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
                        match buffer.path.as_ref() {
                            Some(path) if path.display().to_string().contains(&filter) => Some(1),
                            Some(_) => None,
                            None => Some(0),
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
            .with(|f| self.input.render(state, &self.buffer, self.cursor_id, f));
    }
}

impl Visual for BufferId {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        let Some(buffer) = state.buffers.get(*self) else {
            return;
        };
        let buffer_name = match &buffer.path {
            Some(path) => path.display().to_string(),
            None => format!("<Untitled>"),
        };
        frame.text([0, 0], buffer_name.chars());
    }
}
