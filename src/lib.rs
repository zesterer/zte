pub mod action;
pub mod highlight;
pub mod lang;
pub mod state;
pub mod terminal;
pub mod theme;
pub mod ui;
pub mod util;

pub use crate::{
    action::{Action, Dir, Dist, Event, MouseAction},
    state::State,
    terminal::{Area, Attribute, Attributes, Color, Terminal, TerminalEvent},
    ui::{Element as _, Visual as _},
};
pub use clap::Parser;
pub use futures::{FutureExt, StreamExt};
pub use std::{io, path::PathBuf, sync::Arc, time::Duration};

#[derive(Parser, Debug)]
pub struct Args {
    paths: Vec<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("No such buffer")]
    NoSuchBuffer,
    #[error("File is not yet on disk")]
    FileNotOnDisk,
    #[error("Environment variable `SHELL` does not exist")]
    NoShellEnvVar,
    #[error("Shell process failed to spawn: {0}")]
    ShellProcessFailed(pty_process::Error),
}
