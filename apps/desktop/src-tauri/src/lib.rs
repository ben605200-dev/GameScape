// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg(windows)]
mod spike_s1;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet])
        .setup(|app| {
            #[cfg(windows)]
            {
                // Phase 0 verification gate. Never active in normal use.
                let spike_enabled = std::env::var("GAMESCAPE_SPIKE_S1")
                    .map(|value| value == "1")
                    .unwrap_or(false);
                if spike_enabled {
                    spike_s1::run(app.handle().clone());
                }
            }
            #[cfg(not(windows))]
            {
                let _ = app;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
