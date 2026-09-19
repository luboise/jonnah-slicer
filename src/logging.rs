pub type LogState = std::sync::Arc<std::sync::Mutex<Option<(std::time::SystemTime, String)>>>;

pub fn init() -> Result<LogState, crate::Error> {
    let state = LogState::default();
    let state_clone = state.clone();

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
        if self.enabled(record.metadata()) {
            if record.level() == log::Level::Warn || record.level() == log::Level::Error {
                eprintln!("{} - {}", record.level(), record.args());
            } else {
                println!("{} - {}", record.level(), record.args());
            }
        }

        let time = std::time::SystemTime::now();

        *self.state.lock().unwrap() = Some((time, (record.args().to_string())));
    }

    fn flush(&self) {}
}
