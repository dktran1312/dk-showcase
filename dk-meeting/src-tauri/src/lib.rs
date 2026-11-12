//! DK Meeting — lõi Rust.

mod audio;
mod commands;
mod commands_ai;
mod error;
mod export;
mod live;
mod llm;
mod net;
mod state;
mod storage;
mod stt;
mod system;

use tauri::{Manager, RunEvent};

use state::AppState;

pub fn run() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,whisper_rs=warn,ureq=warn,rustls=warn"),
    )
    .init();
    whisper_rs::install_logging_hooks(); // log của whisper.cpp đi qua `log` thay vì in thẳng ra console

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
            commands_ai::get_system_check,
            commands_ai::list_whisper_models,
            commands_ai::download_whisper_model,
            commands_ai::cancel_download,
            commands_ai::delete_whisper_model,
            commands_ai::ollama_status,
            commands_ai::ollama_pull,
            commands_ai::quick_summary,
            commands_ai::generate_minutes,
            commands_ai::save_minutes,
            commands_ai::save_action_items,
            commands_ai::transcribe_meeting,
        ])
        .build(tauri::generate_context!())
        .expect("không khởi động được DK Meeting");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            // Đóng app khi đang ghi → đóng file WAV đàng hoàng.
            if let Some(state) = handle.try_state::<AppState>() {
                match commands::stop_current(&state) {
                    Ok((Some(m), pipeline)) => {
                        log::info!("Đã lưu cuộc họp '{}' trước khi thoát", m.title);
                        // Cho bộ nhận dạng tối đa 5 s để lưu nốt câu cuối.
                        if let Some(p) = pipeline {
                            let (tx, rx) = crossbeam_channel::bounded::<()>(1);
                            std::thread::spawn(move || {
                                p.stt.finish();
                                drop(p.translator);
                                let _ = tx.send(());
                            });
                            let _ = rx.recv_timeout(std::time::Duration::from_secs(5));
                        }
                    }
                    Ok((None, _)) => {}
                    Err(e) => log::error!("Lỗi khi dừng ghi lúc thoát: {e}"),
                }
            }
        }
    });
}
