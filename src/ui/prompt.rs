use super::*;
use crate::state::BufferId;
use std::str::FromStr;

pub struct Prompt {
    pub input: Input,
}

impl Prompt {
    pub fn get_action(&self) -> Option<Action> {
        match self.input.get_text().as_str() {
            // The root sees 'cancel' as an initiator for quitting
            "q" | "quit" => Some(Action::Cancel),
            "version" => Some(Action::Show(format!(
                "{} {}",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION")
            ))),
            "?" | "help" => Some(Action::Show(format!(
                "Temporary help info:\n\
                - quit\n\
                - version\n\
                - pane_move_left\n\
                - pane_move_right\n\
                - help"
            ))),
            "pane_move_left" => Some(Action::PaneMove(Dir::Left)),
            "pane_move_right" => Some(Action::PaneMove(Dir::Right)),
            _ => None,
        }
    }
}

impl Element<CanEnd> for Prompt {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp<CanEnd>, Event> {
        match event.to_action(|e| e.to_go().or_else(|| e.to_cancel())) {
            Some(Action::Cancel) => Ok(Resp::end(None)),
            Some(Action::Go) => {
                if let Some(action) = self.get_action() {
                    Ok(Resp::end(action))
                } else {
                    Ok(Resp::end(Action::Show(format!(
                        "unknown command `{}`",
                        self.input.get_text()
                    ))))
                }
            }
            _ => self.input.handle(state, event).map(Resp::into_can_end),
        }
    }
}

impl Visual for Prompt {
    fn render(&self, state: &State, frame: &mut Rect) {
        frame.with(|f| self.input.render(state, f));
    }
}

pub struct Show {
    pub label: Label,
}

impl Element<CanEnd> for Show {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp<CanEnd>, Event> {
        match event.to_action(|e| e.to_cancel()) {
            Some(Action::Cancel) => Ok(Resp::end(None)),
            _ => Err(event),
        }
    }
}

impl Visual for Show {
    fn render(&self, state: &State, frame: &mut Rect) {
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

impl Element<CanEnd> for Confirm {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp<CanEnd>, Event> {
        match event.to_action(|e| e.to_yes().or_else(|| e.to_no()).or_else(|| e.to_cancel())) {
            Some(Action::Yes) => Ok(Resp::end(Some(self.action.clone()))),
            Some(Action::No | Action::Cancel) => Ok(Resp::end(None)),
            // All other events get swallowed
            _ => Ok(Resp::handled(None)),
        }
    }
}

impl Visual for Confirm {
    fn render(&self, state: &State, frame: &mut Rect) {
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
    pub selected: usize,
    pub options: Vec<BufferId>,
}

impl Element<CanEnd> for Switcher {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp<CanEnd>, Event> {
        match event.to_action(|e| {
            e.to_cancel()
                .or_else(|| e.to_go())
                .or_else(|| e.to_move().map(Action::Move))
        }) {
            Some(Action::Move(Dir::Up)) => {
                self.selected = (self.selected + self.options.len() - 1) % self.options.len();
                Ok(Resp::handled(None))
            }
            Some(Action::Move(Dir::Down)) => {
                self.selected = (self.selected + 1) % self.options.len();
                Ok(Resp::handled(None))
            }
            Some(Action::Go) => Ok(Resp::end(
                if let Some(buffer) = self.options.get(self.selected) {
                    Some(Action::SwitchBuffer(*buffer))
                } else {
                    None
                },
            )),
            Some(Action::Cancel) => Ok(Resp::end(None)),
            // All other events get swallowed
            _ => Ok(Resp::handled(None)),
        }
    }
}

impl Visual for Switcher {
    fn render(&self, state: &State, frame: &mut Rect) {
        for (i, buffer) in self.options.iter().enumerate() {
            let Some(buffer) = state.buffers.get(*buffer) else {
                continue;
            };
            frame
                .rect(
                    [
                        0,
                        frame.size()[1].saturating_sub(3 + self.options.len()) + i,
                    ],
                    [frame.size()[0], 1],
                )
                .with_bg(if self.selected == i {
                    state.theme.select_bg
                } else {
                    state.theme.ui_bg
                })
                .fill(' ')
                .text([0, 0], buffer.path.display().to_string().chars());
        }
    }
}
