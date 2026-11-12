//! Dịch & tóm tắt bằng Ollama chạy local.
//! - `translate`: dịch live từng câu (kèm 2–3 câu trước + từ điển thuật ngữ).
//! - `minutes`: biên bản JSON theo schema cố định, map-reduce theo đoạn ~10 phút.

pub mod minutes;
pub mod ollama;
pub mod translate;

/// Chỉ giữ các thuật ngữ thật sự xuất hiện trong văn bản (prompt ngắn, model nhỏ không bị "chép" danh sách).
pub fn relevant_terms(glossary: &[String], text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    glossary
        .iter()
        .filter(|t| !t.is_empty() && lower.contains(&t.to_lowercase()))
        .cloned()
        .collect()
}

/// Bỏ định dạng markdown đơn giản mà model hay thêm vào.
pub fn plain_text(s: &str) -> String {
    s.replace("**", "").replace("__", "").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_relevant_terms_only() {
        let g: Vec<String> = ["LED", "pixel pitch", "truss", "MC"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            relevant_terms(&g, "màn led 12x6, Pixel Pitch 3.9"),
            vec!["LED", "pixel pitch"]
        );
    }
}
