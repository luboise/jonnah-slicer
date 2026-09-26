pub type LogState = std::sync::Arc<std::sync::Mutex<Option<(std::time::SystemTime, String)>>>;

/// # Errors
/// Will error if
/// - Unable to set the boxed logger
///   - ie, the logger has already been set
pub fn init() -> Result<LogState, crate::Error> {
    let state = LogState::default();
    let state_clone = std::sync::Arc::clone(&state);

    log::set_boxed_logger(Box::new(Logger { state }))?;
    log::set_max_level(log::LevelFilter::Info);

    Ok(state_clone)
}

pub struct Logger {
    state: LogState,
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Trace
    }

    fn log(&self, record: &log::Record<'_>) {
        #[expect(
            clippy::print_stdout,
            clippy::print_stderr,
            reason = "it's the logger, this is kinda the whole point"
        )]
        if self.enabled(record.metadata()) {
            if record.level() == log::Level::Warn || record.level() == log::Level::Error {
                eprintln!("{} - {}", record.level(), record.args());
            } else {
                println!("{} - {}", record.level(), record.args());
            }
        }

        let time = std::time::SystemTime::now();

        #[expect(clippy::unwrap_used, reason = "the logger being poisoned should panic")]
        let mut state = self.state.lock().unwrap();
        *state = Some((time, (record.args().to_string())));
    }

    fn flush(&self) {}
}
