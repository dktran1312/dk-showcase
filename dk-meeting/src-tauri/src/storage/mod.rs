//! Lưu trữ local: SQLite + thư mục file âm thanh. Không gửi dữ liệu ra ngoài máy.

pub mod migrations;
pub mod models;

use std::path::Path;

use chrono::Utc;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::audio::wav_writer::recover_duration_ms;
use crate::audio::Track;
use crate::error::{AppError, AppResult};
use models::{status, AudioChunk, Bookmark, Meeting, MeetingDetail, MeetingPatch, Settings};

pub struct Db {
    conn: Mutex<Connection>,
}

pub fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

const MEETING_COLS: &str =
    "m.id, m.title, m.project, m.client, m.language, m.status, m.created_at, \
     m.started_at, m.ended_at, m.duration_ms, m.notes, \
     (SELECT COUNT(*) FROM bookmarks b WHERE b.meeting_id = m.id)";

fn meeting_from_row(r: &Row) -> rusqlite::Result<Meeting> {
    Ok(Meeting {
        id: r.get(0)?,
        title: r.get(1)?,
        project: r.get(2)?,
        client: r.get(3)?,
        language: r.get(4)?,
        status: r.get(5)?,
        created_at: r.get(6)?,
        started_at: r.get(7)?,
        ended_at: r.get(8)?,
        duration_ms: r.get(9)?,
        notes: r.get(10)?,
        bookmark_count: r.get(11)?,
    })
}

/// Biến chuỗi người dùng gõ thành truy vấn FTS5 an toàn: mỗi từ là một cụm "..." và tìm theo tiền tố.
fn fts_query(q: &str) -> Option<String> {
    let terms: Vec<String> = q
        .split_whitespace()
        .map(|t| t.replace('"', ""))
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{t}\"*"))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

