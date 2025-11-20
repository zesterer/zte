use super::*;
use crate::state::BufferId;

pub enum PaneKind {
    Empty,
    Doc(Doc),
}

enum PaneTask {
    Opener(Opener),
    Switcher(Switcher),
}

pub struct Pane {
    kind: PaneKind,
    last_area: Area,
    task: Option<PaneTask>,
}

impl Element for Pane {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp, Event> {
        match event.to_action(|_| None) {
            Some(Action::OpenOpener(path)) => {
                self.task = Some(PaneTask::Opener(Opener::new(path)));
                Ok(Resp::handled(None))
            }
            Some(Action::OpenSwitcher) => {
                self.task = Some(PaneTask::Switcher(Switcher::new(state.most_recent())));
                Ok(Resp::handled(None))
            }
            _ => {
                let event = if let Some(task) = &mut self.task {
                    let resp = match task {
                        PaneTask::Opener(opener) => opener.handle(state, event),
                        PaneTask::Switcher(switcher) => switcher.handle(state, event),
                    };
                    match resp {
                        Ok(resp) => {
                            if resp.is_end() {
                                self.task = None;
                            }
                            return Ok(Resp::handled(resp.event));
                        }
                        Err(event) => event,
                    }
                } else {
                    event
                };

                match &mut self.kind {
                    PaneKind::Empty => Err(event),
                    PaneKind::Doc(doc) => doc.handle(state, event),
                }
            }
        }
    }
}

impl Visual for Pane {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        let remaining_space = match &mut self.task {
            Some(PaneTask::Opener(opener)) => {
                opener.render(state, frame);
                None
            }
            Some(PaneTask::Switcher(switcher)) => {
                let switcher_h = switcher.requested_height();
                switcher.render(
                    state,
                    &mut frame.rect([0, frame.size()[1] - switcher_h], [!0, !0]),
                );
                Some(([0, 0], [!0, frame.size()[1] - switcher_h]))
            }
            None => Some(([0, 0], [!0, !0])),
        };

        if let Some((pos, sz)) = remaining_space {
            match &mut self.kind {
                PaneKind::Empty => {}
                PaneKind::Doc(doc) => doc.render(state, &mut frame.rect(pos, sz)),
            }
        }
    }
}

pub struct HBox {
    selected: usize,
    panes: Vec<Pane>,
    last_area: Area,
    size_weight: f32,
}

impl Element<()> for HBox {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp<()>, Event> {
        match event.to_action(|e| {
            e.to_pane_move()
                .map(Action::PaneMove)
                .or_else(|| e.to_pane_open().map(Action::PaneOpen))
                .or_else(|| e.to_pane_close())
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
                    if self.panes.is_empty() {
                        Ok(Resp::end(None))
                    } else {
                        Ok(Resp::handled(None))
                    }
                } else {
                    Err(event)
                }
            }
            Some(Action::PaneOpen(dir)) => {
                let new_idx = match dir {
                    Dir::Left => self.selected.clamp(0, self.panes.len()),
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
                        task: None,
                    },
                );
                self.selected = new_idx;
                Ok(Resp::handled(None))
            }
            Some(action @ Action::Mouse(m_action, pos, is_ctrl, drag_id)) => {
                for (i, pane) in self.panes.iter_mut().enumerate() {
                    if pane.last_area.contains(pos).is_some() {
                        if matches!(m_action, MouseAction::Click) {
                            self.selected = i;
                        }
                        return pane
                            .handle(state, action.clone().into())
                            .map(Resp::into_can_end);
                    }
                }
                Ok(Resp::handled(None))
            }
            // Pass anything else through to the active pane
            _ => {
                if let Some(pane) = self.panes.get_mut(self.selected) {
                    // Pass to pane
                    pane.handle(state, event).map(Resp::into_can_end)
                } else {
                    // No active pane, don't handle
                    Err(event)
                }
            }
        }
    }
}

impl Visual for HBox {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        let n = self.panes.len();
        let frame_w = frame.size()[0];
        let boundary = |i| frame_w * i / n;

        self.last_area = frame.area();

        for (i, pane) in self.panes.iter_mut().enumerate() {
            let (x0, x1) = (boundary(i), boundary(i + 1));

            // Draw pane contents
            frame
                .rect([x0, 0], [x1 - x0, frame.size()[0]])
                .with_focus(self.selected == i)
                .with(|frame| {
                    pane.last_area = frame.area();
                    pane.render(state, frame);
                });
        }
    }
}

pub struct Panes {
    selected: usize,
    hboxes: Vec<HBox>,
    last_area: Area,
}

impl Panes {
    pub fn new(state: &mut State, buffers: &[BufferId]) -> Self {
        Self {
            selected: 0,
            hboxes: buffers
                .iter()
                .map(|b| HBox {
                    selected: 0,
                    panes: vec![Pane {
                        kind: PaneKind::Doc(Doc::new(state, *b)),
                        last_area: Area::default(),
                        task: None,
                    }],
                    last_area: Area::default(),
                    size_weight: 1.0,
                })
                .collect(),
            last_area: Default::default(),
        }
    }

