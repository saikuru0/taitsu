use eframe::egui::Color32;
use oshatori::Account;
use std::path::PathBuf;

pub fn color32(color: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(color[0], color[1], color[2], color[3])
}

pub fn accounts_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("taitsu");
    std::fs::create_dir_all(&path).ok();
    path.push("accounts.json");
    path
}

pub fn load_accounts() -> Vec<Account> {
    std::fs::read_to_string(accounts_path())
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

pub fn save_accounts(accounts: &[Account]) {
    if let Ok(content) = serde_json::to_string_pretty(accounts) {
        std::fs::write(accounts_path(), content).ok();
    }
}
