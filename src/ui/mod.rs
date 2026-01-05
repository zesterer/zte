mod doc;
mod input;
mod panes;
mod prompt;
mod root;
mod search;
mod status;

pub use self::{
    doc::{Doc, Finder},
    input::Input,
    panes::Panes,
    prompt::{Confirm, Opener, Prompt, Show, Switcher},
    root::Root,
    search::Searcher,
    status::Status,
};

use super::*;

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
    pub fn into_ended(self) -> Option<End> {
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
    pub focus: usize,
    pub selected: usize,
    // (score, option)
    pub options: Vec<T>,
    pub ranking: Vec<usize>,
    last_area: Area,
}

impl<T> Options<T> {
    pub fn new(options: impl IntoIterator<Item = T>) -> Self {
        let (ranking, options) = options.into_iter().enumerate().unzip();
        Self {
            focus: 0,
            selected: 0,
            options,
            ranking,
            last_area: Area::default(),
        }
    }

    pub fn selected(&self) -> Option<&T> {
        self.options.get(*self.ranking.get(self.selected)?)
    }

    pub fn set_options<F: FnMut(&T) -> Option<S>, S: Ord + Clone>(
        &mut self,
        options: impl IntoIterator<Item = T>,
        f: F,
    ) {
        self.options = options.into_iter().collect();
        self.apply_scoring(f);
    }

    pub fn apply_scoring<F: FnMut(&T) -> Option<S>, S: Ord + Clone>(&mut self, mut f: F) {
        let mut ranking = self
            .options
            .iter()
            .enumerate()
            .filter_map(|(i, o)| Some((i, f(o)?)))
            .collect::<Vec<_>>();
        ranking.sort_by_key(|(_, score)| score.clone());
        self.ranking = ranking.into_iter().map(|(i, _)| i).collect();
        self.selected = 0;
    }

    pub fn requested_height(&self) -> usize {
        2 + self.ranking.len()
    }

    fn scroll(&mut self, dir: Dir, dist: Dist) {
        let dist = match dist {
            Dist::Char => 1,
            Dist::Page => unimplemented!(),
            Dist::Doc => self.ranking.len(),
        };
        match dir {
            Dir::Up => {
                if self.selected == 0 {
                    self.selected = self.ranking.len().saturating_sub(1);
                } else {
                    self.selected = self.selected.saturating_sub(dist);
                }
            }
            Dir::Down => {
                if self.selected == self.ranking.len().saturating_sub(1) {
                    self.selected = 0;
                } else {
                    self.selected =
                        (self.selected + dist).min(self.ranking.len().saturating_sub(1));
                }
            }
            _ => {}
        }
    }
}

impl<T> Element<T> for Options<T> {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp<T>, Event> {
        match event.to_action(|e| e.to_go().or_else(|| e.to_move())) {
            Some(Action::Move(
                dir @ (Dir::Up | Dir::Down),
                dist @ (Dist::Char | Dist::Doc),
                false,
                false,
            )) => {
                self.scroll(dir, dist);
                Ok(Resp::handled(None))
            }
            Some(Action::Mouse(MouseAction::Click, pos, _, _))
                if self.last_area.contains(pos).is_some() =>
            {
                if let Some(pos) = self.last_area.contains(pos) {
                    self.selected =
                        ((pos[1] - self.focus as isize).max(0) as usize).min(self.ranking.len());
                }
                Ok(Resp::handled(None))
            }
            Some(Action::Mouse(MouseAction::Scroll(dir @ (Dir::Up | Dir::Down)), pos, _, _))
                if self.last_area.contains(pos).is_some() =>
            {
                self.scroll(dir, Dist::Char);
                Ok(Resp::handled(None))
            }
            Some(Action::Go) => {
                if self.selected < self.ranking.len() {
                    Ok(Resp::end_with(
                        self.options.remove(self.ranking[self.selected]),
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

        self.last_area = frame.area();

        self.focus = self
            .focus
            .max(
                self.selected
                    .saturating_sub(frame.size()[1].saturating_sub(1)),
            )
            .min(self.selected);

        for (row, (i, idx)) in self.ranking.iter().enumerate().skip(self.focus).enumerate() {
            let option = &mut self.options[*idx];
            frame
                .rect([0, row], [frame.size()[0], 1])
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