impl Db {
    pub fn open(path: &Path) -> AppResult<Self> {
        let mut conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrations::run(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    #[cfg(test)]
    pub fn open_in_memory() -> AppResult<Self> {
        let mut conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrations::run(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    // ───────────── meetings ─────────────

    pub fn create_meeting(
        &self,
        id: &str,
        title: &str,
        project: &str,
        client: &str,
        language: &str,
        sources_json: &str,
    ) -> AppResult<Meeting> {
        let now = now_iso();
        self.conn.lock().execute(
            "INSERT INTO meetings(id, title, project, client, language, sources, status, created_at, started_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![id, title, project, client, language, sources_json, status::RECORDING, now],
        )?;
        self.get_meeting(id)
    }

    pub fn get_meeting(&self, id: &str) -> AppResult<Meeting> {
        self.conn
            .lock()
            .query_row(
                &format!("SELECT {MEETING_COLS} FROM meetings m WHERE m.id = ?1"),
                [id],
                meeting_from_row,
            )
            .optional()?
            .ok_or_else(|| AppError::msg("Không tìm thấy cuộc họp"))
    }

    pub fn set_status(&self, id: &str, status: &str) -> AppResult<()> {
        self.conn.lock().execute(
            "UPDATE meetings SET status = ?2 WHERE id = ?1",
            params![id, status],
        )?;
        Ok(())
    }

    pub fn finish_meeting(&self, id: &str, status: &str, duration_ms: u64) -> AppResult<Meeting> {
        self.conn.lock().execute(
            "UPDATE meetings SET status = ?2, ended_at = ?3, duration_ms = ?4 WHERE id = ?1",
            params![id, status, now_iso(), duration_ms as i64],
        )?;
        self.get_meeting(id)
    }

    pub fn update_meeting(&self, id: &str, patch: &MeetingPatch) -> AppResult<Meeting> {
        {
            let conn = self.conn.lock();
            if let Some(t) = patch
                .title
                .as_deref()
                .map(str::trim)
                .filter(|t| !t.is_empty())
            {
                conn.execute(
                    "UPDATE meetings SET title = ?2 WHERE id = ?1",
                    params![id, t],
                )?;
            }
            if let Some(p) = &patch.project {
                conn.execute(
                    "UPDATE meetings SET project = ?2 WHERE id = ?1",
                    params![id, p.trim()],
                )?;
            }
            if let Some(c) = &patch.client {
                conn.execute(
                    "UPDATE meetings SET client = ?2 WHERE id = ?1",
                    params![id, c.trim()],
                )?;
            }
            if let Some(n) = &patch.notes {
                conn.execute(
                    "UPDATE meetings SET notes = ?2 WHERE id = ?1",
                    params![id, n],
                )?;
            }
        }
        self.get_meeting(id)
    }

    pub fn delete_meeting(&self, id: &str) -> AppResult<()> {
        self.conn
            .lock()
            .execute("DELETE FROM meetings WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Danh sách cuộc họp, mới nhất trước. `query` tìm theo tên / dự án / khách hàng
    /// và toàn văn transcript (FTS5, không phân biệt dấu).
    pub fn list_meetings(&self, query: Option<&str>) -> AppResult<Vec<Meeting>> {
        let conn = self.conn.lock();
        let q = query.map(str::trim).filter(|q| !q.is_empty());
        let rows = match q {
            None => {
                let mut stmt = conn.prepare(&format!(
                    "SELECT {MEETING_COLS} FROM meetings m ORDER BY m.started_at DESC"
                ))?;
                let rows = stmt
                    .query_map([], meeting_from_row)?
                    .collect::<Result<Vec<_>, _>>()?;
                rows
            }
            Some(q) => {
                let like = format!("%{q}%");
                let fts = fts_query(q).unwrap_or_else(|| "\"\"".into());
                let mut stmt = conn.prepare(&format!(
                    "SELECT {MEETING_COLS} FROM meetings m
                     WHERE m.title LIKE ?1 OR m.project LIKE ?1 OR m.client LIKE ?1
                        OR m.id IN (SELECT s.meeting_id FROM segments s
                                    WHERE s.id IN (SELECT rowid FROM segments_fts WHERE segments_fts MATCH ?2))
                     ORDER BY m.started_at DESC"
                ))?;
                let rows = stmt
                    .query_map(params![like, fts], meeting_from_row)?
                    .collect::<Result<Vec<_>, _>>()?;
                rows
            }
        };
        Ok(rows)
    }

    pub fn meeting_detail(&self, id: &str, recordings_dir: &Path) -> AppResult<MeetingDetail> {
        let meeting = self.get_meeting(id)?;
        let folder = recordings_dir.join(id);
        let conn = self.conn.lock();
        let chunks = conn
            .prepare(
                "SELECT track, idx, file_name, sample_rate, start_ms, duration_ms
                 FROM audio_chunks WHERE meeting_id = ?1 ORDER BY track, idx",
            )?
            .query_map([id], |r| {
                let file_name: String = r.get(2)?;
                Ok(AudioChunk {
                    track: r.get(0)?,
                    idx: r.get(1)?,
                    path: folder.join(&file_name).to_string_lossy().into_owned(),
                    file_name,
                    sample_rate: r.get(3)?,
                    start_ms: r.get(4)?,
                    duration_ms: r.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let bookmarks = conn
            .prepare("SELECT id, meeting_id, at_ms, label FROM bookmarks WHERE meeting_id = ?1 ORDER BY at_ms")?
            .query_map([id], bookmark_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(MeetingDetail {
            meeting,
            chunks,
            bookmarks,
            folder: folder.to_string_lossy().into_owned(),
        })
    }

    // ───────────── audio chunks ─────────────

    pub fn insert_chunk(
        &self,
        meeting_id: &str,
        track: Track,
        idx: u32,
        file_name: &str,
        sample_rate: u32,
        start_ms: u64,
    ) -> AppResult<()> {
        self.conn.lock().execute(
            "INSERT OR REPLACE INTO audio_chunks(meeting_id, track, idx, file_name, sample_rate, start_ms, duration_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
            params![meeting_id, track.as_str(), idx, file_name, sample_rate, start_ms as i64],
        )?;
        Ok(())
    }

    pub fn set_chunk_duration(
        &self,
        meeting_id: &str,
        track: Track,
        idx: u32,
        duration_ms: u64,
    ) -> AppResult<()> {
        self.conn.lock().execute(
            "UPDATE audio_chunks SET duration_ms = ?4 WHERE meeting_id = ?1 AND track = ?2 AND idx = ?3",
            params![meeting_id, track.as_str(), idx, duration_ms as i64],
        )?;
        Ok(())
    }

    // ───────────── bookmarks ─────────────

    pub fn add_bookmark(&self, meeting_id: &str, at_ms: u64, label: &str) -> AppResult<Bookmark> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO bookmarks(meeting_id, at_ms, label, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![meeting_id, at_ms as i64, label.trim(), now_iso()],
        )?;
        let id = conn.last_insert_rowid();
        Ok(conn.query_row(
            "SELECT id, meeting_id, at_ms, label FROM bookmarks WHERE id = ?1",
            [id],
            bookmark_from_row,
        )?)
    }

    pub fn update_bookmark(&self, id: i64, label: &str) -> AppResult<()> {
        self.conn.lock().execute(
            "UPDATE bookmarks SET label = ?2 WHERE id = ?1",
            params![id, label.trim()],
        )?;
        Ok(())
    }

    pub fn delete_bookmark(&self, id: i64) -> AppResult<()> {
        self.conn
            .lock()
            .execute("DELETE FROM bookmarks WHERE id = ?1", [id])?;
        Ok(())
    }

    // ───────────── settings ─────────────

    pub fn get_settings(&self) -> AppResult<Settings> {
        let raw: Option<String> = self
            .conn
            .lock()
            .query_row("SELECT value FROM settings WHERE key = 'app'", [], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(raw
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default())
    }

    pub fn save_settings(&self, s: &Settings) -> AppResult<()> {
        self.conn.lock().execute(
            "INSERT INTO settings(key, value) VALUES ('app', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [serde_json::to_string(s)?],
        )?;
        Ok(())
    }

    // ───────────── crash recovery ─────────────

    /// Gọi lúc mở app: cuộc họp nào còn ở trạng thái "đang ghi" nghĩa là lần trước app bị tắt
    /// đột ngột. Đánh dấu "interrupted", tính lại thời lượng từ các file WAV đã ghi.
    pub fn recover_interrupted(&self, recordings_dir: &Path) -> AppResult<usize> {
        let ids: Vec<String> = {
            let conn = self.conn.lock();
            let mut stmt = conn.prepare("SELECT id FROM meetings WHERE status IN (?1, ?2)")?;
            let ids = stmt
                .query_map(params![status::RECORDING, status::PAUSED], |r| r.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            ids
        };
        for id in &ids {
            let chunks: Vec<(String, i64, String, i64)> = {
                let conn = self.conn.lock();
                let mut stmt =
                    conn.prepare("SELECT track, idx, file_name, start_ms FROM audio_chunks WHERE meeting_id = ?1")?;
                let rows = stmt
                    .query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                    .collect::<Result<Vec<_>, _>>()?;
                rows
            };
            let mut total = 0i64;
            for (track, idx, file_name, start_ms) in chunks {
                let ms = recover_duration_ms(&recordings_dir.join(id).join(&file_name)).unwrap_or(0)
                    as i64;
                self.conn.lock().execute(
                    "UPDATE audio_chunks SET duration_ms = ?4 WHERE meeting_id = ?1 AND track = ?2 AND idx = ?3",
                    params![id, track, idx, ms],
                )?;
                total = total.max(start_ms + ms);
            }
            self.conn.lock().execute(
                "UPDATE meetings SET status = ?2, duration_ms = ?3, ended_at = COALESCE(ended_at, ?4) WHERE id = ?1",
                params![id, status::INTERRUPTED, total, now_iso()],
            )?;
            log::warn!("Khôi phục cuộc họp bị gián đoạn {id}: {total} ms");
        }
        Ok(ids.len())
    }
}

fn bookmark_from_row(r: &Row) -> rusqlite::Result<Bookmark> {
    Ok(Bookmark {
        id: r.get(0)?,
        meeting_id: r.get(1)?,
        at_ms: r.get(2)?,
        label: r.get(3)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meeting_crud_and_search() {
        let db = Db::open_in_memory().unwrap();
        db.create_meeting(
            "a",
            "Họp brief Tết",
            "Sao Mai Show",
            "Vinamilk",
            "auto",
            "[]",
        )
        .unwrap();
        db.create_meeting("b", "Tổng duyệt", "Concert X", "", "vi", "[]")
            .unwrap();
        assert_eq!(db.list_meetings(None).unwrap().len(), 2);
        assert_eq!(db.list_meetings(Some("vinamilk")).unwrap().len(), 1);

        db.conn
            .lock()
            .execute(
                "INSERT INTO segments(meeting_id, track, start_ms, end_ms, text) VALUES ('b', 'mic', 0, 1000, 'Màn LED pixel pitch 3.9')",
                [],
            )
            .unwrap();
        // FTS bỏ dấu: "man led" khớp "Màn LED"
        let hits = db.list_meetings(Some("man led")).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "b");
        // Ký tự đặc biệt không làm vỡ truy vấn
        assert!(db.list_meetings(Some("\"(*")).is_ok());

        let bm = db.add_bookmark("a", 1234, "chốt ngân sách").unwrap();
        assert_eq!(db.get_meeting("a").unwrap().bookmark_count, 1);
        db.delete_bookmark(bm.id).unwrap();

        db.delete_meeting("a").unwrap();
        assert_eq!(db.list_meetings(None).unwrap().len(), 1);
    }

    #[test]
    fn glossary_seeded_and_settings_roundtrip() {
        let db = Db::open_in_memory().unwrap();
        let n: i64 = db
            .conn
            .lock()
            .query_row("SELECT COUNT(*) FROM glossary", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n as usize, migrations::DEFAULT_GLOSSARY.len());
        let mut s = db.get_settings().unwrap();
        s.mic_device = Some("USB Mic".into());
        db.save_settings(&s).unwrap();
        assert_eq!(
            db.get_settings().unwrap().mic_device.as_deref(),
            Some("USB Mic")
        );
    }
}
