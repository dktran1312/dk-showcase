//! Client tối giản cho Ollama chạy trên máy (http://localhost:11434).

use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
use crate::net;

pub const DEFAULT_URL: &str = "http://localhost:11434";
pub const DEFAULT_MODEL: &str = "qwen3:8b";

#[derive(Clone, Debug)]
pub struct Ollama {
    base: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaModel {
    pub name: String,
    #[serde(default)]
    pub size: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Message {
    pub role: &'static str,
    pub content: String,
}

impl Message {
    pub fn system(c: impl Into<String>) -> Self {
        Self {
            role: "system",
            content: c.into(),
        }
    }
    pub fn user(c: impl Into<String>) -> Self {
        Self {
            role: "user",
            content: c.into(),
        }
    }
}

pub struct ChatOptions {
    pub temperature: f32,
    pub num_ctx: u32,
    /// JSON schema để Ollama ép đầu ra đúng cấu trúc (structured outputs).
    pub format: Option<Value>,
    pub timeout: Duration,
}

impl Default for ChatOptions {
    fn default() -> Self {
        Self {
            temperature: 0.2,
            num_ctx: 8192,
            format: None,
            timeout: Duration::from_secs(120),
        }
    }
}

fn unreachable_err(e: impl std::fmt::Display) -> AppError {
    AppError::msg(format!(
        "Không kết nối được Ollama. Hãy mở ứng dụng Ollama (biểu tượng 🦙 cạnh đồng hồ) rồi thử lại. ({e})"
    ))
}

impl Ollama {
    pub fn new(base: &str) -> Self {
        let base = if base.trim().is_empty() {
            DEFAULT_URL
        } else {
            base.trim()
        };
        Self {
            base: base.trim_end_matches('/').to_string(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    pub fn version(&self) -> AppResult<String> {
        let v: Value = net::local_agent(Some(Duration::from_secs(3)))
            .get(&self.url("/api/version"))
            .call()
            .map_err(unreachable_err)?
            .body_mut()
            .read_json()
            .map_err(unreachable_err)?;
        Ok(v["version"].as_str().unwrap_or("?").to_string())
    }

    pub fn models(&self) -> AppResult<Vec<OllamaModel>> {
        #[derive(Deserialize)]
        struct Tags {
            models: Vec<OllamaModel>,
        }
        let t: Tags = net::local_agent(Some(Duration::from_secs(5)))
            .get(&self.url("/api/tags"))
            .call()
            .map_err(unreachable_err)?
            .body_mut()
            .read_json()
            .map_err(unreachable_err)?;
        Ok(t.models)
    }

    /// Tải model (`ollama pull`), báo tiến độ (đã tải, tổng) theo từng phần.
    pub fn pull(
        &self,
        model: &str,
        cancel: &AtomicBool,
        progress: &dyn Fn(&str, u64, u64),
    ) -> AppResult<()> {
        let mut resp = net::local_agent(None)
            .post(&self.url("/api/pull"))
            .send_json(json!({ "model": model, "stream": true }))
            .map_err(unreachable_err)?;
        let reader = BufReader::new(resp.body_mut().as_reader());
        for line in reader.lines() {
            if cancel.load(Ordering::Relaxed) {
                return Err(AppError::msg("Đã huỷ tải"));
            }
            let line = line.map_err(|e| AppError::msg(format!("Mất kết nối với Ollama: {e}")))?;
            let Ok(v) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if let Some(err) = v["error"].as_str() {
                return Err(AppError::msg(format!("Ollama báo lỗi: {err}")));
            }
            let status = v["status"].as_str().unwrap_or("");
            progress(
                status,
                v["completed"].as_u64().unwrap_or(0),
                v["total"].as_u64().unwrap_or(0),
            );
            if status == "success" {
                return Ok(());
            }
        }
        Ok(())
    }

    pub fn chat(&self, model: &str, messages: &[Message], opts: &ChatOptions) -> AppResult<String> {
        let mut body = json!({
            "model": model,
            "messages": messages,
            "stream": false,
            "think": false,
            "keep_alive": "30m",
            "options": { "temperature": opts.temperature, "num_ctx": opts.num_ctx },
        });
        if let Some(f) = &opts.format {
            body["format"] = f.clone();
        }
        let agent = net::local_agent(Some(opts.timeout));
        let mut result = agent
            .post(&self.url("/api/chat"))
            .config()
            .http_status_as_error(false)
            .build()
            .send_json(&body);
        // Model không hỗ trợ tham số "think" → gửi lại không có nó.
        if let Ok(r) = result.as_mut() {
            if r.status().as_u16() == 400 {
                let text = r.body_mut().read_to_string().unwrap_or_default();
                if text.contains("think") {
                    body.as_object_mut().unwrap().remove("think");
                    result = agent
                        .post(&self.url("/api/chat"))
                        .config()
                        .http_status_as_error(false)
                        .build()
                        .send_json(&body);
                } else {
                    return Err(AppError::msg(format!("Ollama báo lỗi: {text}")));
                }
            }
        }
        let mut resp = result.map_err(unreachable_err)?;
        let status = resp.status().as_u16();
        let v: Value = resp
            .body_mut()
            .read_json()
            .map_err(|e| AppError::msg(format!("Ollama trả về dữ liệu lạ: {e}")))?;
        if status >= 400 {
            let err = v["error"].as_str().unwrap_or("không rõ");
            if err.contains("not found") {
                return Err(AppError::msg(format!(
                    "Chưa có model \"{model}\" trong Ollama. Vào Cài đặt → Ollama để tải."
                )));
            }
            return Err(AppError::msg(format!("Ollama báo lỗi: {err}")));
        }
        Ok(strip_think(v["message"]["content"].as_str().unwrap_or(""))
            .trim()
            .to_string())
    }
}

/// Bỏ đoạn suy nghĩ <think>...</think> nếu model vẫn trả về.
pub fn strip_think(s: &str) -> String {
    let mut out = s.to_string();
    while let (Some(a), Some(b)) = (out.find("<think>"), out.find("</think>")) {
        if b < a {
            break;
        }
        out.replace_range(a..b + "</think>".len(), "");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_think_blocks() {
        assert_eq!(
            strip_think("<think>abc</think>\nXin chào").trim(),
            "Xin chào"
        );
        assert_eq!(strip_think("Không có"), "Không có");
    }
}
