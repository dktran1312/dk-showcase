//! Dịch & tóm tắt bằng Ollama chạy local (M4).
//!
//! Kế hoạch: client HTTP tới `http://localhost:11434`, model mặc định `qwen3:8b`.
//! - Dịch live từng câu (kèm 2–3 câu trước + từ điển thuật ngữ), chỉ trả về bản dịch.
//! - Biên bản: JSON theo schema cố định (structured output), map-reduce theo đoạn ~10 phút.
