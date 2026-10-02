use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static LOG_MUTEX: Mutex<()> = Mutex::new(());

const LOG_FILE_PATH: &str = "/home/yogeswar/Desktop/Projects/friday/friday-session.log";

fn format_now() -> String {
    if let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) {
        let secs = duration.as_secs();
        let millis = duration.subsec_millis();
        let s = secs % 86400;
        let hours = s / 3600;
        let mins = (s % 3600) / 60;
        let sec = s % 60;
        format!("{:02}:{:02}:{:02}.{:03}", hours, mins, sec, millis)
    } else {
        "00:00:00.000".to_string()
    }
}

pub fn init_session_log() {
    let _lock = LOG_MUTEX.lock();
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(LOG_FILE_PATH)
    {
        let ts = format_now();
        let header = format!(
            "================================================================================\n\
             FRIDAY SESSION DIAGNOSTIC LOG — STARTED AT {}\n\
             ================================================================================\n",
            ts
        );
        let _ = f.write_all(header.as_bytes());
        let _ = f.flush();
    }
    eprintln!(
        "[{}] Session log initialized at {}",
        format_now(),
        LOG_FILE_PATH
    );
}

pub fn session_log(msg: &str) {
    let ts = format_now();
    let line = format!("[{}] {}\n", ts, msg);

    // Print to console for real-time visibility
    eprint!("{}", line);

    let _lock = LOG_MUTEX.lock();
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_FILE_PATH)
    {
        let _ = f.write_all(line.as_bytes());
        let _ = f.flush();
    }
}
