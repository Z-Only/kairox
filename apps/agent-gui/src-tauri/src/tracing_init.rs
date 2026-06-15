#[cfg(not(test))]
use std::sync::OnceLock;

#[cfg(not(test))]
use chrono::Local;
use chrono::{DateTime, TimeZone};
#[cfg(not(test))]
use tracing_subscriber::fmt::format::Writer;
#[cfg(not(test))]
use tracing_subscriber::fmt::time::FormatTime;
#[cfg(not(test))]
use tracing_subscriber::EnvFilter;

#[cfg(not(test))]
const DEFAULT_LOG_FILTER: &str = "info";
const HUMAN_LOG_TIMESTAMP_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%.3f%:z";

#[cfg(not(test))]
#[derive(Debug, Clone, Copy)]
struct HumanLocalTimer;

#[cfg(not(test))]
impl FormatTime for HumanLocalTimer {
    fn format_time(&self, writer: &mut Writer<'_>) -> std::fmt::Result {
        write!(writer, "{}", format_human_log_timestamp(Local::now()))
    }
}

#[cfg(not(test))]
pub(crate) fn init() {
    static INIT: OnceLock<()> = OnceLock::new();

    INIT.get_or_init(|| {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(env_filter())
            .with_timer(HumanLocalTimer)
            .with_writer(std::io::stderr)
            .try_init();
    });
}

#[cfg(not(test))]
fn env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER))
}

pub(crate) fn format_human_log_timestamp<Tz>(timestamp: DateTime<Tz>) -> String
where
    Tz: TimeZone,
    Tz::Offset: std::fmt::Display,
{
    timestamp.format(HUMAN_LOG_TIMESTAMP_FORMAT).to_string()
}