    pub fn selected_mut(&mut self) -> Option<&mut Pane> {
        let hbox = self.hboxes.get_mut(self.selected)?;
        hbox.panes.get_mut(hbox.selected)
    }

    pub fn selected_hbox_mut(&mut self) -> Option<&mut HBox> {
        self.hboxes.get_mut(self.selected)
    }

    fn rescale(&mut self) {
        let total_weight = self.hboxes.iter().map(|h| h.size_weight).sum::<f32>();
        let sz = self.last_area.size()[1] as f32;
        self.hboxes
            .iter_mut()
            .for_each(|h| h.size_weight = (h.size_weight / total_weight).max(3.0 / sz).min(10.0));
    }
}

impl Element for Panes {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp, Event> {
        let res = match event.to_action(|e| {
            e.to_pane_move()
                .map(Action::PaneMove)
                .or_else(|| e.to_pane_open().map(Action::PaneOpen))
                .or_else(|| e.to_pane_close())
                .or_else(|| e.to_pane_resize())
        }) {
            Some(Action::PaneMove(Dir::Up)) => {
                self.selected = (self.selected + self.hboxes.len() - 1) % self.hboxes.len();
                Ok(Resp::handled(None))
            }
            Some(Action::PaneMove(Dir::Down)) => {
                self.selected = (self.selected + 1) % self.hboxes.len();
                Ok(Resp::handled(None))
            }
            Some(Action::PaneOpen(dir @ (Dir::Up | Dir::Down))) => {
                let new_idx = match dir {
                    Dir::Up => self.selected.clamp(0, self.hboxes.len()),
                    Dir::Down => (self.selected + 1).min(self.hboxes.len()),
                    _ => unreachable!(),
                };
                let kind = match state.buffers.keys().next() {
                    Some(b) => PaneKind::Doc(Doc::new(state, b)),
                    None => PaneKind::Empty,
                };
                let size_weight = 1.0 / self.hboxes.len().max(1) as f32;
                self.hboxes.insert(
                    new_idx,
                    HBox {
                        selected: 0,
                        panes: vec![Pane {
                            kind,
                            last_area: Area::default(),
                            task: None,
                        }],
                        last_area: Area::default(),
                        size_weight,
                    },
                );
                self.selected = new_idx;
                Ok(Resp::handled(None))
            }
            Some(Action::PaneResize(by)) => {
                if let Some(hbox) = self.hboxes.get_mut(self.selected) {
                    hbox.size_weight *= 1.3f32.powi(by);
                }
                Ok(Resp::handled(None))
            }
            Some(action @ Action::Mouse(m_action, pos, is_ctrl, drag_id)) => {
                for (i, hbox) in self.hboxes.iter_mut().enumerate() {
                    if hbox.last_area.contains(pos).is_some() {
                        if matches!(m_action, MouseAction::Click) {
                            self.selected = i;
                        }
                        let resp = hbox.handle(state, action.clone().into())?;
                        if resp.is_end() {
                            self.hboxes.remove(self.selected);
                            self.selected = self.selected.min(self.hboxes.len()).saturating_sub(1);
                        }
                        return Ok(Resp::handled(resp.event));
                    }
                }
                Ok(Resp::handled(None))
            }
            // Pass anything else through to the active pane
            action => {
                let mut to_handle = self.selected;
                // Set selected hbox on mouse click
                if let Some(Action::Mouse(ref m_action, pos, is_ctrl, drag_id)) = action {
                    for (i, hbox) in self.hboxes.iter_mut().enumerate() {
                        if hbox.last_area.contains(pos).is_some() {
                            if matches!(m_action, MouseAction::Click) {
                                self.selected = i;
                            }
                            to_handle = i;
                            break;
                        }
                    }
                }

                if let Some(hbox) = self.hboxes.get_mut(to_handle) {
                    // Pass to hbox
                    let resp = hbox.handle(state, event)?;
                    if resp.is_end() {
                        self.hboxes.remove(self.selected);
                        self.selected = self.selected.min(self.hboxes.len().saturating_sub(1));
                    }
                    Ok(Resp::handled(resp.event))
                } else {
                    // No active pane, don't handle
                    Err(event)
                }
            }
        };
        self.rescale();
        res
    }
}

impl Visual for Panes {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        let n = self.hboxes.len();
        if n == 0 {
            return;
        }

        let total_weight = self.hboxes.iter().map(|h| h.size_weight).sum::<f32>();

        self.last_area = frame.area();

        let mut y0 = 0;
        for (i, hbox) in self.hboxes.iter_mut().enumerate() {
            let y1 = if i == n - 1 {
                frame.size()[1]
            } else {
                y0 + ((hbox.size_weight * frame.size()[1] as f32 / total_weight)
                    .round()
                    .max(1.0) as usize)
                    .min(/*frame.size()[1] - (n - 1) * 3*/ !0)
            };

            // Draw pane contents
            frame
                .rect([0, y0], [frame.size()[0], y1.saturating_sub(y0)])
                .with_focus(self.selected == i)
                .with(|frame| hbox.render(state, frame));

            y0 = y1;
        }
    }
}
