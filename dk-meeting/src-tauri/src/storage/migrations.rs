//! Migration có version theo `PRAGMA user_version`. Chỉ được THÊM migration mới, không sửa cái cũ.

use rusqlite::Connection;

use crate::error::AppResult;

const MIGRATIONS: &[&str] = &[
    // v1 — M1
    r#"
    CREATE TABLE meetings (
        id          TEXT PRIMARY KEY,
        title       TEXT NOT NULL,
        project     TEXT NOT NULL DEFAULT '',
        client      TEXT NOT NULL DEFAULT '',
        language    TEXT NOT NULL DEFAULT 'auto',
        sources     TEXT NOT NULL DEFAULT '[]',
        status      TEXT NOT NULL,
        created_at  TEXT NOT NULL,
        started_at  TEXT NOT NULL,
        ended_at    TEXT,
        duration_ms INTEGER NOT NULL DEFAULT 0,
        notes       TEXT NOT NULL DEFAULT ''
    );
    CREATE INDEX idx_meetings_started ON meetings(started_at DESC);

    CREATE TABLE audio_chunks (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        meeting_id  TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
        track       TEXT NOT NULL,
        idx         INTEGER NOT NULL,
        file_name   TEXT NOT NULL,
        sample_rate INTEGER NOT NULL,
        start_ms    INTEGER NOT NULL,
        duration_ms INTEGER NOT NULL DEFAULT 0,
        UNIQUE(meeting_id, track, idx)
    );

    CREATE TABLE bookmarks (
        id         INTEGER PRIMARY KEY AUTOINCREMENT,
        meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
        at_ms      INTEGER NOT NULL,
        label      TEXT NOT NULL DEFAULT '',
        created_at TEXT NOT NULL
    );
    CREATE INDEX idx_bookmarks_meeting ON bookmarks(meeting_id, at_ms);

    -- Dùng từ M3 (phụ đề) — tạo sẵn để tìm kiếm toàn văn hoạt động ngay.
    CREATE TABLE segments (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        meeting_id  TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
        track       TEXT NOT NULL,
        start_ms    INTEGER NOT NULL,
        end_ms      INTEGER NOT NULL,
        lang        TEXT NOT NULL DEFAULT '',
        text        TEXT NOT NULL,
        translation TEXT,
        edited      INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX idx_segments_meeting ON segments(meeting_id, start_ms);

    CREATE VIRTUAL TABLE segments_fts USING fts5(
        text, translation,
        content='segments', content_rowid='id',
        tokenize='unicode61 remove_diacritics 2'
    );
    CREATE TRIGGER segments_ai AFTER INSERT ON segments BEGIN
        INSERT INTO segments_fts(rowid, text, translation) VALUES (new.id, new.text, new.translation);
    END;
    CREATE TRIGGER segments_ad AFTER DELETE ON segments BEGIN
        INSERT INTO segments_fts(segments_fts, rowid, text, translation)
        VALUES ('delete', old.id, old.text, old.translation);
    END;
    CREATE TRIGGER segments_au AFTER UPDATE ON segments BEGIN
        INSERT INTO segments_fts(segments_fts, rowid, text, translation)
        VALUES ('delete', old.id, old.text, old.translation);
        INSERT INTO segments_fts(rowid, text, translation) VALUES (new.id, new.text, new.translation);
    END;

    CREATE TABLE glossary (
        id      INTEGER PRIMARY KEY AUTOINCREMENT,
        term    TEXT NOT NULL UNIQUE COLLATE NOCASE,
        note    TEXT NOT NULL DEFAULT '',
        builtin INTEGER NOT NULL DEFAULT 0
    );

    CREATE TABLE settings (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    "#,
];

/// Từ điển thuật ngữ mặc định của ngành sự kiện (seed 1 lần ở v1).
pub const DEFAULT_GLOSSARY: &[&str] = &[
    "LED",
    "màn LED",
    "pixel map",
    "pixel pitch",
    "visual",
    "VJ",
    "Resolume",
    "media server",
    "mapping",
    "cue",
    "run-down",
    "kịch bản",
    "rehearsal",
    "tổng duyệt",
    "soundcheck",
    "line check",
    "ánh sáng",
    "moving head",
    "beam",
    "laser",
    "SFX",
    "CO2",
    "pháo sáng",
    "pháo giấy",
    "khói",
    "sân khấu",
    "truss",
    "backdrop",
    "key visual",
    "KV",
    "brief",
    "concept",
    "key message",
    "MC",
    "ca sĩ",
    "nghệ sĩ",
    "nhãn hàng",
    "client",
    "agency",
    "production",
    "timeline",
    "deadline",
    "budget",
    "báo giá",
    "venue",
    "setup",
    "load-in",
    "load-out",
    "opening",
    "show call",
    "stage manager",
    "đạo diễn",
    "in-ear",
    "monitor",
    "FOH",
    "LTC",
    "timecode",
];

pub fn run(conn: &mut Connection) -> AppResult<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate() {
        let version = i as i64 + 1;
        if version <= current {
            continue;
        }
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        if version == 1 {
            let mut stmt =
                tx.prepare("INSERT OR IGNORE INTO glossary(term, builtin) VALUES (?1, 1)")?;
            for term in DEFAULT_GLOSSARY {
                stmt.execute([term])?;
            }
        }
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
        log::info!("DB migrated to v{version}");
    }
    Ok(())
}
