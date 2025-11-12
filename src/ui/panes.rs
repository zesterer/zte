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

pub struct VBox {
    selected: usize,
    panes: Vec<Pane>,
    last_area: Area,
}

impl Element<()> for VBox {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp<()>, Event> {
        match event.to_action(|e| {
            e.to_pane_move()
                .map(Action::PaneMove)
                .or_else(|| e.to_pane_open().map(Action::PaneOpen))
                .or_else(|| e.to_pane_close())
        }) {
            Some(Action::PaneMove(Dir::Up)) => {
                self.selected = (self.selected + self.panes.len() - 1) % self.panes.len();
                Ok(Resp::handled(None))
            }
            Some(Action::PaneMove(Dir::Down)) => {
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
                    Dir::Up => self.selected.clamp(0, self.panes.len()),
                    Dir::Down => (self.selected + 1).min(self.panes.len()),
                    Dir::Left | Dir::Right => return Err(event),
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
            Some(action @ Action::Mouse(m_action, pos, is_ctrl, drag_id)) => {
                for (i, pane) in self.panes.iter_mut().enumerate() {
                    if pane.last_area.contains(pos).is_some() {
                        if matches!(m_action, MouseAction::Click) {
                            self.selected = i;
                        }
                        match &mut pane.kind {
                            PaneKind::Doc(doc) => {
                                return doc
                                    .handle(state, action.clone().into())
                                    .map(Resp::into_can_end);
                            }
                            PaneKind::Empty => {}
                        }
                        break;
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
                        PaneKind::Doc(doc) => doc.handle(state, event).map(Resp::into_can_end),
                    }
                } else {
                    // No active pane, don't handle
                    Err(event)
                }
            }
        }
    }
}

impl Visual for VBox {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        let n = self.panes.len();
        let frame_h = frame.size()[1];
        let boundary = |i| frame_h * i / n;

        self.last_area = frame.area();

        for (i, pane) in self.panes.iter_mut().enumerate() {
            let (y0, y1) = (boundary(i), boundary(i + 1));

            // Draw pane contents
            frame
                .rect([0, y0], [frame.size()[0], y1 - y0])
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

pub struct Panes {
    selected: usize,
    vboxes: Vec<VBox>,
    last_area: Area,
}

impl Panes {
    pub fn new(state: &mut State, buffers: &[BufferId]) -> Self {
        Self {
            selected: 0,
            vboxes: buffers
                .iter()
                .map(|b| VBox {
                    selected: 0,
                    panes: vec![Pane {
                        kind: PaneKind::Doc(Doc::new(state, *b)),
                        last_area: Area::default(),
                    }],
                    last_area: Area::default(),
                })
                .collect(),
            last_area: Default::default(),
        }
    }

    pub fn selected_mut(&mut self) -> Option<&mut Pane> {
        let vbox = self.vboxes.get_mut(self.selected)?;
        vbox.panes.get_mut(vbox.selected)
    }

    pub fn selected_vbox_mut(&mut self) -> Option<&mut VBox> {
        self.vboxes.get_mut(self.selected)
    }
}

impl Element for Panes {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp, Event> {
        match event.to_action(|e| {
            e.to_pane_move()
                .map(Action::PaneMove)
                .or_else(|| e.to_pane_open().map(Action::PaneOpen))
                .or_else(|| e.to_pane_close())
        }) {
            Some(Action::PaneMove(Dir::Left)) => {
                self.selected = (self.selected + self.vboxes.len() - 1) % self.vboxes.len();
                Ok(Resp::handled(None))
            }
            Some(Action::PaneMove(Dir::Right)) => {
                self.selected = (self.selected + 1) % self.vboxes.len();
                Ok(Resp::handled(None))
            }
            Some(Action::PaneOpen(dir @ (Dir::Left | Dir::Right))) => {
                let new_idx = match dir {
                    Dir::Left => self.selected.clamp(0, self.vboxes.len()),
                    Dir::Right => (self.selected + 1).min(self.vboxes.len()),
                    _ => unreachable!(),
                };
                let kind = match state.buffers.keys().next() {
                    Some(b) => PaneKind::Doc(Doc::new(state, b)),
                    None => PaneKind::Empty,
                };
                self.vboxes.insert(
                    new_idx,
                    VBox {
                        selected: 0,
                        panes: vec![Pane {
                            kind,
                            last_area: Area::default(),
                        }],
                        last_area: Area::default(),
                    },
                );
                self.selected = new_idx;
                Ok(Resp::handled(None))
            }
            Some(action @ Action::Mouse(m_action, pos, is_ctrl, drag_id)) => {
                for (i, vbox) in self.vboxes.iter_mut().enumerate() {
                    if vbox.last_area.contains(pos).is_some() {
                        if matches!(m_action, MouseAction::Click) {
                            self.selected = i;
                        }
                        let resp = vbox.handle(state, action.clone().into())?;
                        if resp.is_end() {
                            self.vboxes.remove(self.selected);
                            self.selected = self.selected.min(self.vboxes.len()).saturating_sub(1);
                        }
                        return Ok(Resp::handled(resp.event));
                    }
                }
                Ok(Resp::handled(None))
            }
            // Pass anything else through to the active pane
            action => {
                let mut to_handle = self.selected;
                // Set selected vbox on mouse click
                if let Some(Action::Mouse(ref m_action, pos, is_ctrl, drag_id)) = action {
                    for (i, vbox) in self.vboxes.iter_mut().enumerate() {
                        if vbox.last_area.contains(pos).is_some() {
                            if matches!(m_action, MouseAction::Click) {
                                self.selected = i;
                            }
                            to_handle = i;
                            break;
                        }
                    }
                }

                if let Some(vbox) = self.vboxes.get_mut(to_handle) {
                    // Pass to vbox
                    let resp = vbox.handle(state, event)?;
                    if resp.is_end() {
                        self.vboxes.remove(self.selected);
                        self.selected = self.selected.min(self.vboxes.len().saturating_sub(1));
                    }
                    Ok(Resp::handled(resp.event))
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
        let n = self.vboxes.len();
        let frame_w = frame.size()[0];
        let boundary = |i| frame_w * i / n;

        self.last_area = frame.area();

        for (i, vbox) in self.vboxes.iter_mut().enumerate() {
            let (x0, x1) = (boundary(i), boundary(i + 1));

            // Draw pane contents
            frame
                .rect([x0, 0], [x1 - x0, frame.size()[1]])
                .with_focus(self.selected == i)
                .with(|frame| vbox.render(state, frame));
        }
    }
}
