use super::*;
use crate::state::BufferId;

pub struct Root {
    panes: Panes,
    status: Status,
    tasks: Vec<Task>,
}

pub enum Task {
    Prompt(Prompt),
    Show(Show),
    Confirm(Confirm),
    Switcher(Switcher),
}

impl Root {
    pub fn new(state: &mut State, buffers: &[BufferId]) -> Self {
        Self {
            panes: Panes::new(state, buffers),
            status: Status,
            tasks: Vec::new(),
        }
    }
}

impl Element<CanEnd> for Root {
    fn handle(&mut self, state: &mut State, mut event: Event) -> Result<Resp<CanEnd>, Event> {
        // Pass the event down through the list of tasks until we meet one that can handle it
        let mut task_idx = self.tasks.len();
        let action = loop {
            task_idx = match task_idx.checked_sub(1) {
                Some(task_idx) => task_idx,
                None => {
                    break match self.panes.handle(state, event) {
                        Ok(resp) => resp.action,
                        Err(event) => event.to_action(|e| e.to_open().or_else(|| e.to_cancel())),
                    };
                }
            };

            let res = match &mut self.tasks[task_idx] {
                Task::Prompt(p) => p.handle(state, event),
                Task::Show(s) => s.handle(state, event),
                Task::Confirm(c) => c.handle(state, event),
                Task::Switcher(s) => s.handle(state, event),
            };

            match res {
                Ok(resp) => {
                    // If the task has requested that it should end, kill it and all of its children
                    if resp.should_end() {
                        self.tasks.truncate(task_idx);
                    }
                    break resp.action;
                }
                Err(e) => event = e,
            }
        };

        // Handle 'top-level' actions
        if let Some(action) = action {
            match action {
                Action::OpenPrompt => {
                    self.tasks.clear(); // Prompt overrides all
                    self.tasks.push(Task::Prompt(Prompt {
                        input: Input {
                            preamble: "> ",
                            ..Input::default()
                        },
                    }));
                }
                Action::OpenSwitcher => {
                    self.tasks.clear(); // Prompt overrides all
                    self.tasks.push(Task::Switcher(Switcher {
                        selected: 0,
                        options: state.buffers.keys().collect(),
                    }));
                }
                Action::Cancel => self.tasks.push(Task::Confirm(Confirm {
                    label: Label("Are you sure you wish to quit? (y/n)".to_string()),
                    action: Action::Quit,
                })),
                Action::Show(text) => self.tasks.push(Task::Show(Show { label: Label(text) })),
                Action::Quit => return Ok(Resp::end(None)),
                action => {
                    return self
                        .panes
                        .handle(state, Event::Action(action))
                        .map(|r| r.into_can_end());
                }
            }
        }

        // Root element swallows all other events
        Ok(Resp::handled(None))
    }
}

impl Visual for Root {
    fn render(&self, state: &State, frame: &mut Rect) {
        frame.fill(' ');

        let task_has_focus = matches!(self.tasks.last(), Some(Task::Prompt(_)));

        // Display status bar
        frame
            .rect([0, frame.size()[1].saturating_sub(3)], [frame.size()[0], 3])
            .with_border(
                if task_has_focus {
                    &state.theme.focus_border
                } else {
                    &state.theme.border
                },
                Some("Prompt (press alt + enter)"),
            )
            .with(|frame| {
                if let Some(Task::Prompt(p)) = self.tasks.last() {
                    p.render(state, frame);
                }
            });

        frame
            .rect([0, 0], [frame.size()[0], frame.size()[1].saturating_sub(3)])
            .with_focus(!task_has_focus)
            .with(|frame| {
                self.panes.render(state, frame);
            });

        if let Some(task) = self.tasks.last() {
            match task {
                Task::Prompt(_) => {} // Prompt isn't rendered, it's always rendered above
                Task::Show(s) => s.render(state, frame),
                Task::Confirm(c) => c.render(state, frame),
                Task::Switcher(s) => s.render(state, frame),
            }
        }
    }
}
