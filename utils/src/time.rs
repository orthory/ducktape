use std::time;

pub fn now_u128() -> u128 {
    time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)
        .unwrap()
        .as_micros()
}

pub fn now_string() -> String {
    now_u128().to_string()
}
