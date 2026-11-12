//! Command cho phần AI: model whisper, Ollama, tóm tắt, biên bản, kiểm tra hệ thống.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use chrono::{DateTime, Local};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::audio::wav_writer::recover_duration_ms;
use crate::audio::Track;
use crate::error::{AppError, AppResult};
use crate::live::{start_pipeline, MeetingEvent, EVT_PROCESSED};
use crate::llm::minutes::{self, General, Line, Minutes, MinutesJob};
use crate::llm::ollama::{Ollama, OllamaModel};
use crate::state::AppState;
use crate::storage::models::{ActionItemRow, MinutesRecord, Segment, Settings};
use crate::stt::models::{self as wm, DownloadProgress, ModelState};
use crate::system::{self, Hardware};

pub const EVT_MODEL_PROGRESS: &str = "models://progress";
pub const EVT_OLLAMA_PULL: &str = "ollama://pull";
pub const EVT_MINUTES_PROGRESS: &str = "minutes://progress";
pub const EVT_TRANSCRIBE_PROGRESS: &str = "transcribe://progress";

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::msg(format!("Lỗi nội bộ: {e}")))?
}

// ───────────── kiểm tra hệ thống ─────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaStatus {
    pub url: String,
    pub running: bool,
    pub version: Option<String>,
    pub models: Vec<OllamaModel>,
    pub configured_model: String,
    pub has_model: bool,
    pub error: Option<String>,
}

fn ollama_status_inner(settings: &Settings) -> OllamaStatus {
    let o = Ollama::new(&settings.ollama_url);
    let mut st = OllamaStatus {
        url: settings.ollama_url.clone(),
        running: false,
        version: None,
        models: Vec::new(),
        configured_model: settings.ollama_model.clone(),
        has_model: false,
        error: None,
    };
    match o.version() {
        Ok(v) => {
            st.running = true;
            st.version = Some(v);
            match o.models() {
                Ok(m) => {
                    let want = &settings.ollama_model;
                    st.has_model = m
                        .iter()
                        .any(|x| &x.name == want || x.name == format!("{want}:latest"));
                    st.models = m;
                }
                Err(e) => st.error = Some(e.to_string()),
            }
        }
        Err(e) => st.error = Some(e.to_string()),
    }
    st
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemCheck {
    pub hardware: Hardware,
    /// Backend whisper.cpp được build kèm: "CUDA" | "CPU" ...
    pub stt_backend: String,
    pub whisper_models: Vec<ModelState>,
    pub vad_installed: bool,
    pub recommended_model: String,
    pub ollama: OllamaStatus,
    pub settings: Settings,
}

#[tauri::command]
pub async fn get_system_check(state: State<'_, AppState>) -> AppResult<SystemCheck> {
    let settings = state.db.get_settings()?;
    let dir = state.paths.models_dir.clone();
    blocking(move || {
        let hardware = system::hardware();
        let vram = hardware
            .gpus
            .iter()
            .filter_map(|g| g.vram_gb)
            .reduce(f32::max);
        let recommended = wm::recommend(vram, hardware.ram_gb).to_string();
        Ok(SystemCheck {
            stt_backend: crate::stt::backend_name().into(),
            whisper_models: wm::list(&dir),
            vad_installed: wm::vad_path(&dir).exists(),
            recommended_model: recommended,
            ollama: ollama_status_inner(&settings),
            hardware,
            settings,
        })
    })
    .await
}

// ───────────── model whisper ─────────────

#[tauri::command]
pub fn list_whisper_models(state: State<AppState>) -> Vec<ModelState> {
    wm::list(&state.paths.models_dir)
}

#[tauri::command]
pub async fn download_whisper_model(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut d = state.downloads.lock();
        if d.contains_key(&id) {
            return Err(AppError::msg("Model này đang được tải."));
        }
        d.insert(id.clone(), cancel.clone());
    }
    let dir = state.paths.models_dir.clone();
    let (app2, id2) = (app.clone(), id.clone());
    let result = blocking(move || {
        wm::download(&dir, &id2, &cancel, &|p| {
            let _ = app2.emit(EVT_MODEL_PROGRESS, p);
        })
    })
    .await;
    state.downloads.lock().remove(&id);
    let _ = app.emit(
        EVT_MODEL_PROGRESS,
        DownloadProgress {
            id,
            downloaded: 0,
            total: 0,
            done: true,
            error: result.as_ref().err().map(|e| e.to_string()),
        },
    );
    result
}

