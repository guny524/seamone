//! Streaming mono resampling to the library's fixed PCM16 rate.
use seamone_core::{capture::MAX_RECORDING_SECONDS, review::SAMPLE_RATE};
use std::{collections::VecDeque, f64::consts::PI};

const PHASES: usize = 128;
pub const LENGTH_ERROR: &str = "음성 길이는 30분 이하여야 합니다.";
pub const EMPTY_ERROR: &str = "저장할 음성이 없습니다.";

pub fn check_input(rate: u32, channels: usize) -> Result<(), String> {
    if !(8_000..=192_000).contains(&rate) || !(1..=32).contains(&channels) {
        return Err(format!(
            "지원하지 않는 입력 설정입니다. {rate}Hz, {channels}채널"
        ));
    }
    Ok(())
}

pub fn finite(sample: f32) -> f32 {
    if sample.is_finite() { sample } else { 0.0 }
}

#[allow(clippy::cast_possible_truncation)]
pub fn channel_average(sum: f64, channels: u16) -> f32 {
    (sum / f64::from(channels)) as f32
}

#[allow(clippy::cast_possible_truncation)]
fn pcm16(sample: f64) -> i16 {
    (sample * 32_768.0).round().clamp(-32_768.0, 32_767.0) as i16
}

pub struct Normalizer {
    rate: u32,
    frames: u64,
    output: u64,
    filter: Option<LowPass>,
}

impl Normalizer {
    pub fn new(rate: u32, channels: usize) -> Result<Self, String> {
        check_input(rate, channels)?;
        Ok(Self {
            rate,
            frames: 0,
            output: 0,
            filter: (u64::from(rate) != SAMPLE_RATE).then(|| LowPass::new(rate)),
        })
    }

    pub fn push(
        &mut self,
        mono: f32,
        mut write: impl FnMut(i16) -> Result<(), String>,
    ) -> Result<(), String> {
        if self.frames >= u64::from(self.rate) * u64::from(MAX_RECORDING_SECONDS) {
            return Err(LENGTH_ERROR.into());
        }
        self.frames += 1;
        let mono = finite(mono);
        if let Some(filter) = &mut self.filter {
            filter.input.push_back(mono);
            while self.output * u64::from(self.rate) / SAMPLE_RATE + filter.radius < self.frames {
                write(filter.sample(self.output, self.rate))?;
                self.output += 1;
            }
            filter.discard_before(
                (self.output * u64::from(self.rate) / SAMPLE_RATE).saturating_sub(filter.radius),
            );
        } else {
            write(pcm16(f64::from(mono)))?;
            self.output += 1;
        }
        Ok(())
    }

    pub fn finish(
        mut self,
        mut write: impl FnMut(i16) -> Result<(), String>,
    ) -> Result<(), String> {
        if self.frames == 0 {
            return Err(EMPTY_ERROR.into());
        }
        let rate = u64::from(self.rate);
        let samples = ((self.frames * SAMPLE_RATE + rate / 2) / rate).max(1);
        if let Some(filter) = &mut self.filter {
            while self.output < samples {
                write(filter.sample(self.output, self.rate))?;
                self.output += 1;
            }
        }
        Ok(())
    }
}

struct LowPass {
    radius: u64,
    weights: Vec<Vec<f32>>,
    input: VecDeque<f32>,
    first: u64,
}

impl LowPass {
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    fn new(rate: u32) -> Self {
        let ratio = SAMPLE_RATE as f64 / f64::from(rate);
        // Blackman-windowed sinc. Widen the kernel at higher input rates so the
        // transition below the 8 kHz output Nyquist frequency stays narrow.
        let radius = (24.0 / ratio.min(1.0)).ceil() as i32;
        let cutoff = 0.44 * ratio.min(1.0);
        let weights = (0..PHASES)
            .map(|phase| {
                let fraction = phase as f64 / PHASES as f64;
                let weights: Vec<f64> = (-radius..=radius)
                    .map(|tap| {
                        let distance = f64::from(tap) - fraction;
                        let window = PI * distance / f64::from(radius);
                        if window.abs() >= PI {
                            return 0.0;
                        }
                        let sinc = if distance == 0.0 {
                            2.0 * cutoff
                        } else {
                            (2.0 * PI * cutoff * distance).sin() / (PI * distance)
                        };
                        sinc * (0.42 + 0.5 * window.cos() + 0.08 * (2.0 * window).cos())
                    })
                    .collect();
                let gain: f64 = weights.iter().sum();
                weights
                    .into_iter()
                    .map(|weight| (weight / gain) as f32)
                    .collect()
            })
            .collect();
        let radius = u64::try_from(radius).expect("positive filter radius");
        Self {
            radius,
            weights,
            input: VecDeque::with_capacity(usize::try_from(2 * radius + 4).unwrap()),
            first: 0,
        }
    }

    fn sample(&self, output: u64, rate: u32) -> i16 {
        let position = output * u64::from(rate);
        let center = position / SAMPLE_RATE;
        let phase = usize::try_from(position % SAMPLE_RATE).unwrap() * PHASES
            / usize::try_from(SAMPLE_RATE).unwrap();
        let sample = self.weights[phase]
            .iter()
            .enumerate()
            .filter_map(|(tap, weight)| {
                let index = (center + tap as u64).checked_sub(self.radius + self.first)?;
                self.input
                    .get(usize::try_from(index).ok()?)
                    .map(|&value| f64::from(value) * f64::from(*weight))
            })
            .sum();
        pcm16(sample)
    }

    fn discard_before(&mut self, first: u64) {
        while self.first < first && self.input.pop_front().is_some() {
            self.first += 1;
        }
    }
}
