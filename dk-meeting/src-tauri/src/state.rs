use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::Mutex;

use crate::audio::recorder::Recorder;
use crate::error::AppResult;
use crate::storage::Db;

pub struct Paths {
    pub data_dir: PathBuf,
    pub recordings_dir: PathBuf,
    pub db_path: PathBuf,
}

pub struct AppState {
    pub db: Arc<Db>,
    pub paths: Paths,
    /// Phiên ghi đang chạy (tối đa 1).
    pub recorder: Mutex<Option<Recorder>>,
}

impl AppState {
    pub fn init(data_dir: PathBuf) -> AppResult<Self> {
        let recordings_dir = data_dir.join("recordings");
        std::fs::create_dir_all(&recordings_dir)?;
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
                db_path,
            },
            recorder: Mutex::new(None),
        })
    }
}
