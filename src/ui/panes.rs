use super::*;
use crate::state::{BufferId, TaskId};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

pub enum PaneKind {
    Empty,
    Doc(BufferId),
    Term(TermWindow),
}

enum PaneTask {
    FileBrowser(FileBrowser),
    Switcher(Switcher),
}

// Keep types small to save memory!
const _: () = assert!(core::mem::size_of::<PaneKind>() < 128);

pub struct Pane {
    kind: PaneKind,
    task: Option<Box<PaneTask>>,
    // Remember the cursor we use for each buffer
    docs: HashMap<BufferId, Doc>,
    last_area: Area,
}

impl Pane {
    fn should_close(&mut self, state: &mut State) -> bool {
        match &self.kind {
            PaneKind::Empty | PaneKind::Doc(_) => false,
            PaneKind::Term(term) => term.should_close(state),
        }
    }

    pub fn should_warn_close(&self) -> bool {
        matches!(&self.kind, PaneKind::Term(_))
    }

    fn doc_mut(&mut self, state: &mut State, buffer_id: BufferId) -> &mut Doc {
        self.docs
            .entry(buffer_id)
            .or_insert_with(|| Doc::new(state, buffer_id))
    }

    fn switch_task(&mut self, state: &mut State, task: TaskId) {
        state.set_most_recent(task);
        // Close existing task
        match core::mem::replace(&mut self.kind, PaneKind::Empty) {
            PaneKind::Doc(_) | PaneKind::Empty => {}
            PaneKind::Term(term) => term.close(state),
        }
        self.kind = match task {
            TaskId::Buffer(buffer_id) => PaneKind::Doc(buffer_id),
            TaskId::Term(term_id) => PaneKind::Term(state.switch_term(term_id)),
        };
    }

    pub fn close(self, state: &mut State) {
        match self.kind {
            PaneKind::Doc(_) | PaneKind::Empty => {}
            PaneKind::Term(term) => term.close(state),
        }
        for doc in self.docs.into_values() {
            doc.close(state);
        }
    }

    fn task_dir(&self, state: &mut State) -> PathBuf {
        let path = match &self.kind {
            PaneKind::Doc(buffer_id) => {
                if let Some(buf) = state.buffers.get(*buffer_id)
                    && let Some(path) = buf.path()
                {
                    path.parent().map(ToOwned::to_owned)
                } else {
                    None
                }
            }
            PaneKind::Term(term) => Some(state.terms[term.term].path.clone()),
            PaneKind::Empty => None,
        };
        path.unwrap_or_else(|| std::env::current_dir().expect("no cwd"))
    }
}

