use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use parking_lot::Mutex;

use crate::audio::recorder::Recorder;
use crate::error::AppResult;
use crate::live::Pipeline;
use crate::storage::Db;
use crate::stt::session::EngineCache;
use crate::stt::SttStatus;

pub struct Paths {
    pub data_dir: PathBuf,
    pub recordings_dir: PathBuf,
    pub models_dir: PathBuf,
    pub db_path: PathBuf,
}

pub struct AppState {
    pub db: Arc<Db>,
    pub paths: Paths,
    /// Phiên ghi đang chạy (tối đa 1).
    pub recorder: Mutex<Option<Recorder>>,
    /// Nhận dạng + dịch của phiên đang ghi.
    pub pipeline: Mutex<Option<Pipeline>>,
    /// Trạng thái whisper mới nhất của phiên đang ghi (để UI mở lại vẫn thấy).
    pub stt_status: Mutex<Option<SttStatus>>,
    /// Cuộc họp đang xử lý nốt (nhận dạng/dịch phần cuối, hoặc tạo transcript từ file).
    pub processing: Mutex<HashSet<String>>,
    /// Cờ huỷ của các lượt tải model đang chạy.
    pub downloads: Mutex<HashMap<String, Arc<AtomicBool>>>,
    pub engine_cache: Arc<EngineCache>,
}

impl AppState {
    pub fn init(data_dir: PathBuf) -> AppResult<Self> {
        let recordings_dir = data_dir.join("recordings");
        let models_dir = data_dir.join("models").join("whisper");
        std::fs::create_dir_all(&recordings_dir)?;
        std::fs::create_dir_all(&models_dir)?;
        let db_path = data_dir.join("dk-meeting.db");
        let db = Db::open(&db_path)?;
        let recovered = db.recover_interrupted(&recordings_dir)?;
        if recovered > 0 {
            log::warn!("Đã khôi phục {recovered} cuộc họp bị gián đoạn");
        }
        Ok(Self {
            db: Arc::new(db),
            paths: Paths {
                data_dir,
                recordings_dir,
                models_dir,
                db_path,
            },
            recorder: Mutex::new(None),
            pipeline: Mutex::new(None),
            stt_status: Mutex::new(None),
            processing: Mutex::new(HashSet::new()),
            downloads: Mutex::new(HashMap::new()),
            engine_cache: Arc::new(EngineCache::default()),
        })
    }
}
