use super::*;
use strum::{EnumIter, Display, IntoEnumIterator};
use std::str::FromStr;

macro_rules! commands {
    ($({
        name: $name:ident,
        args: ($($args:ty),* $(,)?),
        aliases: [$($aliases:literal),* $(,)?],
        info: $info:literal,
        handler: $handler:path $(,)?
    }),* $(,)?) => {
        #[derive(EnumIter, Display)]
        pub enum Command { $($name),* }
        
        impl Command {
            pub fn aliases(&self) -> &'static [&'static str] {
                match self {
                    $(Self::$name => &[$($aliases,)*],)*
                }
            }
        }
    };
}

commands! {
    {
        name: Quit,
        args: (),
        aliases: ["quit", "q"],
        info: "Quit the editor",
        handler: exec_quit,
    },
    {
        name: Version,
        args: (),
        aliases: ["version"],
        info: "Show the editor version",
        handler: exec_version,
    },

    {
        name: Help,
        args: (Option<String>),
        aliases: ["help", "?"],
        info: "List commands or show help for a specific command",
        handler: exec_help,
    },
    {
        name: Open,
        args: (Option<PathBuf>),
        aliases: ["open", "o"],
        info: "Open or switch to the buffer of a file",
        handler: exec_help,
    },

}

impl FromStr for Command {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        for c in Self::iter() {
            if c.aliases().contains(&s) {
                return Ok(c);
            }
        }
        Err(())
    }
}

pub struct Prompt {
    pub input: Input,
}

impl Prompt {
    pub fn get_action(&self) -> Option<Action> {
        let input = self.input.get_text();
        let mut args = input.as_str().split_whitespace();
        
        if let Some(cmd) = args.next().and_then(|cmd| cmd.parse().ok()) {
            match cmd {
                Command::Quit => Some(Action::Quit),
                Command::Version => Some(Action::Show(format!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")))),
                Command::Help => {
                    use std::fmt::Write as _;
                    
                    let mut s = format!("Commands:\n");
                    for c in Command::iter() {
                        write!(s, "- {c}").unwrap();
                        let aliases = c.aliases();
                        match c.aliases() {
                            [] => {},
                            aliases => write!(s, " ({})", aliases.join(", ")).unwrap(),
                        }
                        writeln!(s, "").unwrap();
                    }
                    Some(Action::Show(s))                    
                },
                Command::Open => Some(Action::Show(format!("Open a file into a new buffer."))),
            }
        } else {
            None
        }
    }
}

impl Element<CanEnd> for Prompt {
    fn handle(&mut self, event: Event) -> Result<Resp<CanEnd>, Event> {
        match event.to_action(|e| if e.is_cancel() {
            Some(Action::Cancel)
        } else if e.is_go() {
            Some(Action::Go)
        } else if e.is_prompt() {
            Some(Action::OpenPrompt)
        } else {
            None
        }) {
            Some(Action::Cancel /*| Action::Prompt*/) => Ok(Resp::end(None)),
            Some(Action::Go) => if let Some(action) = self.get_action() {
                Ok(Resp::end(action))
            } else {
                Ok(Resp::end(Action::Show(format!("unknown command `{}`", self.input.get_text()))))
            },
            _ => self.input.handle(event).map(Resp::into_can_end),
        }
    }
}

impl Visual for Prompt {
    fn render(&self, state: &State, frame: &mut Rect) {    
        frame
            .with(|f| self.input.render(state, f));
    }
}

pub struct Show {
    pub label: Label,
}

impl Element<CanEnd> for Show {
    fn handle(&mut self, event: Event) -> Result<Resp<CanEnd>, Event> {
        match event.to_action(|e| if e.is_cancel() {
            Some(Action::Cancel)
        } else {
            None
        }) {
            Some(Action::Cancel) => Ok(Resp::end(None)),
            _ => Err(event),
        }
    }
}

impl Visual for Show {
    fn render(&self, state: &State, frame: &mut Rect) {
        let lines = self.label.lines().count();
        self.label.render(
            state,
            &mut frame.rect([0, frame.size()[1].saturating_sub(3 + lines)], [frame.size()[0], lines]),
        );
    }
}

pub struct Confirm {
    pub label: Label,
    pub action: Action,
}

impl Element<CanEnd> for Confirm {
    fn handle(&mut self, event: Event) -> Result<Resp<CanEnd>, Event> {
        match event.to_action(|e| if e.is_cancel() || e.to_char() == Some('n') {
            Some(Action::Cancel)
        } else if e.to_char() == Some('y') {
            Some(Action::Go)
        } else {
            None
        }) {
            Some(Action::Go) => Ok(Resp::end(Some(self.action.clone()))),
            Some(Action::Cancel) => Ok(Resp::end(None)),
            _ => Err(event),
        }
    }
}

impl Visual for Confirm {
    fn render(&self, state: &State, frame: &mut Rect) {
        let lines = self.label.lines().count();
        self.label.render(
            state,
            &mut frame.rect([0, frame.size()[1].saturating_sub(3 + lines)], [frame.size()[0], lines]),
        );
    }
}
