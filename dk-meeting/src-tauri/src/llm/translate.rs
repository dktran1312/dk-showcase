//! Dịch trực tiếp từng câu sang tiếng Việt (chạy nền, không chặn phụ đề).

use std::collections::VecDeque;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crossbeam_channel::{unbounded, Receiver, Sender};

use super::ollama::{ChatOptions, Message, Ollama};

pub struct TranslateJob {
    pub segment_id: i64,
    pub text: String,
    pub lang: String,
}

pub type ResultFn = Arc<dyn Fn(i64, String) + Send + Sync>;
pub type ErrorFn = Arc<dyn Fn(String) + Send + Sync>;

pub struct Translator {
    tx: Option<Sender<TranslateJob>>,
    thread: Option<JoinHandle<()>>,
}

impl Translator {
    pub fn start(
        ollama: Ollama,
        model: String,
        glossary: Vec<String>,
        on_result: ResultFn,
        on_error: ErrorFn,
    ) -> Self {
        let (tx, rx) = unbounded();
        let thread = std::thread::Builder::new()
            .name("translator".into())
            .spawn(move || worker(rx, ollama, model, glossary, on_result, on_error))
            .expect("spawn translator");
        Self {
            tx: Some(tx),
            thread: Some(thread),
        }
    }

    pub fn sender(&self) -> Sender<TranslateJob> {
        self.tx.clone().expect("translator đang chạy")
    }

    /// Chờ dịch xong hàng đợi. Chỉ kết thúc khi mọi `sender()` đã bị drop.
    pub fn finish(mut self) {
        self.tx.take();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

pub fn system_prompt(terms: &[String]) -> String {
    let mut s = String::from(
        "Bạn là phiên dịch viên cho các cuộc họp ngành tổ chức sự kiện / show sân khấu tại Việt Nam.\n\
         Nhiệm vụ: dịch câu nói sang tiếng Việt tự nhiên, ngắn gọn, đúng văn nói trong cuộc họp.\n\
         Quy tắc:\n\
         - Giữ nguyên thuật ngữ chuyên ngành, tên riêng, tên thương hiệu, con số và đơn vị.\n\
         - Chỉ trả về bản dịch của \"Câu cần dịch\". Không giải thích, không thêm ngoặc kép.",
    );
    if !terms.is_empty() {
        s.push_str(&format!(
            "\n- Thuật ngữ trong câu cần giữ nguyên: {}.",
            terms.join(", ")
        ));
    }
    s
}

/// Làm sạch bản dịch; trả về None nếu model "lặp lại hướng dẫn" thay vì dịch.
pub fn clean_translation(source: &str, out: &str) -> Option<String> {
    let mut t = super::plain_text(out);
    for prefix in ["Bản dịch:", "Dịch:", "Câu dịch:", "Translation:"] {
        if let Some(rest) = t.strip_prefix(prefix) {
            t = rest.trim().to_string();
        }
    }
    let t = t
        .trim_matches(|c| c == '"' || c == '“' || c == '”' || c == '\'')
        .trim()
        .to_string();
    let lower = t.to_lowercase();
    let src = source.to_lowercase();
    let echo = [
        "bản dịch",
        "câu cần dịch",
        "thuật ngữ chuyên ngành",
        "phiên dịch viên",
        "ngữ cảnh",
    ]
    .iter()
    .any(|p| lower.contains(p) && !src.contains(p));
    let too_long = t.chars().count() > source.chars().count() * 4 + 40;
    (!t.is_empty() && !echo && !too_long).then_some(t)
}

pub fn user_prompt(context: &VecDeque<String>, text: &str) -> String {
    let mut s = String::new();
    if !context.is_empty() {
        s.push_str("Ngữ cảnh (các câu ngay trước, KHÔNG dịch):\n");
        for c in context {
            s.push_str("- ");
            s.push_str(c);
            s.push('\n');
        }
        s.push('\n');
    }
    s.push_str("Câu cần dịch:\n");
    s.push_str(text);
    s
}

fn worker(
    rx: Receiver<TranslateJob>,
    ollama: Ollama,
    model: String,
    glossary: Vec<String>,
    on_result: ResultFn,
    on_error: ErrorFn,
) {
    let mut context: VecDeque<String> = VecDeque::new();
    let mut paused_until: Option<Instant> = None;
    let opts = ChatOptions {
        temperature: 0.2,
        num_ctx: 4096,
        format: None,
        timeout: Duration::from_secs(60),
    };

    for job in rx {
        if paused_until.is_some_and(|t| Instant::now() < t) {
            continue; // Ollama vừa lỗi: tạm bỏ qua, thử lại sau
        }
        paused_until = None;
        let terms = super::relevant_terms(&glossary, &job.text);
        let msgs = [
            Message::system(system_prompt(&terms)),
            Message::user(user_prompt(&context, &job.text)),
        ];
        match ollama.chat(&model, &msgs, &opts) {
            Ok(t) => match clean_translation(&job.text, &t) {
                Some(t) => on_result(job.segment_id, t),
                None => log::debug!("bỏ bản dịch không hợp lệ: {t:?}"),
            },
            Err(e) => {
                on_error(format!("Tạm dừng dịch: {e}"));
                paused_until = Some(Instant::now() + Duration::from_secs(30));
            }
        }
        log::debug!("dịch câu ({}) {}", job.lang, job.segment_id);
        context.push_back(job.text);
        while context.len() > 3 {
            context.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_echoed_instructions() {
        assert_eq!(
            clean_translation(
                "ASK!",
                "Hãy nhấn mạnh và giữ nguyên thuật ngữ chuyên ngành trong câu dịch."
            ),
            None
        );
        assert_eq!(
            clean_translation("Hello", "Bản dịch: \"Xin chào\""),
            Some("Xin chào".into())
        );
        assert_eq!(
            clean_translation("Pixel pitch 3.9", "**Pixel pitch 3.9**"),
            Some("Pixel pitch 3.9".into())
        );
    }
}
