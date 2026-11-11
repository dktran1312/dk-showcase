//! Speech-to-text (M3).
//!
//! Kế hoạch: resample về 16 kHz mono → Silero VAD (có sẵn trong whisper.cpp) cắt theo câu →
//! `whisper-rs` với model `large-v3-turbo` q5 (CUDA / Vulkan / CPU; Metal trên macOS) →
//! `Segment { start_ms, end_ms, track, lang, text }` lưu thẳng vào bảng `segments`.
//! Từ điển thuật ngữ (bảng `glossary`) được đưa vào `initial_prompt`.
