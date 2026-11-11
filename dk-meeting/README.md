# DK Meeting

App desktop nội bộ của **DK TRAN**: ghi âm cuộc họp, phụ đề + dịch trực tiếp sang tiếng Việt,
tạo biên bản họp theo mẫu ngành sự kiện. 100% chạy trên máy, không dùng API trả phí.

Kiến trúc chi tiết: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)

## Trạng thái

| Mốc | Nội dung | Trạng thái |
|---|---|---|
| M1 | Khung Tauri + UI tối DK TRAN, thu micro + ghi file, danh sách cuộc họp, SQLite | ✅ |
| M2 | Thu âm thanh app họp (WASAPI process loopback), 2 track | ⏳ |
| M3 | whisper.cpp + VAD → phụ đề live | ⏳ |
| M4 | Ollama → dịch live, biên bản, việc cần làm | ⏳ |
| M5 | Transcript bấm-để-nghe, tìm kiếm, xuất file, từ điển | ⏳ |
| M6 | Installer Windows | ⏳ |

## Chạy thử trên Windows (bản dev)

### 1. Cài công cụ (một lần)

1. **Microsoft C++ Build Tools** — <https://visualstudio.microsoft.com/visual-cpp-build-tools/>
   → chọn workload **"Desktop development with C++"**.
2. **Rust** — <https://rustup.rs> (chọn mặc định `stable-x86_64-pc-windows-msvc`).
3. **Node.js 20+** — <https://nodejs.org> (bản LTS).
4. **WebView2** — Windows 11 có sẵn. Windows 10 nếu thiếu thì cài "Evergreen Bootstrapper" từ Microsoft.

Kiểm tra: mở PowerShell, gõ `rustc -V`, `cargo -V`, `node -v` đều ra phiên bản.

### 2. Chạy app

```powershell
cd dk-meeting
npm install
npm run tauri dev
```

Lần đầu Rust sẽ biên dịch khá lâu (5–10 phút); các lần sau nhanh hơn nhiều.

### 3. Chạy test

```powershell
cd dk-meeting\src-tauri
cargo test
```

## Dữ liệu được lưu ở đâu?

`%LOCALAPPDATA%\com.dktran.meeting\`

```
dk-meeting.db                 ← SQLite: cuộc họp, đánh dấu, cài đặt, từ điển thuật ngữ
recordings\<id cuộc họp>\     ← mic_000.wav, mic_001.wav, ... (mỗi file tối đa 5 phút)
```

Muốn reset sạch: tắt app rồi xoá thư mục trên.

## Cấu trúc mã nguồn

```
src/            UI React + TypeScript + Tailwind (pages/, components/, lib/api.ts)
src-tauri/src/  Rust core
  audio/        AudioSource trait, MicSource (cpal), ghi WAV theo đoạn, Recorder
  storage/      SQLite + migration
  stt/ llm/ export/   (khung cho M3–M5)
  commands.rs   các lệnh UI gọi xuống
```
