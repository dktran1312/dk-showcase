//! Lớp command Tauri: mỏng, chỉ kiểm tra đầu vào và gọi xuống các module.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::audio::mic::{list_input_devices as list_mics, MicSource};
use crate::audio::recorder::{meeting_dir, LevelsEvent, Recorder};
use crate::audio::{AudioSource, ErrorFn, InputDevice, SourceKind};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::storage::models::{status, Bookmark, Meeting, MeetingDetail, MeetingPatch, Settings};

pub const EVT_LEVELS: &str = "recording://levels";
pub const EVT_ERROR: &str = "recording://error";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub version: String,
    pub os: String,
    pub arch: String,
    pub data_dir: String,
    pub recordings_dir: String,
    pub db_path: String,
}

#[tauri::command]
pub fn get_system_info(app: AppHandle, state: State<AppState>) -> SystemInfo {
    SystemInfo {
        version: app.package_info().version.to_string(),
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        data_dir: state.paths.data_dir.to_string_lossy().into_owned(),
        recordings_dir: state.paths.recordings_dir.to_string_lossy().into_owned(),
        db_path: state.paths.db_path.to_string_lossy().into_owned(),
    }
}

// ───────────── meetings ─────────────

#[tauri::command]
pub fn list_meetings(state: State<AppState>, query: Option<String>) -> AppResult<Vec<Meeting>> {
    state.db.list_meetings(query.as_deref())
}

#[tauri::command]
pub fn get_meeting(state: State<AppState>, id: String) -> AppResult<MeetingDetail> {
    state.db.meeting_detail(&id, &state.paths.recordings_dir)
}

#[tauri::command]
pub fn update_meeting(
    state: State<AppState>,
    id: String,
    patch: MeetingPatch,
) -> AppResult<Meeting> {
    state.db.update_meeting(&id, &patch)
}

