use std::{fs, path::Path};

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct NotifyState {
    pub unread: u32,
}

pub fn load(path: &Path) -> NotifyState {
    fs::read(path)
        .ok()
        .and_then(|contents| serde_json::from_slice(&contents).ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, state: &NotifyState) {
    let Ok(contents) = serde_json::to_vec(state) else {
        return;
    };

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, contents);
}
