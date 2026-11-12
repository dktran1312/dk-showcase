//! Nối nhận dạng (stt) + dịch (llm) với DB và UI cho một cuộc họp.

use std::collections::HashMap;
use std::sync::Arc;

use crossbeam_channel::Sender;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::audio::Track;
use crate::llm::ollama::Ollama;
use crate::llm::translate::{TranslateJob, Translator};
use crate::state::AppState;
use crate::storage::models::Settings;
use crate::storage::Db;
use crate::stt::models;
use crate::stt::session::{SttConfig, SttSession};
use crate::stt::{NewSegment, SegmentSink, SttStatus};

pub const EVT_STT_STATUS: &str = "stt://status";
pub const EVT_SEGMENT: &str = "stt://segment";
pub const EVT_TRANSLATION: &str = "stt://translation";
pub const EVT_LLM_ERROR: &str = "llm://error";
pub const EVT_PROCESSED: &str = "meeting://processed";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusEvent {
    pub meeting_id: String,
    pub status: SttStatus,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationEvent {
    pub meeting_id: String,
    pub segment_id: i64,
    pub translation: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingEvent {
    pub meeting_id: String,
    pub message: String,
}

pub struct Pipeline {
    pub meeting_id: String,
    pub stt: SttSession,
    pub translator: Option<Translator>,
}

struct LiveSink {
    app: AppHandle,
    db: Arc<Db>,
    meeting_id: String,
    /// Chỉ cập nhật trạng thái "đang ghi" khi là phiên live (không phải tạo transcript từ file).
    live: bool,
    translate: Option<Sender<TranslateJob>>,
}

impl SegmentSink for LiveSink {
    fn on_segment(&self, seg: NewSegment) {
        match self.db.insert_segment(
            &self.meeting_id,
            seg.track.as_str(),
            seg.start_ms,
            seg.end_ms,
            &seg.lang,
            &seg.text,
        ) {
            Ok(saved) => {
                let _ = self.app.emit(EVT_SEGMENT, &saved);
                if let Some(tx) = self.translate.as_ref().filter(|_| seg.lang != "vi") {
                    let _ = tx.send(TranslateJob {
                        segment_id: saved.id,
                        text: seg.text,
                        lang: seg.lang,
                    });
                }
            }
            Err(e) => log::error!("Không lưu được câu transcript: {e}"),
        }
    }

    fn on_status(&self, status: SttStatus) {
        if self.live {
            if let Some(state) = self.app.try_state::<AppState>() {
                *state.stt_status.lock() = Some(status.clone());
            }
        }
        let _ = self.app.emit(
            EVT_STT_STATUS,
            StatusEvent {
                meeting_id: self.meeting_id.clone(),
                status,
            },
        );
    }
}

/// Gợi ý thuật ngữ cho whisper (giới hạn độ dài prompt ~ 220 token).
pub fn whisper_prompt(glossary: &[String]) -> String {
    let mut s = String::from("Cuộc họp tổ chức sự kiện. Thuật ngữ: ");
    for (i, t) in glossary.iter().enumerate() {
        if s.len() + t.len() > 600 {
            break;
        }
        if i > 0 {
            s.push_str(", ");
        }
        s.push_str(t);
    }
    s.push('.');
    s
}

fn make_translator(
    app: &AppHandle,
    db: &Arc<Db>,
    meeting_id: &str,
    settings: &Settings,
    glossary: &[String],
) -> Translator {
    let (app_r, db_r, mid_r) = (app.clone(), db.clone(), meeting_id.to_string());
    let on_result = Arc::new(move |segment_id: i64, translation: String| {
        if let Err(e) = db_r.set_translation(segment_id, &translation) {
            log::error!("Không lưu được bản dịch: {e}");
        }
        let _ = app_r.emit(
            EVT_TRANSLATION,
            TranslationEvent {
                meeting_id: mid_r.clone(),
                segment_id,
                translation,
            },
        );
    });
    let (app_e, mid_e) = (app.clone(), meeting_id.to_string());
    let on_error = Arc::new(move |message: String| {
        log::warn!("{message}");
        let _ = app_e.emit(
            EVT_LLM_ERROR,
            MeetingEvent {
                meeting_id: mid_e.clone(),
                message,
            },
        );
    });
    Translator::start(
        Ollama::new(&settings.ollama_url),
        settings.ollama_model.clone(),
        glossary.to_vec(),
        on_result,
        on_error,
    )
}

/// Dựng pipeline nhận dạng + dịch. Trả về None (kèm lý do qua event) nếu chưa có model.
pub fn start_pipeline(
    app: &AppHandle,
    state: &AppState,
    meeting_id: &str,
    language: &str,
    tracks: &[(Track, u32)],
    live: bool,
) -> (Option<Pipeline>, HashMap<Track, Sender<Vec<f32>>>) {
    let settings = state.db.get_settings().unwrap_or_default();
    let emit_unavailable = |reason: &str| {
        let status = SttStatus::Unavailable {
            reason: reason.into(),
        };
        if live {
            *state.stt_status.lock() = Some(status.clone());
        }
        let _ = app.emit(
            EVT_STT_STATUS,
            StatusEvent {
                meeting_id: meeting_id.into(),
                status,
            },
        );
    };
    if !settings.stt_enabled {
        emit_unavailable("Phụ đề trực tiếp đang tắt trong Cài đặt.");
        return (None, HashMap::new());
    }
    let Some(info) = models::find(&settings.whisper_model) else {
        emit_unavailable("Chưa chọn model nhận giọng nói. Vào Cài đặt để chọn.");
        return (None, HashMap::new());
    };
    let model_path = models::model_path(&state.paths.models_dir, info);
    if !model_path.exists() {
        emit_unavailable(&format!(
            "Chưa tải model nhận giọng nói ({}). Vào Cài đặt → Nhận giọng nói để tải. Âm thanh vẫn được ghi bình thường.",
            info.label
        ));
        return (None, HashMap::new());
    }

    let glossary = state.db.glossary_terms().unwrap_or_default();
    let translator = (settings.translate_enabled && language != "vi")
        .then(|| make_translator(app, &state.db, meeting_id, &settings, &glossary));
    let sink = Arc::new(LiveSink {
        app: app.clone(),
        db: state.db.clone(),
        meeting_id: meeting_id.to_string(),
        live,
        translate: translator.as_ref().map(|t| t.sender()),
    });
    let vad = models::vad_path(&state.paths.models_dir);
    let cfg = SttConfig {
        model_id: info.id.to_string(),
        model_path,
        vad_path: vad.exists().then_some(vad),
        language: language.to_string(),
        prompt: whisper_prompt(&glossary),
    };
    let (stt, taps) = SttSession::start(cfg, state.engine_cache.clone(), tracks, sink);
    (
        Some(Pipeline {
            meeting_id: meeting_id.to_string(),
            stt,
            translator,
        }),
        taps,
    )
}

/// Sau khi dừng ghi: chờ nhận dạng + dịch nốt ở nền rồi báo UI.
pub fn finish_in_background(app: AppHandle, pipeline: Pipeline) {
    let meeting_id = pipeline.meeting_id.clone();
    if let Some(state) = app.try_state::<AppState>() {
        state.processing.lock().insert(meeting_id.clone());
    }
    std::thread::spawn(move || {
        pipeline.stt.finish();
        if let Some(t) = pipeline.translator {
            t.finish();
        }
        if let Some(state) = app.try_state::<AppState>() {
            state.processing.lock().remove(&meeting_id);
        }
        let _ = app.emit(
            EVT_PROCESSED,
            MeetingEvent {
                meeting_id,
                message: String::new(),
            },
        );
    });
}
