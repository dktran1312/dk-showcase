//! Thu micro bằng `cpal` (WASAPI trên Windows, Core Audio trên macOS, ALSA trên Linux).

use std::thread::JoinHandle;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use crossbeam_channel::{bounded, Sender};

use super::{
    AudioFormat, AudioSource, CaptureHandle, ErrorFn, FrameSink, InputDevice, SourceKind, Track,
};
use crate::error::{AppError, AppResult};

/// Liệt kê các micro / thiết bị thu.
pub fn list_input_devices() -> AppResult<Vec<InputDevice>> {
    let host = cpal::default_host();
    let default_name = host.default_input_device().and_then(|d| d.name().ok());
    let devices = host
        .input_devices()
        .map_err(|e| AppError::Audio(format!("Không liệt kê được thiết bị thu: {e}")))?;
    let mut out: Vec<InputDevice> = devices
        .filter_map(|d| d.name().ok())
        .map(|name| InputDevice {
            is_default: Some(&name) == default_name.as_ref(),
            name,
        })
        .collect();
    out.dedup_by(|a, b| a.name == b.name);
    Ok(out)
}

fn find_input_device(name: Option<&str>) -> AppResult<cpal::Device> {
    let host = cpal::default_host();
    if let Some(wanted) = name.filter(|n| !n.is_empty()) {
        let found = host
            .input_devices()
            .map_err(|e| AppError::Audio(e.to_string()))?
            .find(|d| d.name().map(|n| n == wanted).unwrap_or(false));
        if let Some(d) = found {
            return Ok(d);
        }
        log::warn!("Không thấy micro '{wanted}', dùng micro mặc định");
    }
    host.default_input_device()
        .ok_or_else(|| AppError::Audio("Không tìm thấy micro nào. Hãy cắm micro hoặc kiểm tra quyền truy cập micro của Windows.".into()))
}

pub struct MicSource {
    device_name: Option<String>,
    format: AudioFormat,
}

impl MicSource {
    /// Mở micro theo tên (None = mặc định) và đọc định dạng gốc của thiết bị.
    pub fn open(device_name: Option<String>) -> AppResult<Self> {
        let device = find_input_device(device_name.as_deref())?;
        let cfg = device
            .default_input_config()
            .map_err(|e| AppError::Audio(format!("Không đọc được cấu hình micro: {e}")))?;
        Ok(Self {
            device_name,
            format: AudioFormat {
                sample_rate: cfg.sample_rate().0,
                channels: cfg.channels(),
            },
        })
    }
}

impl AudioSource for MicSource {
    fn kind(&self) -> SourceKind {
        SourceKind::Mic {
            device: self.device_name.clone(),
        }
    }
    fn track(&self) -> Track {
        Track::Mic
    }
    fn format(&self) -> AudioFormat {
        self.format
    }

    fn start(
        self: Box<Self>,
        sink: FrameSink,
        on_error: ErrorFn,
    ) -> AppResult<Box<dyn CaptureHandle>> {
        // `cpal::Stream` không `Send` trên một số nền tảng → giữ stream trong thread riêng.
        let (ready_tx, ready_rx) = bounded::<Result<(), String>>(1);
        let (stop_tx, stop_rx) = bounded::<()>(1);
        let expected = self.format;
        let device_name = self.device_name.clone();

        let thread = std::thread::Builder::new()
            .name("mic-capture".into())
            .spawn(move || {
                let built = (|| -> Result<cpal::Stream, String> {
                    let device =
                        find_input_device(device_name.as_deref()).map_err(|e| e.to_string())?;
                    let supported = device.default_input_config().map_err(|e| e.to_string())?;
                    let config: cpal::StreamConfig = supported.config();
                    if config.sample_rate.0 != expected.sample_rate
                        || config.channels != expected.channels
                    {
                        return Err("Cấu hình micro vừa thay đổi, hãy thử lại.".into());
                    }
                    let stream = match supported.sample_format() {
                        SampleFormat::F32 => build::<f32>(&device, &config, sink, on_error),
                        SampleFormat::I16 => build::<i16>(&device, &config, sink, on_error),
                        SampleFormat::U16 => build::<u16>(&device, &config, sink, on_error),
                        SampleFormat::I32 => build::<i32>(&device, &config, sink, on_error),
                        SampleFormat::U8 => build::<u8>(&device, &config, sink, on_error),
                        other => return Err(format!("Định dạng mẫu {other:?} chưa được hỗ trợ")),
                    }?;
                    stream.play().map_err(|e| e.to_string())?;
                    Ok(stream)
                })();

                match built {
                    Ok(stream) => {
                        let _ = ready_tx.send(Ok(()));
                        let _ = stop_rx.recv(); // chờ lệnh dừng (hoặc handle bị drop)
                        drop(stream); // drop stream → drop sink → phía ghi file biết là hết dữ liệu
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                    }
                }
            })?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Box::new(MicHandle {
                stop_tx: Some(stop_tx),
                thread: Some(thread),
            })),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(AppError::Audio(format!("Không mở được micro: {e}")))
            }
            Err(_) => Err(AppError::Audio("Thread thu micro dừng bất thường".into())),
        }
    }
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sink: Sender<Vec<f32>>,
    on_error: ErrorFn,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                // Callback realtime: chỉ chuyển đổi và đẩy đi, không làm gì nặng ở đây.
                let buf: Vec<f32> = data.iter().map(|&s| s.to_sample::<f32>()).collect();
                let _ = sink.send(buf);
            },
            move |err| on_error(format!("Micro gặp lỗi: {err}")),
            None,
        )
        .map_err(|e| e.to_string())
}

struct MicHandle {
    stop_tx: Option<Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl CaptureHandle for MicHandle {
    fn stop(mut self: Box<Self>) {
        self.shutdown();
    }
}

impl MicHandle {
    fn shutdown(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for MicHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}
