//! Kiểm tra cấu hình máy: GPU, RAM, CPU.

use std::process::Command;

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub name: String,
    /// GB, None nếu không đọc được.
    pub vram_gb: Option<f32>,
    pub nvidia: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hardware {
    pub os: String,
    pub cpu: String,
    pub cores: usize,
    pub ram_gb: f32,
    pub gpus: Vec<GpuInfo>,
}

fn command(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: không nháy cửa sổ console
    }
    c
}

fn nvidia_gpus() -> Vec<GpuInfo> {
    let Ok(out) = command("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let mut it = l.split(',').map(str::trim);
            let name = it.next()?.to_string();
            let mb: f32 = it.next()?.parse().ok()?;
            Some(GpuInfo {
                name,
                vram_gb: Some(mb / 1024.0),
                nvidia: true,
            })
        })
        .collect()
}

#[cfg(windows)]
fn other_gpus() -> Vec<GpuInfo> {
    let Ok(out) = command("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "(Get-CimInstance Win32_VideoController).Name",
        ])
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|name| GpuInfo {
            name: name.to_string(),
            vram_gb: None,
            nvidia: name.to_lowercase().contains("nvidia"),
        })
        .collect()
}

#[cfg(not(windows))]
fn other_gpus() -> Vec<GpuInfo> {
    Vec::new()
}

pub fn hardware() -> Hardware {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.refresh_cpu_list(sysinfo::CpuRefreshKind::nothing());
    let cpu = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .unwrap_or_default();
    let mut gpus = nvidia_gpus();
    if gpus.is_empty() {
        gpus = other_gpus();
    }
    Hardware {
        os: sysinfo::System::long_os_version().unwrap_or_else(|| std::env::consts::OS.into()),
        cpu,
        cores: std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1),
        ram_gb: sys.total_memory() as f32 / 1024.0 / 1024.0 / 1024.0,
        gpus,
    }
}
