use super::*;
use crate::state::{BufferId, TaskId};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

trait PaneContainer: Element<()> + Visual + From<Pane> {
    fn close(self, state: &mut State);
    fn should_close(&mut self, state: &mut State) -> bool;
    fn last_area(&self) -> Area;
    fn selected_pane(&self) -> Option<&Pane>;
}

#[derive(Default)]
pub enum PaneKind {
    #[default]
    Empty,
    Doc(BufferId),
    Term(TermWindow),
}

enum PaneTask {
    FileBrowser(FileBrowser),
    Switcher(Switcher),
    Searcher(Searcher),
}

// Keep types small to save memory!
const _: () = assert!(core::mem::size_of::<PaneKind>() < 128);

#[derive(Default)]
pub struct Pane {
    kind: PaneKind,
    task: Option<Box<PaneTask>>,
    // Remember the cursor we use for each buffer
    docs: HashMap<BufferId, Doc>,
    last_area: Area,
}

impl PaneContainer for Pane {
    fn should_close(&mut self, state: &mut State) -> bool {
        match &self.kind {
            PaneKind::Empty | PaneKind::Doc(_) => false,
            PaneKind::Term(term) => term.should_close(state),
        }
    }

    fn close(self, state: &mut State) {
        match self.kind {
            PaneKind::Doc(_) | PaneKind::Empty => {}
            PaneKind::Term(term) => term.close(state),
        }
        for doc in self.docs.into_values() {
            doc.close(state);
        }
    }

    fn last_area(&self) -> Area {
        self.last_area
    }

    fn selected_pane(&self) -> Option<&Pane> {
        Some(self)
    }
}

