use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use log::{Level, LevelFilter, Log, Metadata, Record};

static VERBOSE: AtomicBool = AtomicBool::new(false);

pub struct Logger {
    level: LevelFilter,
    colors: bool,
}

struct Timestamp {
    year: u16,
    month: u16,
    day: u16,
    hours: u8,
    minutes: u8,
    seconds: u8,
}

impl Logger {
    pub fn new(level: LevelFilter) -> Self {
        Self {
            level,
            colors: true,
        }
    }

    pub fn with_colors(&mut self, colors: bool) -> &mut Self {
        self.colors = colors;
        self
    }

    pub fn init(self) {
        log::set_max_level(self.level);
        log::set_boxed_logger(Box::new(self)).expect("logger already set");
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let level = {
            let level = record.level().to_string();

            let red = "\x1b[31m";
            let yellow = "\x1b[33m";
            let cyan = "\x1b[36m";
            let purple = "\x1b[35m";
            let normal = "\x1b[m";

            if self.colors {
                let color = match record.level() {
                    Level::Error => red,
                    Level::Warn => yellow,
                    Level::Info => cyan,
                    Level::Debug => purple,
                    Level::Trace => normal,
                };

                format!("{}{:<5}{}", color, level, normal)
            } else {
                format!("{:<5}", level)
            }
        };

        let location = {
            let module = if !record.target().is_empty() {
                record.target()
            } else if let Some(path) = record.module_path() {
                path
            } else {
                "?"
            };

            module.split("::").last().unwrap_or("?")
        };

        let thread = match std::thread::current().name() {
            Some("main") | None => String::new(),
            Some(name) => format!("/{}", name),
        };

        let timestamp = Timestamp::new();

        println!("{} {} [{}{}] {}", timestamp, level, location, thread, record.args());
    }

    fn flush(&self) {}
}

impl Timestamp {
    fn new() -> Self {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current time before Unix epoch")
            .as_secs();

        Self::from_unix(seconds)
    }

    fn from_unix(total_seconds: u64) -> Self {
        let total_minutes = total_seconds / 60;
        let total_hours = total_minutes / 60;

        let seconds = (total_seconds % 60) as u8;
        let minutes = (total_minutes % 60) as u8;
        let hours = (total_hours % 24) as u8;

        let mut month_lengths = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        let mut day = total_hours / 24;
        let mut year = 1970;
        let mut month = 0;

        loop {
            if is_leap_year(year) {
                if day < 366 {
                    break;
                }
                day -= 366;
            } else {
                if day < 365 {
                    break;
                }
                day -= 365;
            }
            year += 1;
        }

        day += 1;

        if is_leap_year(year) {
            month_lengths[1] = 29;
        }

        while day > month_lengths[month as usize] {
            day -= month_lengths[month as usize];
            month += 1;
        }

        month += 1;

        #[allow(clippy::cast_possible_truncation)]
        let day = day as u16;

        Self {
            year,
            month,
            day,
            hours,
            minutes,
            seconds,
        }
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hours, self.minutes, self.seconds
        )
    }
}

fn is_leap_year(year: u16) -> bool {
    year % 400 == 0 || (year % 4 == 0 && year % 100 != 0)
}

pub fn verbose() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

pub fn set_verbosity(val: bool) {
    VERBOSE.store(val, Ordering::Relaxed);
}
