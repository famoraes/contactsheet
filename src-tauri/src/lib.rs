mod commands;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::add_image,
            commands::remove_images,
            commands::render_preview,
            commands::render_thumbnail,
            commands::recalc,
            commands::auto_crop,
            commands::roll_analysis,
            commands::neutralize,
            commands::export_images,
            commands::read_roll,
            commands::write_roll,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ContactSheet");
}
