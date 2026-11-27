//! Streaming mono resampling to the library's fixed PCM16 rate.
use num_traits::ToPrimitive;
use seamone_core::{capture::MAX_RECORDING_SECONDS, review::SAMPLE_RATE};
use std::{collections::VecDeque, f64::consts::PI};

const PHASES: u16 = 128;
pub const LENGTH_ERROR: &str = "음성 길이는 30분 이하여야 합니다.";
pub const EMPTY_ERROR: &str = "저장할 음성이 없습니다.";

#[derive(Clone, Copy)]
pub struct InputRate {
    hz: u32,
    ratio: f64,
    radius: u16,
}

impl InputRate {
    pub fn hz(self) -> u32 {
        self.hz
    }
}

pub fn check_input(rate: u32, channels: usize) -> Result<InputRate, String> {
    let invalid = || format!("지원하지 않는 입력 설정입니다. {rate}Hz, {channels}채널");
    if !(8_000..=192_000).contains(&rate) || !(1..=32).contains(&channels) {
        return Err(invalid());
    }
    let output_rate = u32::try_from(SAMPLE_RATE).map_err(|_| invalid())?;
    let radius = u16::try_from((24 * rate).div_ceil(output_rate).max(24)).map_err(|_| invalid())?;
    Ok(InputRate {
        hz: rate,
        ratio: f64::from(output_rate) / f64::from(rate),
        radius,
    })
}

pub fn finite(sample: f64) -> f64 {
    if sample.is_finite() { sample } else { 0.0 }
}

pub fn channel_average(sum: f64, channels: u16) -> f64 {
    sum / f64::from(channels)
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
    rate: InputRate,
    frames: u64,
    output: u64,
    filter: Option<LowPass>,
}

impl Normalizer {
    pub fn new(rate: InputRate) -> Self {
        let filter = (u64::from(rate.hz()) != SAMPLE_RATE).then(|| LowPass::new(&rate));
        Self {
            rate,
            frames: 0,
            output: 0,
            filter,
        }
    }

    pub fn push(
        &mut self,
        mono: f64,
        mut write: impl FnMut(i16) -> Result<(), String>,
    ) -> Result<(), String> {
        if self.frames >= u64::from(self.rate.hz()) * u64::from(MAX_RECORDING_SECONDS) {
            return Err(LENGTH_ERROR.into());
        }
        self.frames += 1;
        let mono = finite(mono);
        if let Some(filter) = &mut self.filter {
            filter.input.push_back(mono);
            while self.output * u64::from(self.rate.hz()) / SAMPLE_RATE + filter.radius
                < self.frames
            {
                write(filter.sample(self.output, &self.rate))?;
                self.output += 1;
            }
            filter.discard_before(
                (self.output * u64::from(self.rate.hz()) / SAMPLE_RATE)
                    .saturating_sub(filter.radius),
            );
        } else {
            write(pcm16(mono))?;
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
        let rate = u64::from(self.rate.hz());
        let samples = ((self.frames * SAMPLE_RATE + rate / 2) / rate).max(1);
        if let Some(filter) = &mut self.filter {
            while self.output < samples {
                write(filter.sample(self.output, &self.rate))?;
                self.output += 1;
            }
        }
        Ok(())
    }
}

struct LowPass {
    radius: u64,
    weights: Vec<Vec<f64>>,
    input: VecDeque<f64>,
    first: u64,
}

impl LowPass {
    fn new(rate: &InputRate) -> Self {
        let radius = rate.radius;
        let cutoff = 0.44 * rate.ratio.min(1.0);
        let weights = (0..PHASES)
            .map(|phase| {
                let fraction = f64::from(phase) / f64::from(PHASES);
                let weights: Vec<f64> = (-i32::from(radius)..=i32::from(radius))
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
                weights.into_iter().map(|weight| weight / gain).collect()
            })
            .collect();
        let radius = u64::from(radius);
        Self {
            radius,
            weights,
            input: VecDeque::with_capacity(usize::from(2 * rate.radius + 4)),
            first: 0,
        }
    }

    fn sample(&self, output: u64, rate: &InputRate) -> i16 {
        let position = output * u64::from(rate.hz());
        let center = position / SAMPLE_RATE;
        let phase = usize::try_from(position % SAMPLE_RATE).unwrap() * usize::from(PHASES)
            / usize::try_from(SAMPLE_RATE).unwrap();
        let sample = self.weights[phase]
            .iter()
            .enumerate()
            .filter_map(|(tap, weight)| {
                let index = (center + tap as u64).checked_sub(self.radius + self.first)?;
                self.input
                    .get(usize::try_from(index).ok()?)
                    .map(|&value| value * weight)
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
    fn double_precision_pcm32_downmix_and_filter_quantize_once_to_pcm16() {
        for (rate, length, expected) in [
            (8_000, 131_072, 11_501_411_460_256_212_271),
            (16_000, 65_536, 7_750_520_179_904_512_307),
            (44_100, 23_777, 15_894_898_097_734_927_963),
            (48_000, 21_845, 1_029_984_672_103_818_542),
            (192_000, 5_461, 12_253_848_413_272_739_027),
        ] {
            let mut normalizer = Normalizer::new(check_input(rate, 2).unwrap());
            let mut state = 0x1234_5678_u32;
            let mut output = Vec::new();
            for _ in 0..65_536 {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let left = f64::from(i32::from_le_bytes(state.to_le_bytes())) / 2_147_483_648.0;
                let right = f64::from(i32::from_le_bytes(state.rotate_left(7).to_le_bytes()))
                    / 2_147_483_648.0;
                normalizer
                    .push(channel_average(left + right, 2), |sample| {
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
    fn resampling_preserves_pcm16_samples_at_supported_rate_extremes_and_common_rates() {
        for (rate, length, expected) in [
            (8_000, 4_096, 16_018_315_236_159_049_558),
            (16_000, 2_048, 17_848_847_840_009_084_928),
            (44_100, 743, 11_427_453_305_392_998_375),
            (48_000, 683, 6_038_885_649_343_245_240),
            (192_000, 171, 1_644_297_260_069_288_869),
        ] {
            let mut normalizer = Normalizer::new(check_input(rate, 1).unwrap());
            let mut output = Vec::new();
            for index in 0..2_048_u16 {
                let sample = (f64::from(index % 257) - 128.0) / 100.0;
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
    fn mixing_preserves_double_precision_until_pcm16_quantization() {
        assert_eq!(channel_average(1.0, 3), 1.0 / 3.0);
        assert_eq!(channel_average(-4.5, 32), -0.140_625);
        let sample = (0.5 - f64::EPSILON) / 32_768.0;
        assert_eq!(pcm16(channel_average(sample * 3.0, 3)), 0);
    }

    #[test]
    fn input_rate_rejects_invalid_settings_before_filter_construction() {
        for rate in [0, 7_999, 192_001, u32::MAX] {
            assert!(check_input(rate, 1).is_err());
        }
        for channels in [0, 33, usize::MAX] {
            assert!(check_input(48_000, channels).is_err());
        }
        for (hz, radius) in [(8_000, 24), (16_000, 24), (48_000, 72), (192_000, 288)] {
            let rate = check_input(hz, 32).unwrap();
            let filter = LowPass::new(&rate);
            assert_eq!(filter.radius, radius);
            assert!(
                filter
                    .weights
                    .iter()
                    .all(|phase| phase.len() == usize::from(2 * rate.radius + 1))
            );
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
