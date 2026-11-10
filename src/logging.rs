//! A log file installed before anything else starts, so a failed launch still
//! leaves a trace, even when started without a console for `eprintln!`. Paths
//! stay `PathBuf` end to end, never UTF-8 strings.
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

static FILE: OnceLock<Mutex<File>> = OnceLock::new();
static OPENED: OnceLock<PathBuf> = OnceLock::new();

/// `~/Library/Logs/Quill`; independent of where the program itself lives.
pub fn dir() -> PathBuf {
    if let Some(d) = std::env::var_os("QUILL_LOG_DIR") { return PathBuf::from(d) }
    let base = dirs::home_dir().map(|h| h.join("Library").join("Logs").join("Quill"));
    base.unwrap_or_else(|| std::env::temp_dir().join("Quill-logs"))
}

/// The file actually written (the temp fallback included).
pub fn path() -> PathBuf { OPENED.get().cloned().unwrap_or_else(|| dir().join("quill.log")) }

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, m: &log::Metadata) -> bool { m.level() <= log::Level::Warn || m.target().starts_with("quill") }
    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) { write(&format!("{} {}: {}", r.level(), r.target(), r.args())); }
    }
    fn flush(&self) {}
}

/// Opens the log (trimmed when large), captures `log` records (GPUI's
/// included) and panics. Falls back to the temp directory when the usual one
/// cannot be created; never fails the launch.
pub fn init() {
    let open = |dir: PathBuf| -> std::io::Result<File> {
        std::fs::create_dir_all(&dir)?;
        let p = dir.join("quill.log");
        let big = std::fs::metadata(&p).is_ok_and(|m| m.len() > 1 << 20);
        if big { let _ = std::fs::rename(&p, dir.join("quill.old.log")); }
        let f = OpenOptions::new().create(true).append(true).open(&p)?;
        let _ = OPENED.set(p);
        Ok(f)
    };
    if let Ok(f) = open(dir()).or_else(|_| open(std::env::temp_dir().join("Quill-logs"))) { let _ = FILE.set(Mutex::new(f)); }
    let _ = log::set_logger(&Logger).map(|()| log::set_max_level(log::LevelFilter::Info));
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        write(&format!("PANIC {info}\n{}", std::backtrace::Backtrace::force_capture()));
        default(info);
    }));
    let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
    write(&format!("start Quill {} ({exe})", env!("CARGO_PKG_VERSION")));
}

/// One line to the log file and stderr.
pub fn write(line: &str) {
    eprintln!("{line}");
    let Some(f) = FILE.get() else { return };
    let Ok(mut f) = f.lock() else { return };
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let _ = writeln!(f, "[{secs}] {line}");
    let _ = f.flush();
}

/// A launch that cannot continue: logged, exit code 1.
pub fn fatal(what: &str, err: &anyhow::Error) -> ! {
    write(&format!("ERROR {what}: {err:#} (log: {})", path().display()));
    std::process::exit(1)
}
