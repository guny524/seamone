//! Streaming mono resampling to the library's fixed PCM16 rate.
use num_traits::ToPrimitive;
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

pub fn channel_average(sum: f64, channels: u16) -> f32 {
    (sum / f64::from(channels))
        .to_f32()
        .expect("channel average converts to f32")
}

struct Pcm16(i16);

impl Pcm16 {
    fn quantize(sample: f64) -> Self {
        let rounded = (sample * 32_768.0)
            .round()
            .clamp(f64::from(i16::MIN), f64::from(i16::MAX));
        Self(rounded.to_i16().unwrap_or_default())
    }
}

fn pcm16(sample: f64) -> i16 {
    Pcm16::quantize(sample).0
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

struct KernelRadius(u16);

impl KernelRadius {
    fn from_ratio(ratio: f64) -> Self {
        Self(
            (24.0 / ratio.min(1.0))
                .ceil()
                .to_u16()
                .expect("validated input rate bounds the filter radius"),
        )
    }
}

struct LowPass {
    radius: u64,
    weights: Vec<Vec<f32>>,
    input: VecDeque<f32>,
    first: u64,
}

impl LowPass {
    fn new(rate: u32) -> Self {
        let ratio =
            f64::from(u32::try_from(SAMPLE_RATE).expect("fixed PCM16 rate")) / f64::from(rate);

        let radius = KernelRadius::from_ratio(ratio);
        let cutoff = 0.44 * ratio.min(1.0);
        let weights = (0..PHASES)
            .map(|phase| {
                let fraction = f64::from(u16::try_from(phase).expect("bounded filter phase"))
                    / f64::from(u16::try_from(PHASES).expect("bounded filter phase count"));
                let weights: Vec<f64> = (-i32::from(radius.0)..=i32::from(radius.0))
                    .map(|tap| {
                        let distance = f64::from(tap) - fraction;
                        let window = PI * distance / f64::from(radius.0);
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
                    .map(|weight| {
                        (weight / gain)
                            .to_f32()
                            .expect("normalized filter coefficient")
                    })
                    .collect()
            })
            .collect();
        let radius = u64::from(radius.0);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampling_preserves_pcm16_samples_at_supported_rate_extremes_and_common_rates() {
        for (rate, length, expected) in [
            (8_000, 4_096, 16_018_315_236_159_049_558),
            (16_000, 2_048, 17_848_847_840_009_084_928),
            (44_100, 743, 11_427_453_305_392_998_375),
            (48_000, 683, 6_038_885_649_343_245_240),
            (192_000, 171, 1_644_297_260_069_288_869),
        ] {
            let mut normalizer = Normalizer::new(rate, 1).unwrap();
            let mut output = Vec::new();
            for index in 0..2_048_u16 {
                let sample = (f32::from(index % 257) - 128.0) / 100.0;
                normalizer
                    .push(sample, |sample| {
                        output.push(sample);
                        Ok(())
                    })
                    .unwrap();
            }
            normalizer
                .finish(|sample| {
                    output.push(sample);
                    Ok(())
                })
                .unwrap();
            let digest = output.iter().fold(0_u64, |digest, sample| {
                digest
                    .wrapping_mul(31)
                    .wrapping_add(u64::from(u16::from_le_bytes(sample.to_le_bytes())))
            });
            assert_eq!(output.len(), length, "{rate} Hz");
            assert_eq!(digest, expected, "{rate} Hz");
        }
    }

    #[test]
    fn mixing_preserves_single_precision_rounding() {
        for (sum, channels, bits) in [
            (1.0, 3, 1_051_372_203),
            (-4.5, 32, 3_188_719_616),
            (1e30, 2, 1_892_283_082),
        ] {
            assert_eq!(channel_average(sum, channels).to_bits(), bits);
        }
    }

    #[test]
    fn pcm16_rounds_half_steps_and_saturates_including_non_finite_input() {
        for (input, expected) in [
            (-1.1, i16::MIN),
            (-1.0, i16::MIN),
            (-1.5 / 32_768.0, -2),
            (-0.5 / 32_768.0, -1),
            (0.0, 0),
            (0.5 / 32_768.0, 1),
            (1.5 / 32_768.0, 2),
            (1.0, i16::MAX),
            (1.1, i16::MAX),
            (f64::NAN, 0),
            (f64::INFINITY, i16::MAX),
            (f64::NEG_INFINITY, i16::MIN),
        ] {
            assert_eq!(pcm16(input), expected);
        }
    }
}
