mod config;
mod display;
mod input;
mod event;

use std::{
    panic,
    io::Write,
};
use crate::{
    config::Config,
    display::Display,
    event::Event,
};

const LOG_FILENAME: &str = concat!(env!("CARGO_PKG_NAME"), ".log");

fn main() {
    let config = Config::load().unwrap_or_else(|err| {
        log::warn!("Failed to load config: {:?}", err);
        Config::default()
    });

    panic::set_hook(Box::new(move |info| {
        log::error!("{}", info);
    }));

	simple_logging::log_to_file(LOG_FILENAME, log::LevelFilter::Info)
        .expect("Failed to enable logging");

    let mut display = Display::new();

    let event_rx = input::begin_reading(display.input());
    loop {
        display.flush().unwrap();

        match event_rx.recv().unwrap() {
            Event::Quit => {
                log::info!("Quitting...");
                break;
            },
        }
    }
}
