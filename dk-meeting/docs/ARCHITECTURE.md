# DK Meeting — Kiến trúc

App desktop nội bộ của DK TRAN: ghi âm cuộc họp, phụ đề + dịch trực tiếp sang tiếng Việt,
và tạo biên bản họp theo mẫu ngành sự kiện. **100% chạy local, không dùng API trả phí.**

## 1. Sơ đồ tổng thể

```
┌──────────────────────────── UI (React + TS + Tailwind) ─────────────────────────────┐
│ Trang chủ │ Bắt đầu họp │ Live │ Chi tiết cuộc họp │ Settings │ Kiểm tra hệ thống   │
└───────────────▲───────────────────────────────────────────────▲──────────────────────┘
                │ invoke(command)                                │ events (level, state,
                │                                                │ segment, translation)
┌───────────────┴──────────────── Tauri 2 / Rust core ───────────┴──────────────────────┐
│ commands.rs  ── lớp API mỏng, không chứa logic, chỉ gọi xuống các module              │
│                                                                                       │
│ audio/   AudioSource trait ─┬─ MicSource (cpal)                 ← M1                  │
│                             ├─ WindowsProcessLoopback (WASAPI)  ← M2                  │
│                             ├─ WindowsSystemLoopback  (WASAPI)  ← M2                  │
│                             └─ MacProcessTap / ScreenCaptureKit ← M7                  │
│          Recorder: 1 thread / track → ghi WAV theo đoạn + đo mức âm + đẩy cho STT     │
│                                                                                       │
│ stt/     resample 16 kHz mono → Silero VAD → whisper-rs (CUDA/Vulkan/CPU, Metal) ← M3 │
│ llm/     Ollama client: dịch live, biên bản JSON (map-reduce), việc cần làm      ← M4 │
│ storage/ SQLite (rusqlite, bundled, FTS5) + thư mục audio                              │
│ export/  DOCX / PDF / Markdown / "Copy để gửi Zalo"                              ← M5 │
└───────────────────────────────────────────────────────────────────────────────────────┘
```

**Quy tắc đa nền tảng:** chỉ module `audio/` (và phần chọn backend GPU trong `stt/`) được phép
gọi API riêng của từng OS, đặt sau `#[cfg(target_os = "...")]`. UI, DB, LLM, export dùng chung
100% cho Windows và macOS.

## 2. Pipeline âm thanh

```
AudioSource (mic | system/app)          mỗi track chạy độc lập trên thread riêng
   │  f32 frames, sample rate gốc của thiết bị
   ▼
Recorder track thread ──► WavChunkWriter  (mono 16-bit, file 5 phút/đoạn, flush header mỗi ~1 s)
   │                 └──► LevelMeter      (RMS/peak → event `recording://level` ~15 lần/s)
   ▼ (M3)
Resampler 16 kHz mono → VAD (cắt theo câu) → whisper → Segment{start,end,track,lang,text}
   │                                                      │ lưu DB ngay
   ▼ (M4)                                                 ▼
