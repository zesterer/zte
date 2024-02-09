use super::*;
use std::str::FromStr;

pub enum Command {
    Quit,
    Help,
    Version,
}

impl FromStr for Command {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "q" | "quit" => Ok(Command::Quit),
            "version" => Ok(Command::Version),
            "?" | "help" => Ok(Command::Help),
            _ => Err(()),
        }
    }
}

pub struct Prompt {
    pub input: Input,
}

impl Prompt {
    pub fn get_action(&self) -> Option<Action> {
        match self.input.get_text().as_str() {
            "quit" => Some(Action::Quit),
            "version" => Some(Action::Show(format!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")))),
            "help" => Some(Action::Show(format!("Temporary help info:\n- quit\n- version\n- help"))),
            _ => None,
        }
    }
}

impl Element<CanEnd> for Prompt {
    fn handle(&mut self, event: Event) -> Result<Resp<CanEnd>, Event> {
        match event.to_action(|e| if e.is_cancel() {
            Some(Action::Cancel)
        } else if e.is_go() {
            Some(Action::Go)
        } else if e.is_prompt() {
            Some(Action::OpenPrompt)
        } else {
            None
        }) {
            Some(Action::Cancel /*| Action::Prompt*/) => Ok(Resp::end(None)),
            Some(Action::Go) => if let Some(action) = self.get_action() {
                Ok(Resp::end(action))
            } else {
                Ok(Resp::end(Action::Show(format!("unknown command `{}`", self.input.get_text()))))
            },
            _ => self.input.handle(event).map(Resp::into_can_end),
        }
    }
}

impl Visual for Prompt {
    fn render(&self, state: &State, frame: &mut Rect) {    
        frame
            .rect([0, frame.size()[1].saturating_sub(1)], [frame.size()[0], 1])
            .with_bg(state.theme.status_bg)
            .with(|f| self.input.render(state, f));
    }
}

pub struct Show {
    pub label: Label,
}

impl Element<CanEnd> for Show {
    fn handle(&mut self, event: Event) -> Result<Resp<CanEnd>, Event> {
        match event.to_action(|e| if e.is_cancel() {
            Some(Action::Cancel)
        } else {
            None
        }) {
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
            &mut frame.rect([0, frame.size()[1].saturating_sub(1 + lines)], [frame.size()[0], lines]),
        );
    }
}

pub struct Confirm {
    pub label: Label,
    pub action: Action,
}

impl Element<CanEnd> for Confirm {
    fn handle(&mut self, event: Event) -> Result<Resp<CanEnd>, Event> {
        match event.to_action(|e| if e.is_cancel() || e.to_char() == Some('n') {
            Some(Action::Cancel)
        } else if e.to_char() == Some('y') {
            Some(Action::Go)
        } else {
            None
        }) {
            Some(Action::Go) => Ok(Resp::end(Some(self.action.clone()))),
            Some(Action::Cancel) => Ok(Resp::end(None)),
            _ => Err(event),
        }
    }
}

impl Visual for Confirm {
    fn render(&self, state: &State, frame: &mut Rect) {
        let lines = self.label.lines().count();
        self.label.render(
            state,
            &mut frame.rect([0, frame.size()[1].saturating_sub(1 + lines)], [frame.size()[0], lines]),
        );
    }
}