impl Element<()> for Pane {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp<()>, Event> {
        match event.to_action(|e| {
            e.to_open_op(&self.task_dir(state))
                .or_else(|| e.to_path_search())
        }) {
            Some(Action::NewTerm(path)) => {
                let path = path.unwrap_or_else(|| self.task_dir(state));
                match Term::new(path, state) {
                    Ok(term) => {
                        let term = state.create_term(term);
                        self.switch_task(state, TaskId::Term(term));
                        Ok(Resp::handled(None))
                    }
                    Err(err) => Ok(Resp::handled(Some(
                        Action::Show(Some(format!("Failed to spawn terminal")), format!("{err}"))
                            .into(),
                    ))),
                }
            }
            Some(Action::BeginSearch(needle)) => Ok(Resp::handled(Some(
                Action::OpenSearcher(self.task_dir(state), needle).into(),
            ))),
            Some(Action::OpenOpener(path)) => {
                self.task = Some(
                    PaneTask::FileBrowser(FileBrowser::new(path, FileBrowserMode::Opener)).into(),
                );
                Ok(Resp::handled(None))
            }
            Some(Action::OpenSaver(path)) => {
                self.task = Some(
                    PaneTask::FileBrowser(FileBrowser::new(path, FileBrowserMode::Save)).into(),
                );
                Ok(Resp::handled(None))
            }
            Some(Action::OpenMover(path)) => {
                self.task = Some(
                    PaneTask::FileBrowser(FileBrowser::new(path, FileBrowserMode::Move)).into(),
                );
                Ok(Resp::handled(None))
            }
            Some(Action::OpenSwitcher) => {
                let most_recent = state.most_recent_tasks();
                if most_recent.is_empty() {
                    Err(event)
                } else {
                    self.task = Some(PaneTask::Switcher(Switcher::new(most_recent)).into());
                    Ok(Resp::handled(None))
                }
            }
            Some(Action::NewFile) => {
                let buffer_id = state.new_anonymous();
                self.switch_task(state, TaskId::Buffer(buffer_id));
                Ok(Resp::handled(None))
            }
            Some(Action::OpenFile(path, range)) => match state.create(path) {
                Ok(buffer_id) => {
                    self.switch_task(state, TaskId::Buffer(buffer_id));
                    let cursor = self.doc_mut(state, buffer_id).cursor;
                    if let Some(buffer) = state.buffers.get_mut(buffer_id)
                        && let Some(range) = range
                    {
                        buffer.select_cursor(cursor, range);
                    }
                    Ok(Resp::handled(None))
                }
                Err(err) => Ok(Resp::handled(Some(
                    Action::Show(Some(format!("Could not open file")), format!("{err}")).into(),
                ))),
            },
            Some(Action::SwitchTask(new_task)) => {
                self.switch_task(state, new_task);
                Ok(Resp::handled(None))
            }
            _ => {
                let event = if let Some(task) = self.task.as_deref_mut() {
                    let resp = match task {
                        PaneTask::FileBrowser(browser) => browser.handle(state, event),
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

                let resp = match &mut self.kind {
                    PaneKind::Empty => return Err(event),
                    PaneKind::Doc(buffer_id) => {
                        let buffer_id = *buffer_id;
                        self.doc_mut(state, buffer_id).handle(state, event)?
                    }
                    PaneKind::Term(term) => term.handle(state, event)?,
                };

                if resp.is_end() {
                    // Close the current pane task if it asked to be ended
                    match core::mem::replace(&mut self.kind, PaneKind::Empty) {
                        PaneKind::Empty => {}
                        PaneKind::Doc(buffer_id) => {
                            self.docs.remove(&buffer_id).map(|d| d.close(state));
                        }
                        PaneKind::Term(term) => term.close(state),
                    }

                    // Switch to another buffer, or not
                    if let Some(new_buffer) = state.most_recent().first() {
                        self.switch_task(state, TaskId::Buffer(*new_buffer));
                        Ok(Resp::handled(None))
                    } else {
                        self.kind = PaneKind::Empty;
                        Ok(Resp::end(None))
                    }
                } else {
                    Ok(resp)
                }
            }
        }
    }
}

impl Visual for Pane {
    fn render(&mut self, state: &mut State, frame: &mut Rect) {
        let remaining_space = match self.task.as_deref_mut() {
            Some(PaneTask::FileBrowser(browser)) => {
                browser.render(state, frame);
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
                PaneKind::Doc(buffer_id) => {
                    let buffer_id = *buffer_id;
                    let is_focus = self.task.is_none();
                    self.doc_mut(state, buffer_id)
                        .render(state, &mut frame.with_focus(is_focus).rect(pos, sz))
                }
                PaneKind::Term(term) => term.render(
                    state,
                    &mut frame.with_focus(self.task.is_none()).rect(pos, sz),
                ),
            }
        }
    }
}

pub struct VBox {
    selected: usize,
    panes: Vec<Pane>,
    last_area: Area,
    size_weight: f32,
}

impl VBox {
    pub fn should_warn_close(&self) -> bool {
        self.panes.iter().any(|e| e.should_warn_close())
    }
}

impl Element<()> for VBox {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp<()>, Event> {
        match event.to_action(|e| {
            e.to_pane_move()
                .map(Action::PaneMove)
                .or_else(|| e.to_pane_open().map(Action::PaneOpen))
                .or_else(|| e.to_pane_close())
        }) {
            Some(Action::PaneMove(Dir::Up)) if self.panes.len() > 1 => {
                self.selected = (self.selected + self.panes.len() - 1) % self.panes.len();
                Ok(Resp::handled(None))
            }
            Some(Action::PaneMove(Dir::Down)) if self.panes.len() > 1 => {
                self.selected = (self.selected + 1) % self.panes.len();
                Ok(Resp::handled(None))
            }
            Some(Action::PaneClose | Action::PaneCloseForce) => {
                if self.selected < self.panes.len() {
                    self.panes.remove(self.selected).close(state);
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
                    Some(b) => PaneKind::Doc(b),
                    None => PaneKind::Empty,
                };
                self.panes.insert(
                    new_idx,
                    Pane {
                        kind,
                        last_area: Area::default(),
                        docs: HashMap::default(),
                        task: None,
                    },
                );
                self.selected = new_idx;
                Ok(Resp::handled(None))
            }
            // Pass anything else through to the active pane
            action => {
                let mut to_handle = self.selected;
                // Set selected vbox on mouse click
                if let Some(Action::Mouse(ref m_action, pos, _is_ctrl, _drag_id)) = action {
                    for (i, pane) in self.panes.iter_mut().enumerate() {
                        if pane.last_area.contains(pos).is_some() {
                            if matches!(m_action, MouseAction::Click) {
                                self.selected = i;
                            }
                            to_handle = i;
                            break;
                        }
                    }
                }

                if let Some(pane) = self.panes.get_mut(to_handle) {
                    // Pass to pane
                    let resp = pane.handle(state, event)?;
                    if resp.is_end() {
                        self.panes.remove(self.selected);
                        self.selected = self.selected.min(self.panes.len().saturating_sub(1));
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

impl Visual for VBox {
    fn render(&mut self, state: &mut State, frame: &mut Rect) {
        let n = self.panes.len();
        let frame_sz = frame.size()[1];
        let boundary = |i| frame_sz * i / n;

        self.last_area = frame.area();

        // Close panes that request to be closed
        for (i, pane) in self.panes.iter_mut().enumerate() {
            if pane.should_close(state) {
                self.panes.remove(i).close(state);
                self.selected = self.selected.clamp(0, self.panes.len().saturating_sub(1));
                state.wakeup.notify_one();
                break;
            }
        }

        for (i, pane) in self.panes.iter_mut().enumerate() {
            let (y0, y1) = (boundary(i), boundary(i + 1));

            // Draw pane contents
            frame
                .rect([0, y0], [frame.size()[0], y1 - y0])
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
    vboxes: Vec<VBox>,
    last_area: Area,
    name: Option<String>,
}

impl Panes {
    fn rescale(&mut self) {
        let total_weight = self.vboxes.iter().map(|h| h.size_weight).sum::<f32>();
        let sz = self.last_area.size()[1] as f32;
        self.vboxes
            .iter_mut()
            .for_each(|h| h.size_weight = (h.size_weight / total_weight).max(3.0 / sz).min(10.0));
    }

    pub fn should_warn_close(&self) -> bool {
        self.vboxes.iter().any(|e| e.should_warn_close())
    }
}

impl Element for Panes {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp, Event> {
        // Update tab name
        if let Event::Tick = &event {
            let name = if let Some(vbox) = self.vboxes.get(self.selected)
                && let Some(pane) = vbox.panes.get(vbox.selected)
            {
                if let PaneKind::Doc(buffer_id) = &pane.kind
                    && let Some(buffer) = state.buffers.get(*buffer_id)
                    && let Some(path) = buffer.path()
                    && let Some(dir) = path.parent()
                {
                    Some(format!("{}", util::workspace_dir(dir).display()))
                } else if let PaneKind::Term(term) = &pane.kind {
                    state.terms[term.term].title.clone()
                } else {
                    None
                }
            } else {
                None
            };
            if name != self.name {
                self.name = name;
                state.needs_render = true;
            }
        }

        let res = match event.to_action(|e| {
            e.to_pane_move()
                .map(Action::PaneMove)
                .or_else(|| e.to_pane_open().map(Action::PaneOpen))
                .or_else(|| e.to_pane_close())
                .or_else(|| e.to_pane_resize())
        }) {
            Some(Action::PaneMove(Dir::Left)) if self.vboxes.len() > 1 => {
                self.selected = (self.selected + self.vboxes.len() - 1) % self.vboxes.len();
                Ok(Resp::handled(None))
            }
            Some(Action::PaneMove(Dir::Right)) if self.vboxes.len() > 1 => {
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
                    Some(b) => PaneKind::Doc(b),
                    None => PaneKind::Empty,
                };
                let size_weight = 1.0 / self.vboxes.len().max(1) as f32;
                self.vboxes.insert(
                    new_idx,
                    VBox {
                        selected: 0,
                        panes: vec![Pane {
                            kind,
                            docs: HashMap::default(),
                            task: None,
                            last_area: Area::default(),
                        }],
                        last_area: Area::default(),
                        size_weight,
                    },
                );
                self.selected = new_idx;
                Ok(Resp::handled(None))
            }
            Some(Action::PaneResize(by)) => {
                if let Some(vbox) = self.vboxes.get_mut(self.selected) {
                    vbox.size_weight *= 1.2f32.powi(by);
                }
                Ok(Resp::handled(None))
            }
            // Pass anything else through to the active pane
            action => {
                let mut to_handle = self.selected;
                // Set selected vbox on mouse click
                if let Some(Action::Mouse(ref m_action, pos, _is_ctrl, _drag_id)) = action {
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
        };
        self.rescale();
        res
    }
}

impl Visual for Panes {
    fn render(&mut self, state: &mut State, frame: &mut Rect) {
        let n = self.vboxes.len();
        if n == 0 {
            return;
        }

        let total_weight = self.vboxes.iter().map(|h| h.size_weight).sum::<f32>();

        self.last_area = frame.area();

        // Remove any empty vboxes
        for (i, vbox) in self.vboxes.iter_mut().enumerate() {
            if vbox.panes.is_empty() {
                self.vboxes.remove(i);
                self.selected = self.selected.min(self.vboxes.len().saturating_sub(1));
                self.rescale();
                state.wakeup.notify_one();
                break;
            }
        }

        let mut x0 = 0;
        for (i, vbox) in self.vboxes.iter_mut().enumerate() {
            let x1 = if i == n - 1 {
                frame.size()[0]
            } else {
                x0 + ((vbox.size_weight * frame.size()[0] as f32 / total_weight)
                    .round()
                    .max(1.0) as usize)
                    .min(/*frame.size()[1] - (n - 1) * 3*/ !0)
            };

            // Draw pane contents
            frame
                .rect([x0, 0], [x1.saturating_sub(x0), frame.size()[1]])
                .with_focus(self.selected == i)
                .with(|frame| vbox.render(state, frame));

            x0 = x1;
        }
    }
}

pub struct Tabs {
    selected: usize,
    pub(super) tabs: Vec<Panes>,
    last_area: Area,
    tab_view_timeout: Option<Instant>,
}

impl Tabs {
    pub fn new(state: &mut State, args: &Args) -> Self {
        Self {
            selected: 0,
            tabs: args
                .paths
                .iter()
                .map(Some)
                .chain(if args.paths.is_empty() {
                    Some(None)
                } else {
                    None
                })
                .filter_map(|path| {
                    let (buffer_id, task) = if let Some(path) = path {
                        if path.is_dir() {
                            (
                                state.new_anonymous(),
                                Some(PaneTask::FileBrowser(FileBrowser::new(
                                    path.clone(),
                                    FileBrowserMode::Opener,
                                ))),
                            )
                        } else {
                            (state.create(path.clone()).ok()?, None)
                        }
                    } else {
                        (state.new_anonymous(), None)
                    };
                    Some(Panes {
                        selected: 0,
                        vboxes: vec![VBox {
                            selected: 0,
                            panes: vec![Pane {
                                kind: PaneKind::Doc(buffer_id),
                                task: task.map(Into::into),
                                docs: HashMap::default(),
                                last_area: Area::default(),
                            }],
                            last_area: Area::default(),
                            size_weight: 1.0,
                        }],
                        last_area: Area::default(),
                        name: None,
                    })
                })
                .collect(),
            last_area: Default::default(),
            tab_view_timeout: None,
        }
    }

    fn reset_tab_timeout(&mut self) {
        self.tab_view_timeout = Some(Instant::now() + Duration::from_millis(800));
    }

    pub fn should_warn_close(&self) -> bool {
        self.tabs.iter().any(|t| t.should_warn_close())
    }
}

impl Element for Tabs {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp, Event> {
        if let Some(timeout) = &mut self.tab_view_timeout {
            if &Instant::now() > timeout {
                state.needs_render = true;
                self.tab_view_timeout = None;
            }
        }

        let res = match event.to_action(|e| {
            e.to_tab_move()
                .map(Action::TabMove)
                .or_else(|| e.to_tab_open().map(Action::TabOpen))
        }) {
            Some(Action::TabMove(Dir::Up)) if self.tabs.len() > 1 => {
                self.selected = (self.selected + self.tabs.len() - 1) % self.tabs.len();
                self.reset_tab_timeout();
                Ok(Resp::handled(None))
            }
            Some(Action::TabMove(Dir::Down)) if self.tabs.len() > 1 => {
                self.selected = (self.selected + 1) % self.tabs.len();
                self.reset_tab_timeout();
                Ok(Resp::handled(None))
            }
            Some(Action::TabOpen(dir @ (Dir::Up | Dir::Down))) => {
                let new_idx = match dir {
                    Dir::Up => self.selected.clamp(0, self.tabs.len()),
                    Dir::Down => (self.selected + 1).min(self.tabs.len()),
                    _ => unreachable!(),
                };
                let kind = match state.buffers.keys().next() {
                    Some(b) => PaneKind::Doc(b),
                    None => PaneKind::Empty,
                };
                let size_weight = 1.0 / self.tabs.len().max(1) as f32;
                self.tabs.insert(
                    new_idx,
                    Panes {
                        selected: 0,
                        vboxes: vec![VBox {
                            selected: 0,
                            panes: vec![Pane {
                                kind,
                                last_area: Area::default(),
                                docs: HashMap::default(),
                                task: None,
                            }],
                            last_area: Area::default(),
                            size_weight,
                        }],
                        last_area: Area::default(),
                        name: None,
                    },
                );
                self.reset_tab_timeout();
                self.selected = new_idx;
                Ok(Resp::handled(None))
            }
            // Pass anything else through to the active pane
            _ => {
                if let Some(tab) = self.tabs.get_mut(self.selected) {
                    // Pass to vbox
                    let resp = tab.handle(state, event)?;
                    if resp.is_end() {
                        self.tabs.remove(self.selected);
                        self.selected = self.selected.min(self.tabs.len().saturating_sub(1));
                    }
                    Ok(Resp::handled(resp.event))
                } else {
                    // No active pane, don't handle
                    Err(event)
                }
            }
        };
        res
    }
}

impl Visual for Tabs {
    fn render(&mut self, state: &mut State, frame: &mut Rect) {
        self.last_area = frame.area();

        // Remove any empty tabs
        for (i, tab) in self.tabs.iter_mut().enumerate() {
            if tab.vboxes.is_empty() {
                self.tabs.remove(i);
                self.selected = self.selected.min(self.tabs.len().saturating_sub(1));
                state.wakeup.notify_one();
                break;
            }
        }

        if let Some(tab) = self.tabs.get_mut(self.selected) {
            frame
                .with_focus(self.tab_view_timeout.is_none())
                .with(|frame| tab.render(state, frame));
        }

        if self.tab_view_timeout.is_some() {
            let mut frame = frame.mid([96, self.tabs.len() + 2]);
            let mut frame = frame.with_border(
                if frame.has_focus() {
                    &state.theme.focus_border
                } else {
                    &state.theme.border
                },
                Some("Tab switcher"),
            );
            for (i, tab) in self.tabs.iter().enumerate() {
                let name = if let Some(name) = &tab.name {
                    name
                } else {
                    &format!("Tab #{i}")
                };
                frame
                    .rect([0, i], [!0, 1])
                    .with_theme(if i == self.selected {
                        Some(state.theme.select)
                    } else {
                        None
                    })
                    .fill(' ')
                    .text([0, 0], name);
            }
        }
    }
}