impl Pane {
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
                .or_else(|| e.to_pane_close())
        }) {
            Some(Action::PaneClose | Action::PaneCloseForce) => Ok(Resp::end(None)),
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
            Some(Action::OpenSearcher(path, needle)) => {
                self.task = Some(
                    PaneTask::Searcher(Searcher::new(&path, needle.clone(), state.wakeup.clone()))
                        .into(),
                );
                Ok(Resp::handled(None))
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
                        PaneTask::Searcher(searcher) => searcher.handle(state, event),
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
            Some(PaneTask::Searcher(searcher)) => {
                searcher.render(state, frame);
                None
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

pub struct Child<T> {
    child: T,
    size_weight: f32,
}

pub struct Panes<T, const IS_VERTICAL: bool = true> {
    selected: usize,
    vboxes: Vec<Child<T>>,
    last_area: Area,
}

impl<T: From<Pane>, const IS_VERTICAL: bool> Panes<T, IS_VERTICAL> {
    fn rescale(&mut self) {
        let total_weight = self.vboxes.iter().map(|h| h.size_weight).sum::<f32>();
        let sz = self.last_area.size()[1] as f32;
        self.vboxes
            .iter_mut()
            .for_each(|h| h.size_weight = (h.size_weight / total_weight).max(3.0 / sz).min(10.0));
    }

    fn open_new(&mut self, state: &mut State, is_after: bool) {
        self.selected = if is_after {
            (self.selected + 1).min(self.vboxes.len())
        } else {
            self.selected.clamp(0, self.vboxes.len())
        };
        let size_weight = 1.0 / self.vboxes.len().max(1) as f32;
        self.vboxes.insert(
            self.selected,
            Child {
                child: T::from(Pane {
                    kind: match state.buffers.keys().next() {
                        Some(b) => PaneKind::Doc(b),
                        None => PaneKind::Empty,
                    },
                    last_area: Area::default(),
                    docs: HashMap::default(),
                    task: None,
                }),
                size_weight,
            },
        );
    }
}

impl<T: From<Pane>, const IS_VERTICAL: bool> From<Pane> for Panes<T, IS_VERTICAL> {
    fn from(pane: Pane) -> Self {
        Self {
            selected: 0,
            vboxes: vec![Child {
                child: pane.into(),
                size_weight: 1.0,
            }],
            last_area: Area::default(),
        }
    }
}

impl<T: PaneContainer, const IS_VERTICAL: bool> PaneContainer for Panes<T, IS_VERTICAL> {
    fn should_close(&mut self, _state: &mut State) -> bool {
        self.vboxes.is_empty()
    }

    fn close(self, state: &mut State) {
        for e in self.vboxes {
            e.child.close(state);
        }
    }

    fn last_area(&self) -> Area {
        self.last_area
    }

    fn selected_pane(&self) -> Option<&Pane> {
        self.vboxes.get(self.selected)?.child.selected_pane()
    }
}

impl<T: PaneContainer, const IS_VERTICAL: bool> Element<()> for Panes<T, IS_VERTICAL> {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp<()>, Event> {
        let res = match (
            IS_VERTICAL,
            event.to_action(|e| {
                e.to_pane_move()
                    .map(Action::PaneMove)
                    .or_else(|| e.to_pane_open().map(Action::PaneOpen))
                    .or_else(|| e.to_pane_close())
                    .or_else(|| e.to_pane_resize())
            }),
        ) {
            (false, Some(Action::PaneMove(Dir::Left)))
            | (true, Some(Action::PaneMove(Dir::Up)))
                if self.vboxes.len() > 1 =>
            {
                self.selected = (self.selected + self.vboxes.len() - 1) % self.vboxes.len();
                Ok(Resp::handled(None))
            }
            (false, Some(Action::PaneMove(Dir::Right)))
            | (true, Some(Action::PaneMove(Dir::Down)))
                if self.vboxes.len() > 1 =>
            {
                self.selected = (self.selected + 1) % self.vboxes.len();
                Ok(Resp::handled(None))
            }
            (false, Some(Action::PaneOpen(dir @ (Dir::Left | Dir::Right)))) => {
                self.open_new(state, matches!(dir, Dir::Right));
                Ok(Resp::handled(None))
            }
            (true, Some(Action::PaneOpen(dir @ (Dir::Up | Dir::Down)))) => {
                self.open_new(state, matches!(dir, Dir::Down));
                Ok(Resp::handled(None))
            }
            (false, Some(Action::PaneGrowH(by))) | (true, Some(Action::PaneGrowV(by))) => {
                if let Some(vbox) = self.vboxes.get_mut(self.selected) {
                    vbox.size_weight *= 1.2f32.powi(by);
                }
                Ok(Resp::handled(None))
            }
            // Pass anything else through to the active pane
            (_, action) => {
                let mut to_handle = self.selected;
                // Set selected vbox on mouse click
                if let Some(Action::Mouse(ref m_action, pos, _is_ctrl, _drag_id)) = action {
                    for (i, vbox) in self.vboxes.iter_mut().enumerate() {
                        if vbox.child.last_area().contains(pos).is_some() {
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
                    let resp = vbox.child.handle(state, event)?;
                    if resp.is_end() {
                        self.vboxes.remove(self.selected).child.close(state);
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

impl<T: PaneContainer, const IS_VERTICAL: bool> Visual for Panes<T, IS_VERTICAL> {
    fn render(&mut self, state: &mut State, frame: &mut Rect) {
        let n = self.vboxes.len();
        if n == 0 {
            return;
        }

        let total_weight = self.vboxes.iter().map(|h| h.size_weight).sum::<f32>();

        self.last_area = frame.area();

        // Remove any empty vboxes
        for (i, vbox) in self.vboxes.iter_mut().enumerate() {
            if vbox.child.should_close(state) {
                self.vboxes.remove(i);
                self.selected = self.selected.min(self.vboxes.len().saturating_sub(1));
                self.rescale();
                state.wakeup.notify_one();
                break;
            }
        }

        let dir_idx = if IS_VERTICAL { 1 } else { 0 };

        let mut x0 = 0;
        for (i, vbox) in self.vboxes.iter_mut().enumerate() {
            let x1 = if i == n - 1 {
                frame.size()[dir_idx]
            } else {
                x0 + ((vbox.size_weight * frame.size()[dir_idx] as f32 / total_weight)
                    .round()
                    .max(1.0) as usize)
                    .min(/*frame.size()[1] - (n - 1) * 3*/ !0)
            };

            // Draw pane contents
            frame
                .rect(
                    if IS_VERTICAL { [0, x0] } else { [x0, 0] },
                    if IS_VERTICAL {
                        [frame.size()[0], x1.saturating_sub(x0)]
                    } else {
                        [x1.saturating_sub(x0), frame.size()[1]]
                    },
                )
                .with_focus(self.selected == i)
                .with(|frame| vbox.child.render(state, frame));

            x0 = x1;
        }
    }
}

pub struct Tabs {
    selected: usize,
    pub(super) tabs: Vec<(Panes<Panes<Pane, true>, false>, Option<String>)>,
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
                    Some((
                        Panes::from(Pane {
                            kind: PaneKind::Doc(buffer_id),
                            task: task.map(Into::into),
                            docs: HashMap::default(),
                            last_area: Area::default(),
                        }),
                        None,
                    ))
                })
                .collect(),
            last_area: Default::default(),
            tab_view_timeout: None,
        }
    }

    fn reset_tab_timeout(&mut self) {
        self.tab_view_timeout = Some(Instant::now() + Duration::from_millis(800));
    }
}

impl Element<()> for Tabs {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp<()>, Event> {
        if let Some(timeout) = &mut self.tab_view_timeout {
            if &Instant::now() > timeout {
                state.needs_render = true;
                self.tab_view_timeout = None;
            }
        }

        // Update tab names
        if let Event::Tick = &event {
            for (tab, name) in &mut self.tabs {
                let new_name = tab.selected_pane().and_then(|pane| {
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
                });
                if new_name != *name {
                    *name = new_name;
                    state.needs_render = true;
                }
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
                self.tabs.insert(
                    new_idx,
                    (
                        Panes::from(Pane {
                            kind,
                            last_area: Area::default(),
                            docs: HashMap::default(),
                            task: None,
                        }),
                        None,
                    ),
                );
                self.reset_tab_timeout();
                self.selected = new_idx;
                Ok(Resp::handled(None))
            }
            // Pass anything else through to the active pane
            _ => {
                if let Some((tab, _)) = self.tabs.get_mut(self.selected) {
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
        for (i, (tab, _)) in self.tabs.iter_mut().enumerate() {
            if tab.vboxes.is_empty() {
                self.tabs.remove(i);
                self.selected = self.selected.min(self.tabs.len().saturating_sub(1));
                state.wakeup.notify_one();
                break;
            }
        }

        if let Some((tab, _)) = self.tabs.get_mut(self.selected) {
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
            for (i, (_, name)) in self.tabs.iter().enumerate() {
                let name = if let Some(name) = name {
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
