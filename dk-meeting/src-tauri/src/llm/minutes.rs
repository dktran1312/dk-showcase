//! Biên bản họp ngành sự kiện: LLM trả JSON theo schema cố định rồi mới render ra UI/file.
//! Cuộc họp dài dùng map-reduce: chia transcript ~10 phút/đoạn → ghi chú JSON từng đoạn → gộp.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::ollama::{ChatOptions, Message, Ollama};
use crate::error::{AppError, AppResult};

pub const NOT_MENTIONED: &str = "Chưa đề cập";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    pub title: String,
    pub date: String,
    pub duration: String,
    pub participants: Vec<String>,
    pub client: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Brief {
    pub objective: String,
    pub concept: String,
    pub key_message: String,
    pub audience: String,
    pub show_date: String,
    pub venue: String,
    pub scale: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TechItem {
    pub requirements: Vec<String>,
    pub numbers: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Technical {
    pub led_visual: TechItem,
    pub sound: TechItem,
    pub lighting: TechItem,
    pub sfx: TechItem,
    pub stage: TechItem,
    pub artist_mc: TechItem,
    pub other: TechItem,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RundownItem {
    pub time: String,
    pub content: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BudgetItem {
    pub item: String,
    pub amount: String,
    pub note: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Decision {
    pub content: String,
    /// "mm:ss" hoặc "h:mm:ss" — bấm để nghe lại.
    pub source_time: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ActionItem {
    pub task: String,
    pub owner: String,
    pub deadline: String,
    pub source_time: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Minutes {
    pub general: General,
    pub summary: Vec<String>,
    pub brief: Brief,
    pub technical: Technical,
    pub rundown: Vec<RundownItem>,
    pub budget: Vec<BudgetItem>,
    pub decisions: Vec<Decision>,
    pub action_items: Vec<ActionItem>,
    pub confirm_with_client: Vec<String>,
    pub risks: Vec<String>,
}

/// JSON schema gửi cho Ollama (structured outputs). `general.title/date/duration` do app tự điền.
pub fn schema() -> Value {
    let s = json!({"type": "string"});
    let arr_s = json!({"type": "array", "items": {"type": "string"}});
    let tech = json!({
        "type": "object",
        "properties": {"requirements": arr_s, "numbers": arr_s},
        "required": ["requirements", "numbers"]
    });
    let obj = |props: Value, req: &[&str]| json!({"type": "object", "properties": props, "required": req});
    json!({
        "type": "object",
        "properties": {
            "general": obj(json!({"participants": arr_s, "client": s}), &["participants", "client"]),
            "summary": arr_s,
            "brief": obj(
                json!({"objective": s, "concept": s, "key_message": s, "audience": s, "show_date": s, "venue": s, "scale": s}),
                &["objective", "concept", "key_message", "audience", "show_date", "venue", "scale"]
            ),
            "technical": obj(
                json!({"led_visual": tech, "sound": tech, "lighting": tech, "sfx": tech, "stage": tech, "artist_mc": tech, "other": tech}),
                &["led_visual", "sound", "lighting", "sfx", "stage", "artist_mc", "other"]
            ),
            "rundown": {"type": "array", "items": obj(json!({"time": s, "content": s}), &["time", "content"])},
            "budget": {"type": "array", "items": obj(json!({"item": s, "amount": s, "note": s}), &["item", "amount", "note"])},
            "decisions": {"type": "array", "items": obj(json!({"content": s, "source_time": s}), &["content", "source_time"])},
            "action_items": {"type": "array", "items": obj(
                json!({"task": s, "owner": s, "deadline": s, "source_time": s}),
                &["task", "owner", "deadline", "source_time"]
            )},
            "confirm_with_client": arr_s,
            "risks": arr_s
        },
        "required": ["general", "summary", "brief", "technical", "rundown", "budget", "decisions", "action_items", "confirm_with_client", "risks"]
    })
}

/// Một dòng transcript đưa cho LLM.
pub struct Line {
    pub start_ms: u64,
    pub speaker: String,
    pub text: String,
    pub translation: Option<String>,
}

pub fn fmt_time(ms: u64) -> String {
    let s = ms / 1000;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
    } else {
        format!("{:02}:{:02}", s / 60, s % 60)
    }
}

/// "mm:ss" / "h:mm:ss" → ms.
pub fn parse_time(t: &str) -> Option<u64> {
    let parts: Vec<u64> = t
        .trim()
        .trim_matches(['[', ']'])
        .split(':')
        .map(|p| p.trim().parse().ok())
        .collect::<Option<_>>()?;
    let secs = match parts.as_slice() {
        [m, s] => m * 60 + s,
        [h, m, s] => h * 3600 + m * 60 + s,
        _ => return None,
    };
    Some(secs * 1000)
}

pub fn render_transcript(lines: &[Line]) -> String {
    let mut out = String::new();
    for l in lines {
        out.push_str(&format!(
            "[{}] {}: {}",
            fmt_time(l.start_ms),
            l.speaker,
            l.text
        ));
        if let Some(t) = l.translation.as_deref().filter(|t| !t.is_empty()) {
            out.push_str(&format!(" (dịch: {t})"));
        }
        out.push('\n');
    }
    out
}

/// Chia transcript thành các đoạn ~`window_ms` (cắt ở ranh giới câu).
pub fn chunk_lines(lines: &[Line], window_ms: u64) -> Vec<&[Line]> {
    let mut chunks = Vec::new();
    let mut start = 0;
    for i in 1..=lines.len() {
        let end_of_input = i == lines.len();
        if end_of_input || lines[i].start_ms >= lines[start].start_ms + window_ms {
            chunks.push(&lines[start..i]);
            start = i;
        }
    }
    chunks
}

fn rules(glossary: &[String]) -> String {
    format!(
        "Bạn là thư ký cuộc họp của một công ty tổ chức sự kiện / show sân khấu (làm việc với nhãn hàng, doanh nghiệp, ca sĩ).\n\
         QUY TẮC BẮT BUỘC:\n\
         1. Chỉ ghi những gì CÓ trong transcript. TUYỆT ĐỐI không bịa thông tin, con số, tên người, ngày tháng.\n\
         2. Trường dạng chữ không có thông tin thì ghi đúng \"{NOT_MENTIONED}\". Danh sách không có thông tin thì để mảng rỗng [].\n\
         3. Viết tiếng Việt, ngắn gọn, rõ ý, giữ nguyên thuật ngữ chuyên ngành và con số.\n\
         4. Mỗi quyết định và việc cần làm phải có source_time = thời điểm [mm:ss] của câu gốc trong transcript.\n\
         5. \"Phía mình\" / \"Phía online\" chỉ là nhãn kênh âm thanh (team công ty sự kiện / người họp qua mạng), \
            KHÔNG phải tên người: không ghi chúng vào participants, client hay owner.\n\
         6. action_items chỉ gồm việc thật sự được giao hoặc hứa sẽ làm; không chép lại câu nói. Không có thì để [].\n\
         7. Transcript do máy nhận dạng giọng nói nên có thể sai chính tả; hãy hiểu theo ngữ cảnh ngành sự kiện \
            (thuật ngữ xuất hiện: {}).\n\
         Ý nghĩa các trường:\n\
         - general.participants: tên người tham gia nghe được; general.client: khách hàng / nhãn hàng / nghệ sĩ.\n\
         - summary: 5–7 ý chính của cuộc họp.\n\
         - brief: mục tiêu sự kiện, concept, key message, đối tượng khán giả, ngày show, địa điểm (venue), quy mô.\n\
         - technical: từng hạng mục (led_visual = LED/Visual, sound = âm thanh, lighting = ánh sáng, sfx = hiệu ứng CO2/pháo/khói/laser, \
           stage = sân khấu/kết cấu/truss, artist_mc = nghệ sĩ/MC, other = khác); requirements = yêu cầu, numbers = con số (kích thước, số lượng, thời lượng...).\n\
         - rundown: các mốc run-down / timeline được nhắc tới.\n\
         - budget: ngân sách / con số tiền được nhắc tới.\n\
         - decisions: những điều đã chốt.\n\
         - action_items: việc cần làm, ai làm (owner), hạn (deadline); không rõ thì \"{NOT_MENTIONED}\".\n\
         - confirm_with_client: câu hỏi còn bỏ ngỏ / điểm cần confirm lại với khách.\n\
         - risks: rủi ro, lưu ý.",
        glossary.join(", ")
    )
}

pub struct MinutesJob<'a> {
    pub ollama: &'a Ollama,
    pub model: &'a str,
    pub glossary: &'a [String],
    pub lines: &'a [Line],
    pub general: General,
}

const WINDOW_MS: u64 = 10 * 60 * 1000;
const REDUCE_GROUP: usize = 4;

fn opts() -> ChatOptions {
    ChatOptions {
        temperature: 0.1,
        num_ctx: 16384,
        format: Some(schema()),
        timeout: Duration::from_secs(600),
    }
}

fn call(job: &MinutesJob, user: String) -> AppResult<Minutes> {
    let terms = super::relevant_terms(job.glossary, &user);
    let msgs = [Message::system(rules(&terms)), Message::user(user)];
    let raw = job.ollama.chat(job.model, &msgs, &opts())?;
    parse_minutes(&raw)
}

pub fn parse_minutes(raw: &str) -> AppResult<Minutes> {
    let start = raw.find('{').unwrap_or(0);
    let end = raw.rfind('}').map(|i| i + 1).unwrap_or(raw.len());
    serde_json::from_str(&raw[start..end]).map_err(|e| {
        AppError::msg(format!(
            "Model trả về biên bản không đúng định dạng, hãy thử lại. ({e})"
        ))
    })
}

/// Tạo biên bản. `progress(bước hiện tại, tổng số bước, mô tả)`.
pub fn generate(job: MinutesJob, progress: &dyn Fn(usize, usize, &str)) -> AppResult<Minutes> {
    if job.lines.is_empty() {
        return Err(AppError::msg(
            "Cuộc họp chưa có transcript nên chưa tạo được biên bản.",
        ));
    }
    let chunks = chunk_lines(job.lines, WINDOW_MS);
    let mut minutes = if chunks.len() == 1 {
        progress(1, 1, "Đang viết biên bản…");
        call(
            &job,
            format!(
                "TRANSCRIPT CUỘC HỌP:\n{}\n\nHãy lập biên bản theo đúng schema.",
                render_transcript(job.lines)
            ),
        )?
    } else {
        let total = chunks.len() + chunks.len().div_ceil(REDUCE_GROUP).max(1);
        let mut step = 0;
        // MAP: ghi chú từng đoạn
        let mut notes = Vec::new();
        for (i, c) in chunks.iter().enumerate() {
            step += 1;
            progress(
                step,
                total,
                &format!(
                    "Đang đọc đoạn {}/{} ({}–{})…",
                    i + 1,
                    chunks.len(),
                    fmt_time(c[0].start_ms),
                    fmt_time(c[c.len() - 1].start_ms)
                ),
            );
            let user = format!(
                "Đây là ĐOẠN {}/{} của một cuộc họp dài. Ghi chú đầy đủ mọi thông tin của đoạn này theo schema \
                 (summary là các ý chính của riêng đoạn này).\n\nTRANSCRIPT ĐOẠN:\n{}",
                i + 1,
                chunks.len(),
                render_transcript(c)
            );
            notes.push(call(&job, user)?);
        }
        // REDUCE: gộp dần theo nhóm cho vừa context
        while notes.len() > 1 {
            let mut next = Vec::new();
            for group in notes.chunks(REDUCE_GROUP) {
                if group.len() == 1 {
                    next.push(group[0].clone());
                    continue;
                }
                step += 1;
                progress(step.min(total), total, "Đang tổng hợp biên bản…");
                let parts: Vec<String> = group
                    .iter()
                    .map(|m| serde_json::to_string(m).unwrap_or_default())
                    .collect();
                let user = format!(
                    "Dưới đây là ghi chú JSON của các đoạn liên tiếp trong CÙNG một cuộc họp. Hãy gộp thành MỘT biên bản: \
                     loại bỏ trùng lặp, giữ thông tin mới nhất nếu mâu thuẫn, giữ nguyên source_time, \
                     summary còn 5–7 ý chính của toàn bộ cuộc họp.\n\n{}",
                    parts.iter().enumerate().map(|(i, p)| format!("GHI CHÚ ĐOẠN {}:\n{p}", i + 1)).collect::<Vec<_>>().join("\n\n")
                );
                next.push(call(&job, user)?);
            }
            notes = next;
        }
        notes.pop().unwrap_or_default()
    };
    // Thông tin chung do app biết chắc chắn
    minutes.general.title = job.general.title;
    minutes.general.date = job.general.date;
    minutes.general.duration = job.general.duration;
    // Nhãn kênh âm thanh không phải tên người.
    let is_label = |s: &str| {
        let l = s.trim().to_lowercase();
        l.is_empty() || l == "phía mình" || l == "phía online" || l == NOT_MENTIONED.to_lowercase()
    };
    minutes.general.participants.retain(|p| !is_label(p));
    if is_label(&minutes.general.client) {
        minutes.general.client = NOT_MENTIONED.into();
    }
    let client_missing =
        minutes.general.client.trim().is_empty() || minutes.general.client == NOT_MENTIONED;
    if client_missing && !job.general.client.is_empty() {
        minutes.general.client = job.general.client;
    }
    Ok(minutes)
}

/// Tóm tắt nhanh (văn bản thuần, gạch đầu dòng) phần đã nói tới giờ.
pub fn quick_summary(
    ollama: &Ollama,
    model: &str,
    glossary: &[String],
    lines: &[Line],
) -> AppResult<String> {
    if lines.is_empty() {
        return Err(AppError::msg("Chưa có nội dung nào để tóm tắt."));
    }
    // Giữ phần cuối nếu quá dài (~ 12k ký tự)
    let mut text = render_transcript(lines);
    if text.len() > 12_000 {
        let cut = text.len() - 12_000;
        let cut = text[cut..].find('\n').map(|i| cut + i + 1).unwrap_or(cut);
        text = format!("(…phần đầu đã lược bớt…)\n{}", &text[cut..]);
    }
    let terms = super::relevant_terms(glossary, &text);
    let msgs = [
        Message::system(format!(
            "Bạn là thư ký cuộc họp ngành tổ chức sự kiện. Tóm tắt nhanh nội dung đã bàn tới thời điểm này \
             thành 5–7 gạch đầu dòng tiếng Việt, ngắn gọn, giữ thuật ngữ và con số. Sau đó thêm mục \
             \"Đã chốt:\" và \"Còn bỏ ngỏ:\" (mục nào không có thì bỏ hẳn, không lặp lại tiêu đề). \
             Không dùng định dạng in đậm. Không bịa, không nhắc lại hướng dẫn này. Thuật ngữ xuất hiện: {}.",
            if terms.is_empty() { "không có".to_string() } else { terms.join(", ") }
        )),
        Message::user(format!("TRANSCRIPT:\n{text}")),
    ];
    ollama
        .chat(
            model,
            &msgs,
            &ChatOptions {
                temperature: 0.2,
                num_ctx: 16384,
                format: None,
                timeout: Duration::from_secs(300),
            },
        )
        .map(|s| super::plain_text(&s))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(ms: u64, t: &str) -> Line {
        Line {
            start_ms: ms,
            speaker: "Phía mình".into(),
            text: t.into(),
            translation: None,
        }
    }

    #[test]
    fn time_roundtrip() {
        assert_eq!(fmt_time(65_000), "01:05");
        assert_eq!(fmt_time(3_725_000), "1:02:05");
        assert_eq!(parse_time("01:05"), Some(65_000));
        assert_eq!(parse_time("[1:02:05]"), Some(3_725_000));
        assert_eq!(parse_time("Chưa đề cập"), None);
    }

    #[test]
    fn chunks_by_window() {
        let lines: Vec<Line> = (0..25).map(|i| line(i * 60_000, "x")).collect(); // 25 phút
        let c = chunk_lines(&lines, WINDOW_MS);
        assert_eq!(c.len(), 3);
        assert_eq!(c[0].len(), 10);
        assert_eq!(c.iter().map(|x| x.len()).sum::<usize>(), 25);
    }

    #[test]
    fn parses_partial_json_with_defaults() {
        let m = parse_minutes("```json\n{\"summary\": [\"a\"], \"decisions\": [{\"content\": \"Chốt LED 12x6\"}]}\n```").unwrap();
        assert_eq!(m.summary, vec!["a"]);
        assert_eq!(m.decisions[0].content, "Chốt LED 12x6");
        assert!(m.action_items.is_empty());
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;

    /// DKM_OLLAMA_MODEL=qwen3:1.7b cargo test minutes_real -- --ignored --nocapture
    #[test]
    #[ignore]
    fn minutes_real_ollama() {
        let model = std::env::var("DKM_OLLAMA_MODEL").unwrap_or_else(|_| "qwen3:8b".into());
        let t = |ms: u64, who: &str, text: &str| Line {
            start_ms: ms,
            speaker: who.into(),
            text: text.into(),
            translation: None,
        };
        let lines = vec![
            t(
                5_000,
                "Phía online",
                "Chào team, bên mình là Vinamilk, muốn làm event ra mắt sản phẩm sữa hạt mới.",
            ),
            t(
                15_000,
                "Phía online",
                "Concept là Xanh từ hạt, key message là dinh dưỡng thuần thực vật cho gia đình.",
            ),
            t(32_000, "Phía mình", "Dạ, show dự kiến ngày nào và ở đâu ạ?"),
            t(
                38_000,
                "Phía online",
                "Ngày 15 tháng 3, tại GEM Center quận 1, khoảng 500 khách.",
            ),
            t(
                55_000,
                "Phía mình",
                "Màn LED chính em đề xuất 12 x 6 mét, pixel pitch 3.9, hai màn phụ 3 x 5 mét.",
            ),
            t(
                70_000,
                "Phía online",
                "OK chốt LED như vậy. Ngân sách tổng khoảng 1 tỷ 2.",
            ),
            t(
                90_000,
                "Phía mình",
                "Ca sĩ thì bên mình đề xuất Hà Anh Tuấn, cần confirm lịch.",
            ),
            t(
                105_000,
                "Phía mình",
                "Anh Nam sẽ gửi báo giá và kịch bản sơ bộ trước thứ sáu tuần này.",
            ),
            t(
                120_000,
                "Phía online",
                "Lưu ý venue chỉ cho load-in từ 10 giờ tối hôm trước.",
            ),
        ];
        let general = General {
            title: "Họp brief Vinamilk".into(),
            date: "12/11/2026".into(),
            duration: "3 phút".into(),
            ..Default::default()
        };
        let o = Ollama::new(DEFAULT_URL_FOR_TEST);
        let glossary: Vec<String> = ["LED", "pixel pitch", "load-in", "venue", "KV"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let started = std::time::Instant::now();
        let m = generate(
            MinutesJob {
                ollama: &o,
                model: &model,
                glossary: &glossary,
                lines: &lines,
                general,
            },
            &|a, b, s| println!("{a}/{b} {s}"),
        )
        .unwrap();
        println!(
            "{} trong {:?}",
            serde_json::to_string_pretty(&m).unwrap(),
            started.elapsed()
        );
        let s = quick_summary(&o, &model, &glossary, &lines).unwrap();
        println!("--- tóm tắt nhanh ---\n{s}");
    }
    const DEFAULT_URL_FOR_TEST: &str = crate::llm::ollama::DEFAULT_URL;
}
