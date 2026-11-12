//! HTTP client dùng chung (tải model, gọi Ollama).

use std::time::Duration;

use ureq::tls::{RootCerts, TlsConfig};
use ureq::Agent;

/// Agent cho Internet: dùng kho chứng chỉ của hệ điều hành (chạy được sau proxy công ty).
pub fn agent(timeout: Option<Duration>) -> Agent {
    let tls = TlsConfig::builder()
        .root_certs(RootCerts::PlatformVerifier)
        .build();
    Agent::new_with_config(
        Agent::config_builder()
            .tls_config(tls)
            .timeout_global(timeout)
            .build(),
    )
}

/// Agent cho dịch vụ chạy trên máy (Ollama): bỏ qua proxy.
pub fn local_agent(timeout: Option<Duration>) -> Agent {
    Agent::new_with_config(
        Agent::config_builder()
            .proxy(None)
            .timeout_global(timeout)
            .build(),
    )
}
