mod action;
mod ui;
mod terminal;
mod state;
mod theme;

use crate::{
    terminal::{Terminal, TerminalEvent, Color},
    action::{Event, Action, Dir},
    state::State,
    ui::{Element as _, Visual as _},
};
use clap::Parser;
use std::{
    path::PathBuf,
    time::Duration,
    io,
};

#[derive(Parser, Debug)]
struct Args {
    paths: Vec<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] io::Error),
}

fn main() -> Result<(), Error> {
    let args = Args::parse();
    println!("{args:?}");
    
    let mut state = State::try_from(args)?;
    let mut ui = ui::Root::new(&state);
    
    Terminal::with(move |term| {
        loop {
            // Render the state to the screen
            term.update(|fb| ui.render(&state, fb));
            
            // Wait for a while
            term.wait_at_least(Duration::from_millis(250));
            state.tick();
            
            while let Some(ev) = term.get_event() {
                // Resize events are special and need handling by the terminal
                if let TerminalEvent::Resize(cols, rows) = ev { term.set_size([cols, rows]); }
                
                // Have the UI handle events
                if ui
                    .handle(Event::from_raw(ev))
                    .map_or(false, |r| r.should_end())
                { return Ok(()); }
            }
        }
    })
}