#[tauri::command]
pub fn cancel_download(state: State<AppState>, id: String) {
    if let Some(c) = state.downloads.lock().get(&id) {
        c.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
pub fn delete_whisper_model(state: State<AppState>, id: String) -> AppResult<()> {
    if state.pipeline.lock().is_some() || !state.processing.lock().is_empty() {
        return Err(AppError::msg(
            "Đang có cuộc họp dùng model, hãy thử lại sau.",
        ));
    }
    state.engine_cache.clear();
    wm::delete(&state.paths.models_dir, &id)
}

// ───────────── Ollama ─────────────

#[tauri::command]
pub async fn ollama_status(state: State<'_, AppState>) -> AppResult<OllamaStatus> {
    let settings = state.db.get_settings()?;
    blocking(move || Ok(ollama_status_inner(&settings))).await
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PullEvent {
    model: String,
    status: String,
    completed: u64,
    total: u64,
}

#[tauri::command]
pub async fn ollama_pull(
    app: AppHandle,
    state: State<'_, AppState>,
    model: String,
) -> AppResult<()> {
    let model = model.trim().to_string();
    if model.is_empty() {
        return Err(AppError::msg("Hãy nhập tên model, ví dụ qwen3:8b"));
    }
    let key = format!("ollama:{model}");
    let cancel = Arc::new(AtomicBool::new(false));
    state.downloads.lock().insert(key.clone(), cancel.clone());
    let o = Ollama::new(&state.db.get_settings()?.ollama_url);
    let app2 = app.clone();
    let m2 = model.clone();
    let res = blocking(move || {
        o.pull(&m2, &cancel, &|status, completed, total| {
            let _ = app2.emit(
                EVT_OLLAMA_PULL,
                PullEvent {
                    model: m2.clone(),
                    status: status.into(),
                    completed,
                    total,
                },
            );
        })
    })
    .await;
    state.downloads.lock().remove(&key);
    res
}

// ───────────── tóm tắt & biên bản ─────────────

fn speaker(track: &str) -> String {
    if track == Track::System.as_str() {
        "Phía online"
    } else {
        "Phía mình"
    }
    .to_string()
}

fn to_lines(segs: &[Segment]) -> Vec<Line> {
    segs.iter()
        .map(|s| Line {
            start_ms: s.start_ms.max(0) as u64,
            speaker: speaker(&s.track),
            text: s.text.clone(),
            translation: s.translation.clone(),
        })
        .collect()
}

#[tauri::command]
pub async fn quick_summary(state: State<'_, AppState>) -> AppResult<String> {
    let meeting_id = state
        .recorder
        .lock()
        .as_ref()
        .map(|r| r.meeting_id().to_string())
        .ok_or_else(|| AppError::msg("Không có cuộc họp nào đang ghi."))?;
    let db = state.db.clone();
    blocking(move || {
        let settings = db.get_settings()?;
        let glossary = db.glossary_terms()?;
        let lines = to_lines(&db.list_segments(&meeting_id)?);
        minutes::quick_summary(
            &Ollama::new(&settings.ollama_url),
            &settings.ollama_model,
            &glossary,
            &lines,
        )
    })
    .await
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MinutesProgress {
    meeting_id: String,
    step: usize,
    total: usize,
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MinutesResult {
    pub minutes: Option<MinutesRecord>,
    pub action_items: Vec<ActionItemRow>,
}

fn fmt_duration(ms: i64) -> String {
    let m = ms.max(0) / 60_000;
    if m >= 60 {
        format!("{} giờ {} phút", m / 60, m % 60)
    } else {
        format!("{} phút", m.max(1))
    }
}

#[tauri::command]
pub async fn generate_minutes(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> AppResult<MinutesResult> {
    if state.processing.lock().contains(&id) {
        return Err(AppError::msg(
            "Transcript đang được xử lý nốt, đợi vài giây rồi thử lại.",
        ));
    }
    let db = state.db.clone();
    blocking(move || {
        let settings = db.get_settings()?;
        let meeting = db.get_meeting(&id)?;
        let lines = to_lines(&db.list_segments(&id)?);
        let glossary = db.glossary_terms()?;
        let started: Option<DateTime<Local>> = DateTime::parse_from_rfc3339(&meeting.started_at)
            .ok()
            .map(|d| d.with_timezone(&Local));
        let general = General {
            title: meeting.title.clone(),
            date: started
                .map(|d| d.format("%d/%m/%Y %H:%M").to_string())
                .unwrap_or_default(),
            duration: fmt_duration(meeting.duration_ms),
            participants: Vec::new(),
            client: [meeting.client.as_str(), meeting.project.as_str()]
                .iter()
                .filter(|s| !s.is_empty())
                .copied()
                .collect::<Vec<_>>()
                .join(" — "),
        };
        let ollama = Ollama::new(&settings.ollama_url);
        let job = MinutesJob {
            ollama: &ollama,
            model: &settings.ollama_model,
            glossary: &glossary,
            lines: &lines,
            general,
        };
        let m: Minutes = minutes::generate(job, &|step, total, msg| {
            let _ = app.emit(
                EVT_MINUTES_PROGRESS,
                MinutesProgress {
                    meeting_id: id.clone(),
                    step,
                    total,
                    message: msg.into(),
                },
            );
        })?;
        let items: Vec<ActionItemRow> = m
            .action_items
            .iter()
            .filter(|a| !a.task.trim().is_empty())
            .map(|a| ActionItemRow {
                id: 0,
                task: a.task.clone(),
                owner: a.owner.clone(),
                deadline: a.deadline.clone(),
                status: "todo".into(),
                source_ms: minutes::parse_time(&a.source_time).map(|v| v as i64),
            })
            .collect();
        db.save_minutes(
            &id,
            &serde_json::to_value(&m)?,
            &settings.ollama_model,
            false,
        )?;
        let action_items = db.replace_action_items(&id, &items)?;
        Ok(MinutesResult {
            minutes: db.get_minutes(&id)?,
            action_items,
        })
    })
    .await
}

#[tauri::command]
pub fn save_minutes(
    state: State<AppState>,
    id: String,
    data: serde_json::Value,
) -> AppResult<Option<MinutesRecord>> {
    state.db.save_minutes(&id, &data, "", true)?;
    state.db.get_minutes(&id)
}

#[tauri::command]
pub fn save_action_items(
    state: State<AppState>,
    id: String,
    items: Vec<ActionItemRow>,
) -> AppResult<Vec<ActionItemRow>> {
    state.db.replace_action_items(&id, &items)
}

// ───────────── tạo transcript từ file ghi âm ─────────────

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TranscribeProgress {
    meeting_id: String,
    done_ms: u64,
    total_ms: u64,
}

/// Nhận dạng lại toàn bộ từ các file WAV (cho cuộc họp ghi khi chưa có model, hoặc muốn làm lại).
#[tauri::command]
pub async fn transcribe_meeting(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    if state
        .recorder
        .lock()
        .as_ref()
        .is_some_and(|r| r.meeting_id() == id)
    {
        return Err(AppError::msg("Cuộc họp đang ghi âm."));
    }
    if !state.processing.lock().insert(id.clone()) {
        return Err(AppError::msg("Cuộc họp đang được xử lý."));
    }
    let detail = match state.db.meeting_detail(&id, &state.paths.recordings_dir) {
        Ok(d) => d,
        Err(e) => {
            state.processing.lock().remove(&id);
            return Err(e);
        }
    };
    // chunks theo track, đúng thứ tự
    let mut by_track: Vec<(Track, Vec<(String, u32)>)> = Vec::new();
    for c in &detail.chunks {
        let track = if c.track == "system" {
            Track::System
        } else {
            Track::Mic
        };
        match by_track.iter_mut().find(|(t, _)| *t == track) {
            Some((_, v)) => v.push((c.path.clone(), c.sample_rate as u32)),
            None => by_track.push((track, vec![(c.path.clone(), c.sample_rate as u32)])),
        }
    }
    if by_track.is_empty() {
        state.processing.lock().remove(&id);
        return Err(AppError::msg("Cuộc họp không có file âm thanh."));
    }
    let tracks: Vec<(Track, u32)> = by_track.iter().map(|(t, v)| (*t, v[0].1)).collect();
    let (pipeline, mut taps) =
        start_pipeline(&app, &state, &id, &detail.meeting.language, &tracks, false);
    let Some(pipeline) = pipeline else {
        state.processing.lock().remove(&id);
        return Err(AppError::msg(
            "Chưa có model nhận giọng nói. Vào Cài đặt → Nhận giọng nói để tải.",
        ));
    };
    state.db.clear_segments(&id)?;

    let total_ms: u64 = detail
        .chunks
        .iter()
        .map(|c| c.duration_ms.max(0) as u64)
        .sum();
    let app2 = app.clone();
    let id2 = id.clone();
    let res = blocking(move || {
        let mut done_ms = 0u64;
        for (track, files) in by_track {
            let Some(tap) = taps.remove(&track) else {
                continue;
            };
            for (path, rate) in files {
                let mut reader = match hound::WavReader::open(&path) {
                    Ok(r) => r,
                    Err(_) => {
                        // file của cuộc họp bị gián đoạn có thể hỏng header → thử sửa rồi mở lại
                        recover_duration_ms(std::path::Path::new(&path));
                        hound::WavReader::open(&path)?
                    }
                };
                let mut buf = Vec::with_capacity(rate as usize);
                for s in reader.samples::<i16>() {
                    let Ok(s) = s else { break };
                    buf.push(s as f32 / 32768.0);
                    if buf.len() == rate as usize {
                        let _ = tap.send(std::mem::take(&mut buf));
                        done_ms += 1000;
                        if done_ms % 10_000 == 0 {
                            let _ = app2.emit(
                                EVT_TRANSCRIBE_PROGRESS,
                                TranscribeProgress {
                                    meeting_id: id2.clone(),
                                    done_ms,
                                    total_ms,
                                },
                            );
                        }
                    }
                }
                if !buf.is_empty() {
                    let _ = tap.send(buf);
                }
            }
        }
        drop(taps);
        pipeline.stt.finish();
        if let Some(t) = pipeline.translator {
            t.finish();
        }
        Ok(())
    })
    .await;
    if let Some(st) = app.try_state::<AppState>() {
        st.processing.lock().remove(&id);
    }
    let _ = app.emit(
        EVT_PROCESSED,
        MeetingEvent {
            meeting_id: id,
            message: String::new(),
        },
    );
    res
}
