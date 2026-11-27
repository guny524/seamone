//! An explicit test clock drives the same renderer as the native device.

use super::{
    engine::{self, Driver, Running},
    renderer::{Format, Renderer, Timing},
};
use std::sync::{Arc, atomic::AtomicBool};

pub(super) struct Device;

struct Clock {
    renderer: Renderer,
    frames: u64,
    sequence: u64,
}

impl engine::Device for Device {
    fn start(&self, source: Arc<[f32]>, at: u64) -> Result<Running, String> {
        let fault = Arc::new(AtomicBool::new(false));
        let (renderer, reader, commands) = Renderer::new(
            source,
            Format {
                rate: 48_000,
                channels: 2,
            },
            at,
            fault,
        )?;
        Ok(Running {
            reader,
            commands,
            driver: Box::new(Clock {
                renderer,
                frames: 0,
                sequence: 0,
            }),
        })
    }
}

impl Driver for Clock {
    fn stop(&mut self) {}

    fn advance(&mut self, mut frames: usize) -> Result<(), String> {
        let mut output = [0.0_f32; 1_024];
        while frames > 0 {
            let count = frames.min(512);
            self.sequence += 1;
            let time = self.frames * 1_000_000_000 / 48_000 + self.sequence;
            self.renderer.render(
                &mut output[..count * 2],
                super::clock::timing(
                    Some(std::time::Duration::from_nanos(time)),
                    Some(std::time::Duration::from_nanos(time + 20_000_000)),
                    Some(std::time::Duration::ZERO),
                )
                .unwrap_or(Timing {
                    callback: 1,
                    playback: 0,
                }),
                |sample| sample,
            );
            self.frames += u64::try_from(count).map_err(|e| e.to_string())?;
            frames -= count;
        }
        Ok(())
    }
}
