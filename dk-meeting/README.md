# DK Meeting

App desktop nội bộ của **DK TRAN**: ghi âm cuộc họp, phụ đề + dịch trực tiếp sang tiếng Việt,
tạo biên bản họp theo mẫu ngành sự kiện. 100% chạy trên máy, không dùng API trả phí.

Kiến trúc chi tiết: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)

## Trạng thái

| Mốc | Nội dung | Trạng thái |
|---|---|---|
| M1 | Khung Tauri + UI tối DK TRAN, thu micro + ghi file, danh sách cuộc họp, SQLite | ✅ |
| M3 | whisper.cpp + Silero VAD → phụ đề live, tải model trong app, nhận GPU, Kiểm tra hệ thống | ✅ |
| M4 | Ollama → dịch live, Tóm tắt nhanh, biên bản (JSON schema, map-reduce), Việc cần làm | ✅ |
| M2 | Thu âm thanh app họp (WASAPI process loopback), 2 track | ⏳ |
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

Từ M3 cần thêm **CMake** và **LLVM** (để biên dịch whisper.cpp — bộ nhận giọng nói):

```powershell
winget install Kitware.CMake
winget install LLVM.LLVM
```
Không có winget: tải CMake tại <https://cmake.org/download/> (khi cài chọn *Add CMake to the system PATH*) và LLVM tại
<https://github.com/llvm/llvm-project/releases> (file `LLVM-<phiên bản>-win64.exe`, cài vào thư mục mặc định).
Mở lại PowerShell, gõ `cmake --version` để kiểm tra.

> Báo lỗi `Unable to find libclang` → LLVM chưa cài vào `C:\Program Files\LLVM`. Chạy
> `setx LIBCLANG_PATH "C:\Program Files\LLVM\bin"` rồi mở lại PowerShell.

### 2. Chạy app

```powershell
cd dk-meeting
npm install
npm run tauri dev
```

Lần đầu Rust sẽ biên dịch khá lâu (5–10 phút); các lần sau nhanh hơn nhiều.

Lệnh trên chạy nhận giọng nói bằng **CPU**. Máy có card NVIDIA nên dùng bản **GPU (CUDA)** bên dưới — nhanh hơn hàng chục lần.

### 3. Bật GPU (CUDA) — cho máy có card NVIDIA

1. Cài **CUDA Toolkit 12.8 trở lên** (card RTX 50xx bắt buộc ≥ 12.8): <https://developer.nvidia.com/cuda-downloads>
   → Windows → x86_64 → 11 (hoặc 10) → exe (local). Cài kiểu *Express*.
   - Phải cài **sau** Visual C++ Build Tools để CUDA tự gắn vào Build Tools.
2. Mở lại PowerShell, kiểm tra: `nvcc --version`.
3. Chạy:
   ```powershell
   cd D:\_Dev\dk-showcase\dk-meeting
   npm run dev:cuda
   ```
   Lần đầu biên dịch CUDA mất **15–30 phút** (chỉ một lần). Vào **Cài đặt → Kiểm tra hệ thống**: dòng
   “Nhận giọng nói chạy bằng” phải là **GPU (CUDA)**.

Lỗi hay gặp: `No CUDA toolset found` → chạy lại bộ cài CUDA, chọn *Custom* và tick **Visual Studio Integration**.

### 4. Chuẩn bị AI (một lần)

Lần đầu mở app sẽ hiện màn hình **Kiểm tra hệ thống**:
- **Model nhận giọng nói**: bấm *Tải về* ở dòng *Large v3 Turbo* (574 MB, gợi ý cho máy có GPU).
- **Ollama**: cài tại <https://ollama.com/download>, mở Ollama, rồi bấm *Tải qwen3:8b* (~5 GB) ngay trong app
  (hoặc chạy `ollama pull qwen3:8b`).

Thiếu model thì app vẫn ghi âm bình thường; về sau vào tab Transcript bấm **Tạo transcript từ file ghi âm**.

### 5. Chạy test

```powershell
cd dk-meeting\src-tauri
cargo test
```

## Cách dùng (M3 + M4)

1. **Bắt đầu họp** → chọn ngôn ngữ (*Tự nhận diện* nếu có người nói tiếng Anh) → phụ đề hiện sau khi người nói ngắt câu 2–5 giây.
   Câu không phải tiếng Việt được dịch ở cột phải.
2. **Tóm tắt nhanh tới giờ**: tóm tắt các ý đã bàn, hiện ở cột phải (dùng khi vào họp trễ / cần chốt nhanh).
3. **Dừng & lưu** → màn chi tiết → **✦ Tạo biên bản**: biên bản theo mẫu ngành sự kiện (10 mục), sửa được (*✎ Sửa biên bản*).
   Bấm ▶ mm:ss cạnh quyết định / việc cần làm để nghe lại đúng đoạn đó.
4. Tab **Việc cần làm**: sửa trực tiếp người phụ trách, deadline, trạng thái — tự lưu.
5. Tab **Transcript**: bấm vào câu để nghe lại.

## Dữ liệu được lưu ở đâu?

`%LOCALAPPDATA%\com.dktran.meeting\`

```
dk-meeting.db                 ← SQLite: cuộc họp, đánh dấu, cài đặt, từ điển thuật ngữ
recordings\<id cuộc họp>\     ← mic_000.wav, mic_001.wav, ... (mỗi file tối đa 5 phút)
models\whisper\               ← model nhận giọng nói (ggml-*.bin)
```

Muốn reset sạch: tắt app rồi xoá thư mục trên.

## Cấu trúc mã nguồn

```
src/            UI React + TypeScript + Tailwind (pages/, components/, lib/api.ts)
src-tauri/src/  Rust core
  audio/        AudioSource trait, MicSource (cpal), ghi WAV theo đoạn, Recorder
  stt/          resample 16 kHz, Silero VAD (tăng dần), cắt câu, whisper, tải model
  llm/          client Ollama, dịch live, biên bản (schema + map-reduce), tóm tắt nhanh
  live.rs       nối stt + llm với DB và UI cho từng cuộc họp
  storage/      SQLite + migration
  commands.rs, commands_ai.rs   các lệnh UI gọi xuống
```
