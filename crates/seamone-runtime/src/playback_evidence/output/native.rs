//! CPAL owns the device timestamps used by the shared output renderer.

use super::{
    engine::{self, Driver, Running},
    renderer::{Format, Renderer, Timing},
};
use cpal::{
    FromSample, SampleFormat, SizedSample, StreamConfig, StreamInstant,
    traits::{DeviceTrait, StreamTrait},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use self::macos as latency;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use self::windows as latency;

#[cfg(target_os = "linux")]
mod latency {
    use cpal::traits::HostTrait;
    use std::time::Duration;

    pub(super) fn open() -> Result<(cpal::Device, Duration), String> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("기본 출력 장치가 없습니다.")?;
        Ok((device, Duration::ZERO))
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn output_device() -> Result<cpal::Device, String> {
    use cpal::traits::HostTrait;
    let host = cpal::default_host();
    let name = host
        .default_output_device()
        .ok_or("기본 출력 장치가 없습니다.")?
        .name()
        .map_err(|error| error.to_string())?;
    let mut devices = host
        .output_devices()
        .map_err(|error| error.to_string())?
        .filter(|device| device.name().is_ok_and(|candidate| candidate == name));
    let device = devices.next().ok_or("출력 장치가 바뀌었습니다.")?;
    if devices.next().is_some() {
        return Err("출력 장치 이름이 중복되어 지연을 확인할 수 없습니다.".into());
    }
    Ok(device)
}

pub(super) struct Device;

struct Native {
    stop: mpsc::Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl Driver for Native {
    fn stop(&mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = self.stop.send(());
            let _ = thread.join();
        }
    }
}

impl engine::Device for Device {
    fn start(&self, source: Arc<[f32]>, at: u64) -> Result<Running, String> {
        let (ready, result) = mpsc::sync_channel(1);
        let (stop, stopped) = mpsc::channel();
        let thread = thread::spawn(move || match open(source, at) {
            Ok((stream, reader, commands)) => {
                if ready.send(Ok((reader, commands))).is_ok() {
                    let _ = stopped.recv();
                }
                drop(stream);
            }
            Err(error) => {
                let _ = ready.send(Err(error));
            }
        });
        match result.recv().map_err(|e| e.to_string())? {
            Ok((reader, commands)) => Ok(Running {
                reader,
                commands,
                driver: Box::new(Native {
                    stop,
                    thread: Some(thread),
                }),
            }),
            Err(error) => {
                let _ = thread.join();
                Err(error)
            }
        }
    }
}

fn open(
    source: Arc<[f32]>,
    at: u64,
) -> Result<
    (
        cpal::Stream,
        super::renderer::Reader,
        rtrb::Producer<seamone_core::playback::Action>,
    ),
    String,
> {
    let (device, latency) = latency::open()?;
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    let format = supported.sample_format();
    let config = supported.config();
    let fault = Arc::new(AtomicBool::new(false));
    let (renderer, reader, commands) = Renderer::new(
        source,
        Format {
            rate: config.sample_rate.0,
            channels: config.channels,
        },
        at,
        Arc::clone(&fault),
    )?;
    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, &config, renderer, fault, latency),
        SampleFormat::I16 => build::<i16>(&device, &config, renderer, fault, latency),
        SampleFormat::U16 => build::<u16>(&device, &config, renderer, fault, latency),
        SampleFormat::F64 => build::<f64>(&device, &config, renderer, fault, latency),
        SampleFormat::I8 => build::<i8>(&device, &config, renderer, fault, latency),
        SampleFormat::I32 => build::<i32>(&device, &config, renderer, fault, latency),
        SampleFormat::I64 => build::<i64>(&device, &config, renderer, fault, latency),
        SampleFormat::U8 => build::<u8>(&device, &config, renderer, fault, latency),
        SampleFormat::U32 => build::<u32>(&device, &config, renderer, fault, latency),
        SampleFormat::U64 => build::<u64>(&device, &config, renderer, fault, latency),
        _ => return Err("지원하지 않는 출력 장치 sample 형식입니다.".into()),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((stream, reader, commands))
}

fn build<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut renderer: Renderer,
    fault: Arc<AtomicBool>,
    latency: std::time::Duration,
) -> Result<cpal::Stream, String> {
    let mut origin: Option<StreamInstant> = None;
    device
        .build_output_stream(
            config,
            move |output: &mut [T], info| write(&mut renderer, &mut origin, output, info, latency),
            move |_| {
                fault.store(true, Ordering::Release);
            },
            None,
        )
        .map_err(|e| e.to_string())
}

fn write<T: SizedSample + FromSample<f32>>(
    renderer: &mut Renderer,
    origin: &mut Option<StreamInstant>,
    output: &mut [T],
    info: &cpal::OutputCallbackInfo,
    latency: std::time::Duration,
) {
    let stamp = info.timestamp();
    let start = *origin.get_or_insert(stamp.callback);
    let timing = super::clock::timing(
        stamp.callback.duration_since(&start),
        stamp.playback.duration_since(&start),
        Some(latency),
    );
    renderer.render(
        output,
        timing.unwrap_or(Timing {
            callback: 1,
            playback: 0,
        }),
        T::from_sample,
    );
}

#[cfg(test)]
mod tests {
    use super::super::Stream;
    use seamone_core::playback::{Action, State};
    use std::{
        thread,
        time::{Duration, Instant},
    };

    #[test]
    #[ignore = "requires an audible physical output device; run make playback-device-proof"]
    fn real_output_device_confirms_the_tone_with_its_clock() {
        let source = (0..16_000_u16)
            .map(|index| 0.05 * (std::f32::consts::TAU * f32::from(index % 40) / 40.0).sin())
            .collect::<Vec<_>>();
        let mut stream = Stream::create(source.into(), 0).unwrap();
        let (_, latency) = super::latency::open().unwrap();
        eprintln!("reported additional output latency: {latency:?}");
        let started = Instant::now();
        stream.control(Action::Play).unwrap();
        let deadline = started + Duration::from_secs(9) + latency;
        loop {
            let observed = stream.observation();
            assert_ne!(observed.state, State::Failed, "{:?}", observed.error);
            if observed.state == State::Finished {
                assert!(observed.heard);
                assert_eq!(observed.position, 16_000);
                assert!(
                    started.elapsed() >= Duration::from_secs(1) + latency,
                    "completion preceded the reported output latency"
                );
                break;
            }
            assert!(
                Instant::now() < deadline,
                "device output did not reach the tone's end"
            );
            thread::sleep(Duration::from_millis(10));
        }
        stream.control(Action::Stop).unwrap();
    }
}
