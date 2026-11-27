use base64::{Engine, engine::general_purpose::STANDARD};
use num_traits::ToPrimitive;
use seamone_core::review::SAMPLE_RATE;
use std::{f32::consts::TAU, io::Cursor};

const FFT_LENGTH: u16 = 512;
const FFT_SIZE: usize = FFT_LENGTH as usize;
pub const BINS: usize = FFT_SIZE / 2;
const HOP: usize = 128;
const MAX_COLUMNS: usize = 512;

pub struct Spectrum {
    pub columns: usize,
    pub rgba: Vec<u8>,
}

pub fn wav_spectrum(wav: &str, samples: u64) -> Result<Spectrum, String> {
    let invalid = || "스펙트로그램에 필요한 원본 PCM을 읽지 못했습니다.".to_owned();
    let bytes = STANDARD.decode(wav).map_err(|_| invalid())?;
    let mut reader = hound::WavReader::new(Cursor::new(bytes)).map_err(|_| invalid())?;
    let spec = reader.spec();
    if spec.channels != 1
        || u64::from(spec.sample_rate) != SAMPLE_RATE
        || spec.sample_format != hound::SampleFormat::Int
        || spec.bits_per_sample != 16
        || u64::from(reader.duration()) != samples
        || samples == 0
    {
        return Err(invalid());
    }
    let pcm = reader
        .samples::<i16>()
        .map(|sample| sample.map(|sample| f32::from(sample) / 32_768.0))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| invalid())?;
    Ok(stft(&pcm))
}

struct Intensity(f32);

impl Intensity {
    fn from_magnitude(magnitude: f32) -> Self {
        Self(((20.0 * magnitude.max(0.0001).log10() + 80.0) / 80.0).clamp(0.0, 1.0))
    }

    fn color(&self, base: u8, scale: u8) -> u8 {
        (f32::from(base) + self.0 * f32::from(scale))
            .to_u8()
            .unwrap_or_default()
    }
}

fn stft(samples: &[f32]) -> Spectrum {
    let columns = samples.len().div_ceil(HOP).clamp(1, MAX_COLUMNS);
    let mut rgba = vec![0; columns * BINS * 4];
    let mut window = [0.0; FFT_SIZE];
    for (index, weight) in (0..FFT_LENGTH).zip(&mut window) {
        *weight = 0.5 - 0.5 * (TAU * f32::from(index) / f32::from(FFT_LENGTH - 1)).cos();
    }
    for column in 0..columns {
        let center = (2 * column + 1) * samples.len() / (2 * columns);
        let mut bins: [(f32, f32); FFT_SIZE] = std::array::from_fn(|i| {
            let value = (center + i)
                .checked_sub(FFT_SIZE / 2)
                .and_then(|index| samples.get(index))
                .copied()
                .unwrap_or_default();
            (value * window[i], 0.0)
        });
        fft(&mut bins);
        for (bin, &(real, imaginary)) in bins.iter().take(BINS).enumerate() {
            let magnitude = real.hypot(imaginary) / (f32::from(FFT_LENGTH) / 4.0);
            let level = Intensity::from_magnitude(magnitude);
            let offset = ((BINS - 1 - bin) * columns + column) * 4;
            rgba[offset..offset + 4].copy_from_slice(&[
                level.color(18, 230),
                level.color(26, 208),
                level.color(39, 111),
                255,
            ]);
        }
    }
    Spectrum { columns, rgba }
}

fn fft(bins: &mut [(f32, f32); FFT_SIZE]) {
    let shift = usize::BITS - FFT_SIZE.trailing_zeros();
    for index in 0..FFT_SIZE {
        let reverse = index.reverse_bits() >> shift;
        if index < reverse {
            bins.swap(index, reverse);
        }
    }
    for stage in 1..=FFT_SIZE.trailing_zeros() {
        let length = 1_u16 << stage;
        let half = length / 2;
        for i in 0..half {
            let (sin, cos) = (-TAU * f32::from(i) / f32::from(length)).sin_cos();
            let i = usize::from(i);
            let half = usize::from(half);
            for start in (0..FFT_SIZE).step_by(usize::from(length)) {
                let (ar, ai) = bins[start + i];
                let (br, bi) = bins[start + i + half];
                let real = br * cos - bi * sin;
                let imaginary = br * sin + bi * cos;
                bins[start + i] = (ar + real, ai + imaginary);
                bins[start + i + half] = (ar - real, ai - imaginary);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impulse_spectra_preserve_color_bytes_for_short_and_long_windows() {
        for (length, columns, expected) in [
            (1, 1, 18_135_993_168_881_828_352),
            (512, 4, 7_219_218_557_376_742_144),
            (1_024, 8, 1_076_836_409_009_062_656),
            (16_000, 125, 878_296_205_277_272_064),
        ] {
            let mut samples = vec![0.0; length];
            samples[length / 2] = 1.0;
            let spectrum = stft(&samples);
            let digest = spectrum.rgba.iter().fold(0_u64, |digest, byte| {
                digest.wrapping_mul(31).wrapping_add(u64::from(*byte))
            });
            assert_eq!(spectrum.columns, columns);
            assert_eq!(digest, expected);
        }
    }

    #[test]
    fn wav_spectrum_locates_a_tone_in_its_sample_interval_and_frequency_bin() {
        let mut wav = Cursor::new(Vec::new());
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            sample_format: hound::SampleFormat::Int,
            bits_per_sample: 16,
        };
        let mut writer = hound::WavWriter::new(&mut wav, spec).unwrap();
        for i in 0..16_000 {
            let wave = [0_i16, 24_000, 0, -24_000];
            writer
                .write_sample(if i < 8_000 { 0 } else { wave[i % 4] })
                .unwrap();
        }
        writer.finalize().unwrap();
        let encoded = STANDARD.encode(wav.into_inner());
        let spectrum = wav_spectrum(&encoded, 16_000).unwrap();
        let intensity =
            |column, bin| spectrum.rgba[((BINS - 1 - bin) * spectrum.columns + column) * 4];
        assert_eq!(intensity(10, 128), 18);
        assert!(intensity(100, 128) > 200);
        assert_eq!(intensity(100, 32), 18);
        assert!(wav_spectrum(&encoded, 8_000).is_err());
    }
}
