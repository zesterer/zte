use super::*;
use crate::state::BufferId;

pub enum PaneKind {
    Empty,
    Doc(Doc),
}

pub struct Pane {
    kind: PaneKind,
    last_area: Area,
}

pub struct Panes {
    selected: usize,
    panes: Vec<Pane>,
    last_area: Area,
}

impl Panes {
    pub fn new(state: &mut State, buffers: &[BufferId]) -> Self {
        Self {
            selected: 0,
            panes: buffers
                .iter()
                .map(|b| Pane {
                    kind: PaneKind::Doc(Doc::new(state, *b)),
                    last_area: Area::default(),
                })
                .collect(),
            last_area: Default::default(),
        }
    }

    pub fn selected_mut(&mut self) -> Option<&mut Pane> {
        self.panes.get_mut(self.selected)
    }
}

impl Element for Panes {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp, Event> {
        match event.to_action(|e| {
            e.to_pane_move()
                .map(Action::PaneMove)
                .or_else(|| e.to_pane_open().map(Action::PaneOpen))
                .or_else(|| e.to_pane_close())
                .or_else(|| e.to_mouse(self.last_area))
        }) {
            Some(Action::PaneMove(Dir::Left)) => {
                self.selected = (self.selected + self.panes.len() - 1) % self.panes.len();
                Ok(Resp::handled(None))
            }
            Some(Action::PaneMove(Dir::Right)) => {
                self.selected = (self.selected + 1) % self.panes.len();
                Ok(Resp::handled(None))
            }
            Some(Action::PaneClose) => {
                if self.selected < self.panes.len() {
                    match self.panes.remove(self.selected).kind {
                        PaneKind::Empty => {}
                        PaneKind::Doc(doc) => doc.close(state),
                    }
                    self.selected = self.selected.clamp(0, self.panes.len().saturating_sub(1));
                    Ok(Resp::handled(None))
                } else {
                    Err(event)
                }
            }
            Some(Action::PaneOpen(dir)) => {
                let new_idx = match dir {
                    Dir::Left => self.selected.saturating_sub(1).clamp(0, self.panes.len()),
                    Dir::Right => (self.selected + 1).min(self.panes.len()),
                    Dir::Up | Dir::Down => return Err(event),
                };
                let kind = match state.buffers.keys().next() {
                    Some(b) => PaneKind::Doc(Doc::new(state, b)),
                    None => PaneKind::Empty,
                };
                self.panes.insert(
                    new_idx,
                    Pane {
                        kind,
                        last_area: Area::default(),
                    },
                );
                self.selected = new_idx;
                Ok(Resp::handled(None))
            }
            Some(Action::Mouse(action, pos, _)) => {
                for (i, pane) in self.panes.iter_mut().enumerate() {
                    if pane.last_area.contains(pos).is_some() {
                        if matches!(action, MouseAction::Click) {
                            self.selected = i;
                        }
                        match &mut pane.kind {
                            PaneKind::Doc(doc) => return doc.handle(state, event),
                            PaneKind::Empty => {}
                        }
                    }
                }
                Ok(Resp::handled(None))
            }
            // Pass anything else through to the active pane
            _ => {
                if let Some(pane) = self.panes.get_mut(self.selected) {
                    // Pass to pane
                    match &mut pane.kind {
                        PaneKind::Empty => Err(event),
                        PaneKind::Doc(doc) => doc.handle(state, event),
                    }
                } else {
                    // No active pane, don't handle
                    Err(event)
                }
            }
        }
    }
}

impl Visual for Panes {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        let n = self.panes.len();
        let frame_w = frame.size()[0];
        let boundary = |i| frame_w * i / n;

        self.last_area = frame.area();

        for (i, pane) in self.panes.iter_mut().enumerate() {
            let (x0, x1) = (boundary(i), boundary(i + 1));

            // Draw pane contents
            frame
                .rect([x0, 0], [x1 - x0, frame.size()[1]])
                .with_focus(self.selected == i)
                .with(|frame| {
                    pane.last_area = frame.area();
                    match &mut pane.kind {
                        PaneKind::Empty => {}
                        PaneKind::Doc(doc) => doc.render(state, frame),
                    }
                });
        }
    }
}
