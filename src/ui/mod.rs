mod doc;
mod input;
mod panes;
mod prompt;
mod root;
mod status;

pub use self::{
    doc::Doc,
    input::Input,
    panes::Panes,
    prompt::{Confirm, Prompt, Show, Switcher},
    root::Root,
    status::Status,
};

use crate::{
    Action, Dir, Event, State,
    terminal::{Color, Rect},
};

pub enum CannotEnd {}
pub struct CanEnd;

pub struct Resp<CanEnd = CannotEnd> {
    should_end: Option<CanEnd>,
    pub action: Option<Action>,
}

impl Resp<CanEnd> {
    pub fn end(action: impl Into<Option<Action>>) -> Self {
        Self {
            should_end: Some(CanEnd),
            action: action.into(),
        }
    }

    pub fn should_end(&self) -> bool {
        self.should_end.is_some()
    }
}

impl<T> Resp<T> {
    pub fn handled(action: impl Into<Option<Action>>) -> Self {
        Self {
            should_end: None,
            action: action.into(),
        }
    }

    pub fn into_can_end(self) -> Resp<CanEnd> {
        Resp {
            should_end: None,
            action: self.action,
        }
    }
}

pub trait Element<CanEnd = CannotEnd> {
    /// Attempt to handle an event.
    ///
    /// If handled, convert into a series of secondary actions.
    /// If unhandled, return the original event to be handled by a lower element.
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp<CanEnd>, Event>;
}

pub trait Visual {
    fn render(&mut self, state: &State, frame: &mut Rect);
}

pub struct Label(String);

impl std::ops::Deref for Label {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Visual for Label {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        frame.with_bg(state.theme.ui_bg).fill(' ').with(|frame| {
            for (idx, line) in self.lines().enumerate() {
                frame.text([0, idx as isize], line.chars());
            }
        });
    }
}
