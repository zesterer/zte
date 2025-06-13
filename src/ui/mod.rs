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
    prompt::{Confirm, Opener, Prompt, Show, Switcher},
    root::Root,
    status::Status,
};

use crate::{
    Action, Dir, Event, State,
    terminal::{Color, Rect},
};

pub enum CannotEnd {}

pub struct Resp<End = CannotEnd> {
    ended: Option<End>,
    pub event: Option<Event>,
}

impl Resp<CannotEnd> {
    pub fn into_can_end<End>(self) -> Resp<End> {
        Resp {
            ended: None,
            event: self.event,
        }
    }
}

impl<End> Resp<End> {
    pub fn end(event: Option<Event>) -> Self
    where
        End: Default,
    {
        Self::end_with(Default::default(), event)
    }

    pub fn end_with(end: End, event: Option<Event>) -> Self {
        Self {
            ended: Some(end),
            event: event.into(),
        }
    }

    pub fn handled(event: Option<Event>) -> Self {
        Self {
            ended: None,
            event: event.into(),
        }
    }

    pub fn is_end(&self) -> bool {
        self.ended.is_some()
    }
    pub fn into_ended(mut self) -> Option<End> {
        self.ended
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

impl Label {
    pub fn requested_height(&self) -> usize {
        self.0.lines().count()
    }
}

impl Visual for Label {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        frame.with(|frame| {
            for (idx, line) in self.lines().enumerate() {
                frame.text([0, idx as isize], &line);
            }
        });
    }
}

/// List selection
pub struct Options<T> {
    pub selected: usize,
    // (score, option)
    pub options: Vec<T>,
    pub ranking: Vec<usize>,
}

impl<T> Options<T> {
    pub fn new(options: impl IntoIterator<Item = T>) -> Self {
        let (ranking, options) = options.into_iter().enumerate().unzip();
        Self {
            selected: 0,
            options,
            ranking,
        }
    }

    pub fn set_options<F: FnMut(&T) -> Option<S>, S: Ord + Copy>(
        &mut self,
        options: impl IntoIterator<Item = T>,
        mut f: F,
    ) {
        self.options = options.into_iter().collect();
        self.apply_scoring(f);
    }

    pub fn apply_scoring<F: FnMut(&T) -> Option<S>, S: Ord + Copy>(&mut self, mut f: F) {
        let mut ranking = self
            .options
            .iter()
            .enumerate()
            .filter_map(|(i, o)| Some((i, f(o)?)))
            .collect::<Vec<_>>();
        ranking.sort_by_key(|(_, score)| *score);
        self.ranking = ranking.into_iter().map(|(i, _)| i).collect();
        self.selected = 0;
    }

    pub fn requested_height(&self) -> usize {
        2 + self.ranking.len()
    }
}

impl<T: Clone> Element<T> for Options<T> {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp<T>, Event> {
        match event.to_action(|e| e.to_go().or_else(|| e.to_move())) {
            Some(Action::Move(Dir::Up, false, _)) => {
                self.selected = (self.selected + self.ranking.len() - 1) % self.ranking.len();
                Ok(Resp::handled(None))
            }
            Some(Action::Move(Dir::Down, false, _)) => {
                self.selected = (self.selected + 1) % self.ranking.len();
                Ok(Resp::handled(None))
            }
            Some(Action::Go) => {
                if self.selected < self.ranking.len() {
                    Ok(Resp::end_with(
                        self.options[self.ranking[self.selected]].clone(),
                        None,
                    ))
                } else {
                    Err(event)
                }
            }
            _ => Err(event),
        }
    }
}

impl<T: Visual> Visual for Options<T> {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        let mut frame = frame.with_border(
            if frame.has_focus() {
                &state.theme.focus_border
            } else {
                &state.theme.border
            },
            None,
        );

        for (i, idx) in self.ranking.iter().enumerate() {
            let option = &mut self.options[*idx];
            frame
                .rect([0, i], [frame.size()[0], 1])
                .with_bg(if self.selected == i {
                    state.theme.select_bg
                } else {
                    Color::Reset
                })
                .fill(' ')
                .with(|f| option.render(state, f));
        }
    }
}