lang != vi → Ollama dịch (kèm 2–3 câu trước + từ điển) → event `segment://*` → UI
```

- **2 track riêng:** `mic` = "Phía mình", `system` = "Phía online". Nhãn người nói thô lấy từ track,
  chưa cần diarization. Khi xuất file âm thanh thì mix 2 track lại.
- **Không mất dữ liệu khi crash:** WAV được ghi theo đoạn 5 phút; header được cập nhật mỗi giây
  (`hound::WavWriter::flush`) nên file luôn mở được kể cả khi app bị tắt đột ngột. Segment lưu vào
  DB ngay khi có. Lần mở app sau, cuộc họp đang ghi dở sẽ được đánh dấu "Bị gián đoạn" và
  thời lượng được tính lại từ file.
- **Định dạng lưu trữ:** M1 ghi WAV (an toàn khi crash, không cần thư viện native). Từ M5, sau khi
  dừng họp sẽ nén các đoạn sang Opus/OGG (hoặc FLAC) để tiết kiệm dung lượng.
- **UI không đứng:** cpal callback chỉ đẩy mẫu vào channel; ghi file, đo mức, nhận dạng, gọi LLM
  đều chạy ở thread/tokio task riêng. Command Tauri không bao giờ block lâu.

## 3. Cơ sở dữ liệu (SQLite)

| Bảng | Nội dung | Mốc |
|---|---|---|
| `meetings` | id, title, project, client, language, status, started_at, ended_at, duration_ms | M1 |
| `audio_chunks` | meeting_id, track, idx, file_name, sample_rate, start_ms, duration_ms | M1 |
| `bookmarks` | meeting_id, at_ms, label | M1 |
| `segments` (+ `segments_fts` FTS5) | meeting_id, track, start_ms, end_ms, lang, text, translation, edited | tạo M1, dùng M3 |
| `glossary` | term, note (seed sẵn từ điển ngành sự kiện) | tạo M1, UI M5 |
| `settings` | key → JSON value | M1 |
| `minutes`, `action_items` | biên bản JSON theo schema, việc cần làm | M4 |

Schema có version (`PRAGMA user_version`), migration chạy tự động khi mở app.
DB bật `journal_mode=WAL` để ghi segment liên tục mà vẫn đọc được.

Thư mục dữ liệu (Windows): `%LOCALAPPDATA%\com.dktran.meeting\`
```
dk-meeting.db
recordings/<meeting-id>/mic_000.wav, mic_001.wav, system_000.wav, ...
models/whisper/...       (M3)
```

## 4. Cấu trúc thư mục

```
dk-meeting/
├── docs/ARCHITECTURE.md
├── package.json · vite.config.ts · tsconfig.json · index.html
├── src/                              # UI React
│   ├── main.tsx · App.tsx · index.css
│   ├── lib/
│   │   ├── api.ts                    # wrapper có kiểu cho mọi invoke() + listen()
│   │   ├── types.ts                  # type dùng chung, khớp với struct Rust (serde camelCase)
│   │   └── format.ts                 # định dạng thời lượng, ngày giờ
│   ├── components/
│   │   ├── TitleBar.tsx · Logo.tsx · Splash.tsx
│   │   ├── LevelMeter.tsx · Button.tsx · RecordingConsent.tsx
│   └── pages/
│       ├── Home.tsx · NewMeeting.tsx · Live.tsx · MeetingDetail.tsx · Settings.tsx
└── src-tauri/                        # Rust core
    ├── Cargo.toml · tauri.conf.json · build.rs
    ├── capabilities/default.json · icons/
    └── src/
        ├── main.rs · lib.rs          # khởi tạo app, state, đăng ký command
        ├── commands.rs               # các #[tauri::command]
        ├── state.rs                  # AppState (DB, recorder đang chạy, đường dẫn)
        ├── error.rs                  # AppError → trả về UI dạng chuỗi tiếng Việt
        ├── audio/
        │   ├── mod.rs                # trait AudioSource, AudioFormat, SourceKind, Track
        │   ├── mic.rs                # MicSource (cpal) — mọi OS
        │   ├── wav_writer.rs         # WavChunkWriter (đoạn 5 phút, flush mỗi giây)
        │   ├── level.rs              # đo RMS/peak
        │   ├── recorder.rs           # điều phối các track, pause/resume/stop, timer
        │   ├── windows/              # (M2) process_loopback.rs, system_loopback.rs
        │   └── macos/                # (M7) process_tap.rs
        ├── storage/
        │   ├── mod.rs · migrations.rs · models.rs
        ├── stt/mod.rs                # (M3) whisper + VAD
        ├── llm/mod.rs                # (M4) Ollama
        └── export/mod.rs             # (M5) DOCX/PDF/MD/Zalo
```

## 5. Hợp đồng `AudioSource`

```rust
pub trait AudioSource: Send {
    fn kind(&self) -> SourceKind;              // Mic | SystemLoopback | ProcessLoopback{pid}
    fn track(&self) -> Track;                  // Mine ("Phía mình") | Remote ("Phía online")
    fn format(&self) -> AudioFormat;           // sample_rate, channels của thiết bị
    /// Bắt đầu thu; mỗi khối mẫu f32 interleaved được gửi qua `sink`.
    /// Trả về một handle; drop handle = dừng thu.
    fn start(&mut self, sink: FrameSink) -> Result<Box<dyn CaptureHandle>>;
}
```

`MicSource` dùng cpal. Vì stream của cpal không `Send` trên Windows, mỗi source tự giữ stream trong
một thread riêng và chỉ lộ ra handle `Send`. Implement WASAPI (M2) và Core Audio tap (M7) làm theo
đúng mẫu này nên `Recorder` và UI không phải sửa.

## 6. Lộ trình

| Mốc | Nội dung |
|---|---|
| **M1** | Khung Tauri + UI tối, branding DK TRAN; thu micro + ghi file; danh sách cuộc họp; SQLite |
| M2 | WASAPI process loopback + chọn app đang phát âm thanh; system loopback dự phòng; 2 track |
| M3 | whisper.cpp + Silero VAD → phụ đề live; Settings model; tự nhận GPU; màn hình Kiểm tra hệ thống |
| M4 | Ollama → dịch live, biên bản JSON (map-reduce), Việc cần làm, Tóm tắt nhanh |
| M5 | Transcript bấm-để-nghe, sửa transcript, tìm kiếm FTS, xuất DOCX/PDF/MD/Zalo, từ điển thuật ngữ, nén Opus |
| M6 | Installer `.msi`/`.exe`, README cài đặt (kèm Ollama + model) |
| M7 | macOS: MacProcessTap, Metal, `.dmg` |
