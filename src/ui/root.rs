use super::*;

pub struct Root {
    tabs: Tabs,
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
            tabs: Tabs::new(state, args),
            tasks: Vec::new(),
            drag_id_counter: 0,
        }
    }

    pub fn should_close(&self) -> bool {
        self.tabs.tabs.is_empty()
    }
}

impl Element<()> for Root {
    fn handle(&mut self, state: &mut State, event: Event) -> Result<Resp<()>, Event> {
        // Perform any top-level conversion of raw events
        let mut event = event
            .to_action(|e| e.to_mouse(&mut self.drag_id_counter))
            .map(Event::Action)
            .unwrap_or(event);

        loop {
            // Pass the event down through the list of tasks until we meet one that can handle it
            let mut task_idx = self.tasks.len();
            event = loop {
                task_idx = match task_idx.checked_sub(1) {
                    Some(task_idx) => task_idx,
                    None => {
                        break match self.tabs.handle(state, event) {
                            Ok(resp) => match resp.event {
                                Some(new_event) => new_event,
                                None => return Ok(Resp::handled(None)),
                            },
                            Err(event) => event,
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
                            return Ok(Resp::handled(None));
                        };
                    }
                    Err(e) => event = e,
                }
            };

            // Handle 'top-level' actions
            event = if let Some(action) = event.to_action(|e| {
                e.to_open_prompt()
                    .or_else(|| e.to_cancel())
                    .or_else(|| e.to_command_start())
            }) {
                match action {
                    Action::OpenPrompt => {
                        self.tasks.clear(); // Prompt overrides all
                        self.tasks.push(Task::Prompt(Prompt::new("")));
                        break Ok(Resp::handled(None));
                    }
                    Action::OpenSearcher(path, needle) => {
                        self.tasks.clear(); // Overrides all
                        self.tasks.push(Task::Searcher(Searcher::new(path, needle)));
                        break Ok(Resp::handled(None));
                    }
                    Action::CommandStart(cmd) => {
                        self.tasks.clear(); // Prompt overrides all
                        self.tasks
                            .push(Task::Prompt(Prompt::new(&format!("{cmd} "))));
                        break Ok(Resp::handled(None));
                    }
                    Action::Cancel => {
                        let unsaved = state
                            .buffers
                            .values_mut()
                            .map(|b| b.has_changes())
                            .filter(|c| *c)
                            .count();
                        if unsaved > 0 {
                            self.tasks.push(Task::Confirm(Confirm {
                                label: Label(format!("Are you sure you wish to quit? (y/n). Note that {} file(s) have unsaved changes!", unsaved)),
                                action: Action::Quit,
                            }));
                            break Ok(Resp::handled(None));
                        } else if self.tabs.should_warn_close() {
                            self.tasks.push(Task::Confirm(Confirm {
                                label: Label(format!("Are you sure you wish to quit? (y/n). Some tasks are still active!")),
                                action: Action::Quit,
                            }));
                            break Ok(Resp::handled(None));
                        } else {
                            break Ok(Resp::end(None));
                        }
                    }
                    Action::Confirm(q, action) => {
                        self.tasks.push(Task::Confirm(Confirm {
                            label: Label(q),
                            action: *action,
                        }));
                        break Ok(Resp::handled(None));
                    }
                    Action::Show(title, text) => {
                        self.tasks.push(Task::Show(Show {
                            title,
                            label: Label(text),
                        }));
                        break Ok(Resp::handled(None));
                    }
                    Action::Quit => break Ok(Resp::end(None)),
                    action => match self
                        .tabs
                        .handle(state, Event::Action(action))
                        .map(|r| r.into_can_end::<()>())
                    {
                        Ok(resp) if resp.is_end() => return Ok(resp),
                        Ok(resp) => {
                            if let Some(new_event) = resp.event {
                                new_event
                            } else {
                                // Nothing to do
                                break Ok(Resp::handled(None));
                            }
                        }
                        Err(event) => break Err(event),
                    },
                }
            } else {
                break Err(event);
            }
        }
    }
}

impl Visual for Root {
    fn render(&mut self, state: &mut State, frame: &mut Rect) {
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

        // Render tabs
        frame
            .rect(
                [0, 0],
                [frame.size()[0], frame.size()[1].saturating_sub(task_h)],
            )
            .with_focus(!task_has_focus)
            .with(|frame| {
                self.tabs.render(state, frame);
            });
    }
}
