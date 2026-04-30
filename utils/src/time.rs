use std::time;

pub fn now_micros() -> u64 {
    time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}

pub fn now_string() -> String {
    now_micros().to_string()
}
