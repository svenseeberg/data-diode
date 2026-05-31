use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use log::LevelFilter;
use syslog::{Facility, Formatter3164, Logger, LoggerBackend};

pub struct DualLogger {
    syslog: Option<Mutex<Logger<LoggerBackend, Formatter3164>>>,
}

impl log::Log for DualLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        eprintln!("{} {}: {}", fmt_now(), record.level(), record.args());
        if let Some(s) = &self.syslog {
            if let Ok(mut guard) = s.lock() {
                let msg = format!("{}", record.args());
                let _ = match record.level() {
                    log::Level::Error => guard.err(msg),
                    log::Level::Warn => guard.warning(msg),
                    _ => guard.info(msg),
                };
            }
        }
    }

    fn flush(&self) {}
}

pub fn fmt_now() -> String {
    let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = dur.as_secs() as i64;
    let ms = dur.subsec_millis();
    let s = (secs.rem_euclid(60)) as u32;
    let m = ((secs / 60).rem_euclid(60)) as u32;
    let h = ((secs / 3600).rem_euclid(24)) as u32;
    let days = secs.div_euclid(86400);
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let mo = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let year = (if mo <= 2 { y + 1 } else { y }) as i32;
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02},{:03}", year, mo, d, h, m, s, ms)
}

pub fn init_logger() {
    let formatter = Formatter3164 {
        facility: Facility::LOG_USER,
        hostname: None,
        process: "DIODE".into(),
        pid: std::process::id(),
    };
    let syslog = syslog::unix(formatter).ok().map(Mutex::new);
    let logger = DualLogger { syslog };
    let _ = log::set_boxed_logger(Box::new(logger));
    log::set_max_level(LevelFilter::Info);
}
