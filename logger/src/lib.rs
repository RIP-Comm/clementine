#[cfg(feature = "logger")]
use chrono::Utc;
#[cfg(feature = "logger")]
use once_cell::sync::OnceCell;
#[cfg(feature = "logger")]
use std::{
    fs::File,
    io::{self, BufWriter, Write},
    sync::Mutex,
    time::Instant,
};

#[cfg(feature = "logger")]
static LOGGER: OnceCell<Logger> = OnceCell::new();

#[cfg(feature = "logger")]
struct LoggerImpl {
    pub sink: Box<dyn Write + Send>,
    pub start_instant: Instant,
}

#[cfg(feature = "logger")]
impl LoggerImpl {
    fn new(kind: LogKind) -> Self {
        let start_instant = Instant::now();
        match kind {
            LogKind::STDOUT => Self {
                sink: Box::new(io::stdout()),
                start_instant,
            },
            LogKind::FILE => {
                let now = Utc::now();
                let filename = format!("clementine-{}.log", now.timestamp());
                let path = std::env::temp_dir().join(filename);
                println!("Logging to file: {:?}", path);
                let file = File::create(path).unwrap();
                // Use BufWriter for much better performance (batches writes)
                Self {
                    sink: Box::new(BufWriter::new(file)),
                    start_instant,
                }
            }
        }
    }

    fn log<T>(&mut self, data: T)
    where
        T: std::fmt::Display,
    {
        let now = self.start_instant.elapsed();
        let seconds = now.as_secs();
        let hours = seconds / 3600;
        let minutes = (seconds / 60) % 60;
        let seconds = seconds % 60;
        let milliseconds = now.subsec_millis();

        writeln!(
            self.sink,
            "[{hours:02}:{minutes:02}:{seconds:02}.{milliseconds:03}] {data}"
        )
        .unwrap();
    }

    fn flush(&mut self) {
        self.sink.flush().ok();
    }
}

/// `LogKind` represents the kind of logging: `stdout` or `logfile`.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum LogKind {
    /// It logs to console, the default choice.
    STDOUT,

    /// It logs on a file in /tmp/clementine-<timestamp>.log
    FILE,
}

/// Logger
#[cfg(feature = "logger")]
struct Logger {
    pub inner_impl: Mutex<LoggerImpl>,
}

#[cfg(feature = "logger")]
impl Default for Logger {
    fn default() -> Self {
        Self {
            inner_impl: Mutex::new(LoggerImpl::new(LogKind::STDOUT)),
        }
    }
}

#[cfg(feature = "logger")]
impl Logger {
    fn new(kind: LogKind) -> Self {
        Self {
            inner_impl: Mutex::new(LoggerImpl::new(kind)),
        }
    }

    fn log<T>(&self, data: T)
    where
        T: std::fmt::Display,
    {
        if let Ok(ref mut inner) = self.inner_impl.lock() {
            inner.log(data);
        }
    }

    fn flush(&self) {
        if let Ok(ref mut inner) = self.inner_impl.lock() {
            inner.flush();
        }
    }
}

#[cfg(feature = "logger")]
pub fn init_logger(kind: LogKind) {
    LOGGER.set(Logger::new(kind)).ok();
}

pub fn log<T>(data: T)
where
    T: std::fmt::Display,
{
    let _ = data;
    #[cfg(feature = "logger")]
    if let Some(logger) = LOGGER.get() {
        logger.log(data)
    }
}

/// Flushes any buffered logs to disk.
/// This is useful to ensure logs are written before a potential crash or at important checkpoints.
/// For file logging, this forces the BufWriter to write its buffer to disk.
/// For stdout logging, this calls flush on stdout (though stdout is usually auto-flushed on newlines).
pub fn flush() {
    #[cfg(feature = "logger")]
    if let Some(logger) = LOGGER.get() {
        logger.flush()
    }
}

#[cfg(feature = "logger")]
#[cfg(test)]
mod tests {
    use std::fs;

    use crate::{LogKind, init_logger, log};

    #[test]
    fn logger_file() {
        init_logger(LogKind::FILE);
        log("ok".to_string());
        // Flush to ensure the buffered write is committed to disk
        crate::flush();
        let dir = std::env::temp_dir();
        let files = fs::read_dir(dir).unwrap();
        for f in files.flatten() {
            let p = f.path();
            if let Some(ext) = p.extension() {
                let s = p.to_str().unwrap();
                if ext == "log" && s.contains("clementine") {
                    print!("{p:?}");
                    let s = fs::read_to_string(p.clone()).unwrap();
                    fs::remove_file(p).unwrap();
                    assert_eq!(s, "[00:00:00.000] ok\n".to_string());
                }
            }
        }
    }
}
