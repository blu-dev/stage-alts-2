use locks::Mutex;
use log::Level;
use owo_colors::OwoColorize;
use std::{fs::File, io::Write};

pub struct StageAltsLogger(locks::Mutex<File>);

impl StageAltsLogger {
    pub fn new() -> Self {
        std::fs::create_dir_all("sd:/ultimate/stage-alts").unwrap();
        let path = format!("sd:/ultimate/stage-alts/stage-alts.log",);
        Self(Mutex::new(File::create(path).unwrap()))
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

        #[cfg(feature = "file-log")]
        self.0
            .lock()
            .write_all(
                format!(
                    "[{}:{} | {}] {}\n",
                    record.file().unwrap(),
                    record.line().unwrap(),
                    record.level().as_str(),
                    record.args()
                )
                .as_bytes(),
            )
            .unwrap();
    }

    fn flush(&self) {
        self.0.lock().flush().unwrap();
    }

    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }
}
