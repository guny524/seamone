//! CPAL owns the device timestamps used by the shared output renderer.

use super::{
    engine::{self, Driver, Running},
    renderer::{Format, Renderer, Timing},
};
use cpal::{
    FromSample, SampleFormat, SizedSample, StreamConfig, StreamInstant,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
};

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
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("기본 출력 장치가 없습니다.")?;
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
        SampleFormat::F32 => build::<f32>(&device, &config, renderer, fault),
        SampleFormat::I16 => build::<i16>(&device, &config, renderer, fault),
        SampleFormat::U16 => build::<u16>(&device, &config, renderer, fault),
        SampleFormat::F64 => build::<f64>(&device, &config, renderer, fault),
        SampleFormat::I8 => build::<i8>(&device, &config, renderer, fault),
        SampleFormat::I32 => build::<i32>(&device, &config, renderer, fault),
        SampleFormat::I64 => build::<i64>(&device, &config, renderer, fault),
        SampleFormat::U8 => build::<u8>(&device, &config, renderer, fault),
        SampleFormat::U32 => build::<u32>(&device, &config, renderer, fault),
        SampleFormat::U64 => build::<u64>(&device, &config, renderer, fault),
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
) -> Result<cpal::Stream, String> {
    let mut origin: Option<StreamInstant> = None;
    device
        .build_output_stream(
            config,
            move |output: &mut [T], info| write(&mut renderer, &mut origin, output, info),
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
) {
    let stamp = info.timestamp();
    let start = *origin.get_or_insert(stamp.callback);
    let timing = stamp
        .callback
        .duration_since(&start)
        .zip(stamp.playback.duration_since(&start))
        .and_then(|(callback, playback)| {
            Some(Timing {
                callback: callback.as_nanos().try_into().ok()?,
                playback: playback.as_nanos().try_into().ok()?,
            })
        });
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
        stream.control(Action::Play).unwrap();
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let observed = stream.observation();
            assert_ne!(observed.state, State::Failed, "{:?}", observed.error);
            if observed.state == State::Finished {
                assert!(observed.heard);
                assert_eq!(observed.position, 16_000);
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
