use super::*;

pub struct Root {
    panes: Panes,
    status: Status,
    tasks: Vec<Task>,
}

pub enum Task {
    Prompt(Prompt),
    Show(Show),
    Confirm(Confirm),
}

impl Root {
    pub fn new(state: &State) -> Self {
        Self {
            panes: Panes,
            status: Status,
            tasks: Vec::new(),
        }
    }
}

impl Element<CanEnd> for Root {
    fn handle(&mut self, mut event: Event) -> Result<Resp<CanEnd>, Event> {
        // Pass the event down through the list of tasks until we meet one that can handle it
        let mut task_idx = self.tasks.len();
        let action = loop {
            task_idx = match task_idx.checked_sub(1) {
                Some(task_idx) => task_idx,
                None => break match self.panes.handle(event) {
                    Ok(resp) => resp.action,
                    Err(event) => event.to_action(|e| if e.is_prompt() {
                        Some(Action::OpenPrompt)
                    } else if e.is_cancel() {
                        Some(Action::Cancel)
                    } else {
                        None
                    }),
                },
            };
            
            let res = match &mut self.tasks[task_idx] {
                Task::Prompt(p) => p.handle(event),
                Task::Show(s) => s.handle(event),
                Task::Confirm(c) => c.handle(event),
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
                        input: Input { preamble: "> ", ..Input::default() },
                    }));
                },
                Action::Cancel => self.tasks.push(Task::Confirm(Confirm {
                    label: Label("Are you sure you wish to quit? (y/n)".to_string()),
                    action: Action::Quit,
                })),
                Action::Show(text) => self.tasks.push(Task::Show(Show { label: Label(text) })),
                Action::Quit => return Ok(Resp::end(None)),
                action => todo!("Unhandled action {action:?}"),
            }
        }
        
        // Root element swallows all other events
        Ok(Resp::handled(None))
    }
}

impl Visual for Root {
    fn render(&self, state: &State, frame: &mut Rect) {
        frame
            .fill(' ');
        
        // Display status bar
        frame
            .rect([0, frame.size()[1].saturating_sub(3)], [frame.size()[0], 3])
            .fill(' ')
            .with_border(&state.theme.border)
            .with(|frame| {
                if let Some(Task::Prompt(p)) = self.tasks.last() {
                    p.render(state, frame);
                }
            });
        
        if let Some(task) = self.tasks.last() {
            match task {
                Task::Show(s) => s.render(state, frame),
                Task::Confirm(c) => c.render(state, frame),
                _ => {},
            }
        }
    }
}
