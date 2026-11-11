//! DK Meeting — lõi Rust.

mod audio;
mod commands;
mod error;
mod export;
mod llm;
mod state;
mod storage;
mod stt;

use tauri::{Manager, RunEvent};

use state::AppState;

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_local_data_dir()?;
            log::info!("Thư mục dữ liệu: {}", data_dir.display());
            app.manage(AppState::init(data_dir)?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_system_info,
            commands::list_meetings,
            commands::get_meeting,
            commands::update_meeting,
            commands::delete_meeting,
            commands::list_input_devices,
            commands::start_meeting,
            commands::get_recording_status,
            commands::pause_meeting,
            commands::resume_meeting,
            commands::stop_meeting,
            commands::add_bookmark,
            commands::update_bookmark,
            commands::delete_bookmark,
            commands::get_settings,
            commands::save_settings,
        ])
        .build(tauri::generate_context!())
        .expect("không khởi động được DK Meeting");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            // Đóng app khi đang ghi → đóng file WAV đàng hoàng.
            if let Some(state) = handle.try_state::<AppState>() {
                match commands::stop_current(&state) {
                    Ok(Some(m)) => log::info!("Đã lưu cuộc họp '{}' trước khi thoát", m.title),
                    Ok(None) => {}
                    Err(e) => log::error!("Lỗi khi dừng ghi lúc thoát: {e}"),
                }
            }
        }
    });
}