#[tauri::command]
pub fn delete_meeting(state: State<AppState>, id: String) -> AppResult<()> {
    if state
        .recorder
        .lock()
        .as_ref()
        .is_some_and(|r| r.meeting_id() == id)
    {
        return Err(AppError::msg(
            "Cuộc họp đang ghi âm, hãy dừng trước khi xoá.",
        ));
    }
    state.db.delete_meeting(&id)?;
    let dir = meeting_dir(&state.paths.recordings_dir, &id);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

// ───────────── recording ─────────────

#[tauri::command]
pub fn list_input_devices() -> AppResult<Vec<InputDevice>> {
    list_mics()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartMeetingRequest {
    pub title: String,
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub client: String,
    #[serde(default = "default_lang")]
    pub language: String,
    pub mic_device: Option<String>,
}

fn default_lang() -> String {
    "auto".into()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStatus {
    pub meeting: Meeting,
    pub paused: bool,
    pub elapsed_ms: u64,
    pub sources: Vec<SourceKind>,
}

#[tauri::command]
pub async fn start_meeting(
    app: AppHandle,
    state: State<'_, AppState>,
    req: StartMeetingRequest,
) -> AppResult<Meeting> {
    let mut slot = state.recorder.lock();
    if slot.is_some() {
        return Err(AppError::msg("Đang có một cuộc họp được ghi âm."));
    }
    let title = req.title.trim();
    if title.is_empty() {
        return Err(AppError::msg("Hãy đặt tên cho cuộc họp."));
    }
    if !matches!(req.language.as_str(), "auto" | "vi" | "en") {
        return Err(AppError::msg("Ngôn ngữ không hợp lệ."));
    }

    // M1: chỉ có micro. M2 sẽ thêm WindowsProcessLoopback / WindowsSystemLoopback vào đây.
    let sources: Vec<Box<dyn AudioSource>> =
        vec![Box::new(MicSource::open(req.mic_device.clone())?)];
    let kinds: Vec<SourceKind> = sources.iter().map(|s| s.kind()).collect();

    let id = uuid::Uuid::new_v4().to_string();
    let meeting = state.db.create_meeting(
        &id,
        title,
        req.project.trim(),
        req.client.trim(),
        &req.language,
        &serde_json::to_string(&kinds)?,
    )?;

    let levels_app = app.clone();
    let on_levels = Arc::new(move |ev: LevelsEvent| {
        let _ = levels_app.emit(EVT_LEVELS, ev);
    });
    let err_app = app.clone();
    let on_error: ErrorFn = Arc::new(move |msg: String| {
        log::error!("{msg}");
        let _ = err_app.emit(EVT_ERROR, msg);
    });

    match Recorder::start(
        id.clone(),
        meeting_dir(&state.paths.recordings_dir, &id),
        sources,
        state.db.clone(),
        on_levels,
        on_error,
    ) {
        Ok(rec) => {
            *slot = Some(rec);
            Ok(meeting)
        }
        Err(e) => {
            let _ = state.db.delete_meeting(&id);
            let _ = std::fs::remove_dir_all(meeting_dir(&state.paths.recordings_dir, &id));
            Err(e)
        }
    }
}

#[tauri::command]
pub fn get_recording_status(state: State<AppState>) -> AppResult<Option<RecordingStatus>> {
    let slot = state.recorder.lock();
    let Some(rec) = slot.as_ref() else {
        return Ok(None);
    };
    Ok(Some(RecordingStatus {
        meeting: state.db.get_meeting(rec.meeting_id())?,
        paused: rec.is_paused(),
        elapsed_ms: rec.elapsed_ms(),
        sources: rec.sources().to_vec(),
    }))
}

#[tauri::command]
pub fn pause_meeting(state: State<AppState>) -> AppResult<()> {
    let slot = state.recorder.lock();
    let rec = slot
        .as_ref()
        .ok_or_else(|| AppError::msg("Không có cuộc họp nào đang ghi."))?;
    rec.pause();
    state.db.set_status(rec.meeting_id(), status::PAUSED)
}

#[tauri::command]
pub fn resume_meeting(state: State<AppState>) -> AppResult<()> {
    let slot = state.recorder.lock();
    let rec = slot
        .as_ref()
        .ok_or_else(|| AppError::msg("Không có cuộc họp nào đang ghi."))?;
    rec.resume();
    state.db.set_status(rec.meeting_id(), status::RECORDING)
}

#[tauri::command]
pub async fn stop_meeting(state: State<'_, AppState>) -> AppResult<Meeting> {
    stop_current(&state)?.ok_or_else(|| AppError::msg("Không có cuộc họp nào đang ghi."))
}

/// Dừng phiên ghi hiện tại (dùng cả khi đóng app).
pub fn stop_current(state: &AppState) -> AppResult<Option<Meeting>> {
    let Some(rec) = state.recorder.lock().take() else {
        return Ok(None);
    };
    let id = rec.meeting_id().to_string();
    let duration_ms = rec.stop();
    Ok(Some(state.db.finish_meeting(
        &id,
        status::DONE,
        duration_ms,
    )?))
}

#[tauri::command]
pub fn add_bookmark(state: State<AppState>, label: Option<String>) -> AppResult<Bookmark> {
    let slot = state.recorder.lock();
    let rec = slot
        .as_ref()
        .ok_or_else(|| AppError::msg("Không có cuộc họp nào đang ghi."))?;
    state.db.add_bookmark(
        rec.meeting_id(),
        rec.elapsed_ms(),
        label.as_deref().unwrap_or(""),
    )
}

#[tauri::command]
pub fn update_bookmark(state: State<AppState>, id: i64, label: String) -> AppResult<()> {
    state.db.update_bookmark(id, &label)
}

#[tauri::command]
pub fn delete_bookmark(state: State<AppState>, id: i64) -> AppResult<()> {
    state.db.delete_bookmark(id)
}

// ───────────── settings ─────────────

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> AppResult<Settings> {
    state.db.get_settings()
}

#[tauri::command]
pub fn save_settings(state: State<AppState>, settings: Settings) -> AppResult<Settings> {
    state.db.save_settings(&settings)?;
    Ok(settings)
}
