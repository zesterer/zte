mod doc;
mod input;
mod panes;
mod prompt;
mod root;
mod search;
mod term;

pub use self::{
    doc::{Doc, Finder},
    input::Input,
    panes::Tabs,
    prompt::{Confirm, FileBrowser, FileBrowserMode, Prompt, Show, Switcher},
    root::Root,
    search::Searcher,
    term::{Term, TermWindow},
};

use super::*;

use crate::{
    Action, Dir, Event, State,
    terminal::{Color, CursorStyle, Rect},
};

pub enum CannotEnd {}

#[derive(Debug)]
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
    fn render(&mut self, state: &mut State, frame: &mut Rect);
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
    fn render(&mut self, _state: &mut State, frame: &mut Rect) {
        frame.with(|frame| {
            for (idx, line) in self.lines().enumerate() {
                frame.text([0, idx as isize], &line);
            }
        });
    }
}

/// List selection
pub struct Options<T> {
    pub focus: isize,
    pub selected: usize,
    // (score, option)
    pub options: Vec<T>,
    pub ranking: Vec<usize>,
    scroller: Scroller,
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
            scroller: Scroller::default().with_clamp_focus(),
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
        // Pretty much just a reset
        *self = Self::new(options);
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
        self.focus = 0;
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

        // Clamp focus to selected idx
        self.focus = self
            .focus
            .max(
                self.selected
                    .saturating_sub(self.last_area.size()[1].saturating_sub(1))
                    as isize,
            )
            .min(self.selected as isize);
    }
}

impl<T> Element<T> for Options<T> {
    fn handle(&mut self, _state: &mut State, event: Event) -> Result<Resp<T>, Event> {
        let event = match self
            .scroller
            .handle(event, self.ranking.len(), [&mut 0, &mut self.focus])
        {
            Ok(resp) => return Ok(Resp::handled(resp.event)),
            Err(event) => event,
        };

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
                    let new_selected =
                        ((pos[1] + self.focus).max(0) as usize).min(self.ranking.len());
                    if self.selected == new_selected {
                        return Ok(Resp::end_with(
                            self.options.remove(self.ranking[self.selected]),
                            None,
                        ));
                    } else {
                        self.selected = new_selected;
                    }
                }
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
    fn render(&mut self, state: &mut State, outer_frame: &mut Rect) {
        let mut frame = outer_frame.with_border(
            if outer_frame.has_focus() {
                &state.theme.focus_border
            } else {
                &state.theme.border
            },
            None,
        );

        self.last_area = frame.area();

        for (row, (i, idx)) in self
            .ranking
            .iter()
            .enumerate()
            .skip(self.focus.max(0) as usize)
            .take(frame.size()[1])
            .enumerate()
        {
            let option = &mut self.options[*idx];
            frame
                .rect([0, row], [frame.size()[0], 1])
                .with_theme(if self.selected == i {
                    Some(state.theme.unfocus_select)
                } else {
                    None
                })
                .fill(' ')
                .with(|f| option.render(state, f));
        }

        self.scroller
            .render(outer_frame, self.ranking.len(), [0, self.focus]);
    }
}

#[derive(Clone, Default)]
pub struct Scroller {
    // Remember the last area for things like scrolling
    pub last_area: Area,
    pub last_scroll_pos: Option<([isize; 2], usize, usize)>,
    pub scroll_grab: Option<(usize, isize)>,
    clamp_focus: bool,
}

impl Scroller {
    pub fn with_clamp_focus(mut self) -> Self {
        self.clamp_focus = true;
        self
    }

    fn handle(
        &mut self,
        event: Event,
        line_count: usize,
        focus: [&mut isize; 2],
    ) -> Result<Resp, Event> {
        let res = match event.to_action(|_| None) {
            Some(Action::Mouse(MouseAction::Scroll(dir), pos, _, _))
                if self.last_area.contains(pos).is_some() =>
            {
                let dist = [1, 1];
                let dfocus = match dir {
                    Dir::Up => [0, -1],
                    Dir::Down => [0, 1],
                    Dir::Left => [-1, 0],
                    Dir::Right => [1, 0],
                };
                *focus[0] += dfocus[0] * dist[0] as isize;
                *focus[1] += dfocus[1] * dist[1] as isize;
                Ok(Resp::handled(None))
            }
            Some(Action::Mouse(MouseAction::Click, pos, Modifiers::NONE, drag_id))
                if self.last_area.contains(pos).is_some() =>
            {
                if let Some((scroll_pos, h, _)) = self.last_scroll_pos
                    && let Some(pos) = self.last_area.contains(pos)
                    && scroll_pos[0] == pos[0]
                    && (scroll_pos[1]..=scroll_pos[1] + h as isize).contains(&pos[1])
                {
                    self.scroll_grab = Some((drag_id, pos[1] - scroll_pos[1]));
                    Ok(Resp::handled(None))
                } else {
                    Err(event)
                }
            }
            Some(Action::Mouse(MouseAction::Drag, pos, Modifiers::NONE, drag_id))
                if self.last_area.contains(pos).is_some()
                    && self.scroll_grab.map_or(false, |(di, _)| di == drag_id) =>
            {
                if let Some((_, offset)) = self.scroll_grab
                    && let Some((_, _, frame_sz)) = self.last_scroll_pos
                {
                    *focus[1] = ((self.last_area.translate(pos)[1] - offset).max(0) as usize
                        * line_count
                        / frame_sz) as isize;
                }
                Ok(Resp::handled(None))
            }
            _ => Err(event),
        };

        // Limit focus to the content area
        let limit = if self.clamp_focus {
            [
                !0,
                line_count.saturating_sub(self.last_area.size()[1].saturating_sub(2)),
            ]
        } else {
            [!0, line_count]
        };
        for i in 0..2 {
            *focus[i] = (*focus[i]).max(0).min(limit[i] as isize);
        }

        res
    }

    fn render(&mut self, frame: &mut Rect, line_count: usize, focus: [isize; 2]) {
        self.last_area = frame.area();

        let frame_sz = frame.size()[1].saturating_sub(2).max(1);
        let scroll_sz = (frame_sz * frame_sz / line_count.max(1))
            .max(1)
            .min(frame_sz);
        self.last_scroll_pos = if scroll_sz != frame_sz {
            let lines2 = line_count.saturating_sub(frame_sz).max(1);
            let offset = frame_sz.saturating_sub(scroll_sz)
                * (focus[1].max(0) as usize).min(lines2)
                / lines2;
            let pos = [frame.size()[0].saturating_sub(1), 1 + offset];
            frame
                .rect(pos, [1, scroll_sz])
                .with_bg(Color::White)
                .fill(' ');
            Some((pos.map(|e| e as isize), scroll_sz, frame_sz))
        } else {
            None
        };
    }
}
