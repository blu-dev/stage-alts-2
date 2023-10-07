use locks::Mutex;
use log::Level;
use owo_colors::OwoColorize;
use std::{fs::File, io::Write};

pub struct StageAltsLogger;

impl StageAltsLogger {
    pub fn new() -> Self {
        Self
    }
}

fn colorize_level(level: Level) -> String {
    match level {
        Level::Error => "ERROR".bright_red().to_string(),
        Level::Warn => " WARN".yellow().to_string(),
        Level::Info => " INFO".green().to_string(),
        Level::Debug => "DEBUG".bright_green().to_string(),
        Level::Trace => "TRACE".white().to_string(),
    }
}

impl log::Log for StageAltsLogger {
    fn log(&self, record: &log::Record) {
        println!(
            "[{}:{} | {}] {}",
            record.file().unwrap(),
            record.line().unwrap(),
            colorize_level(record.level()),
            record.args()
        );
    }

    fn flush(&self) {}

    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }
}
