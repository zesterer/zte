use super::*;
use crate::state::BufferId;

pub struct Root {
    panes: Panes,
    status: Status,
    tasks: Vec<Task>,
    drag_id_counter: usize,
}

pub enum Task {
    Prompt(Prompt),
    Show(Show),
    Confirm(Confirm),
    Searcher(Searcher),
}

impl Task {
    pub fn requested_height(&self) -> usize {
        match self {
            Self::Prompt(p) => p.requested_height(),
            Self::Show(s) => s.requested_height(),
            Self::Confirm(c) => c.requested_height(),
            Self::Searcher(s) => s.requested_height(),
        }
    }
}

impl Root {
    pub fn new(state: &mut State, args: &Args) -> Self {
        Self {
            panes: Panes::new(state, args),
            status: Status,
            tasks: Vec::new(),
            drag_id_counter: 0,
        }
    }
}

impl Element<()> for Root {
    fn handle(&mut self, state: &mut State, mut event: Event) -> Result<Resp<()>, Event> {
        // Perform any top-level conversion of raw events
        let mut event = event
            .to_action(|e| e.to_mouse(&mut self.drag_id_counter))
            .map(Event::Action)
            .unwrap_or(event);

        // Pass the event down through the list of tasks until we meet one that can handle it
        let mut task_idx = self.tasks.len();
        let event = loop {
            task_idx = match task_idx.checked_sub(1) {
                Some(task_idx) => task_idx,
                None => {
                    break match self.panes.handle(state, event) {
                        Ok(resp) => resp.event,
                        Err(event) => Some(event),
                    };
                }
            };

            let res = match &mut self.tasks[task_idx] {
                Task::Prompt(p) => p.handle(state, event),
                Task::Show(s) => s.handle(state, event),
                Task::Confirm(c) => c.handle(state, event),
                Task::Searcher(s) => s.handle(state, event),
            };

            match res {
                Ok(resp) => {
                    // If the task has requested that it should end, kill it and all of its children
                    if resp.is_end() {
                        self.tasks.truncate(task_idx);
                    }
                    event = if let Some(event) = resp.event {
                        event
                    } else {
                        break None;
                    };
                }
                Err(e) => event = e,
            }
        };

        // Handle 'top-level' actions
        if let Some(action) = event.as_ref().and_then(|e| {
            e.to_action(|e| {
                e.to_open_prompt()
                    .or_else(|| e.to_cancel())
                    .or_else(|| e.to_command_start())
            })
        }) {
            match action {
                Action::OpenPrompt => {
                    self.tasks.clear(); // Prompt overrides all
                    self.tasks.push(Task::Prompt(Prompt::new("")));
                }
                Action::OpenSearcher(path, needle) => {
                    self.tasks.clear(); // Overrides all
                    self.tasks.push(Task::Searcher(Searcher::new(path, needle)));
                }
                Action::CommandStart(cmd) => {
                    self.tasks.clear(); // Prompt overrides all
                    self.tasks
                        .push(Task::Prompt(Prompt::new(&format!("{cmd} "))));
                }
                Action::Cancel => {
                    let unsaved = state.buffers.values().filter(|b| b.unsaved).count();
                    if state.buffers.is_empty() {
                        return Ok(Resp::end(None));
                    } else {
                        self.tasks.push(Task::Confirm(Confirm {
                            label: Label(if unsaved == 0 {
                                format!("Are you sure you wish to quit? (y/n). You have multiple documents open!")
                            } else {
                                format!("Are you sure you wish to quit? (y/n). Note that {} files are unsaved!", unsaved)
                            }),
                            action: Action::Quit,
                        }));
                    }
                }
                Action::Confirm(q, action) => self.tasks.push(Task::Confirm(Confirm {
                    label: Label(q),
                    action: *action,
                })),
                Action::Show(title, text) => self.tasks.push(Task::Show(Show {
                    title,
                    label: Label(text),
                })),
                Action::Quit => return Ok(Resp::end(None)),
                action => {
                    return self
                        .panes
                        .handle(state, Event::Action(action))
                        .map(|r| r.into_can_end());
                }
            }
        } else if let Some(event) = event {
            return Err(event);
        }

        // Root element swallows all other events
        Ok(Resp::handled(None))
    }
}

impl Visual for Root {
    fn render(&mut self, state: &State, frame: &mut Rect) {
        frame.fill(' ');

        let task_has_focus = !self.tasks.is_empty();

        // Determine how much space the active task should use
        let task_h = self.tasks.last().map_or(0, |t| t.requested_height());

        // Render active task
        if let Some(task) = self.tasks.last_mut() {
            frame
                .rect(
                    [0, frame.size()[1].saturating_sub(task_h)],
                    [frame.size()[0], task_h],
                )
                .with_focus(task_has_focus)
                .with(|frame| match task {
                    Task::Prompt(p) => p.render(state, frame),
                    Task::Show(s) => s.render(state, frame),
                    Task::Confirm(c) => c.render(state, frame),
                    Task::Searcher(s) => s.render(state, frame),
                });
        }

        // Render panes
        frame
            .rect(
                [0, 0],
                [frame.size()[0], frame.size()[1].saturating_sub(task_h)],
            )
            .with_focus(!task_has_focus)
            .with(|frame| {
                self.panes.render(state, frame);
            });
    }
}
