//! 속도 측정에 쓰는 순수 함수, 시계나 파일을 읽지 않고 값만 받아 계산한다.

use crate::SAMPLE_RATE_HZ;
use seamone_core::sample_count_as_f64;

/// 표본 수에서 음성 길이(초)를 구한다.
#[must_use]
pub fn audio_secs(sample_count: usize) -> f64 {
    sample_count_as_f64(sample_count as u64) / f64::from(SAMPLE_RATE_HZ)
}

/// 실시간 배수, 음성 길이를 처리 시간으로 나눈 값이다(1 이면 실시간, 3 이면 실시간의 3배 빠르다).
/// 음성 길이가 음수이거나 처리 시간이 0 이하이거나 숫자가 아니면 `None` 이다.
#[must_use]
pub fn realtime_factor(audio_secs: f64, processing_secs: f64) -> Option<f64> {
    let valid = audio_secs.is_finite()
        && processing_secs.is_finite()
        && audio_secs >= 0.0
        && processing_secs > 0.0;
    valid.then(|| audio_secs / processing_secs)
}

/// 가운데값, 개수가 짝수이면 가운데 두 값의 평균이다, 값이 없거나 숫자가 아닌 값이 하나라도 있으면 `None` 이다.
#[must_use]
pub fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() || values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        Some(sorted[middle])
    } else {
        Some(f64::midpoint(sorted[middle - 1], sorted[middle]))
    }
}

#[cfg(test)]
mod tests {
    use super::{audio_secs, median, realtime_factor};

    fn assert_close(actual: Option<f64>, expected: f64) {
        let value = actual.expect("값이 있다");
        assert!(
            (value - expected).abs() < 1e-9,
            "기대 {expected}, 실제 {value}"
        );
    }

    #[test]
    fn audio_secs_divides_the_sample_count_by_the_sample_rate() {
        assert_close(Some(audio_secs(0)), 0.0);
        assert_close(Some(audio_secs(16_000)), 1.0);
        assert_close(Some(audio_secs(8_000)), 0.5);
        assert_close(Some(audio_secs(160_000)), 10.0);
    }

    #[test]
    fn audio_duration_preserves_rounding_across_machine_word_boundaries() {
        for (count, bits) in [
            (0_u64, 0_u64),
            (16_000, 0x3ff0_0000_0000_0000),
            (4_294_967_295, 0x4110_624d_d2e1_47ae),
            (4_294_967_296, 0x4110_624d_d2f1_a9fc),
            (9_007_199_254_740_993, 0x4260_624d_d2f1_a9fc),
            (u64::MAX, 0x4310_624d_d2f1_a9fc),
        ] {
            if let Ok(count) = usize::try_from(count) {
                assert_eq!(audio_secs(count).to_bits(), bits);
            }
        }
    }

    #[test]
    fn realtime_factor_is_audio_length_over_processing_time() {
        assert_close(realtime_factor(10.0, 2.0), 5.0);
        assert_close(realtime_factor(10.0, 10.0), 1.0);
        assert_close(realtime_factor(10.0, 40.0), 0.25);
        assert_close(realtime_factor(0.0, 1.0), 0.0);
    }

    #[test]
    fn realtime_factor_rejects_values_that_have_no_meaning() {
        assert_eq!(realtime_factor(10.0, 0.0), None);
        assert_eq!(realtime_factor(10.0, -1.0), None);
        assert_eq!(realtime_factor(-1.0, 1.0), None);
        assert_eq!(realtime_factor(f64::NAN, 1.0), None);
        assert_eq!(realtime_factor(10.0, f64::NAN), None);
        assert_eq!(realtime_factor(f64::INFINITY, 1.0), None);
        assert_eq!(realtime_factor(10.0, f64::INFINITY), None);
    }

    #[test]
    fn median_of_an_odd_count_is_the_middle_value() {
        assert_close(median(&[3.0, 1.0, 2.0]), 2.0);
        assert_close(median(&[5.0]), 5.0);
    }

    #[test]
    fn median_of_an_even_count_is_the_mean_of_the_two_middle_values() {
        assert_close(median(&[4.0, 1.0, 3.0, 2.0]), 2.5);
        assert_close(median(&[1.0, 3.0]), 2.0);
    }

    #[test]
    fn median_of_three_timed_runs_ignores_the_slowest_outlier() {
        assert_close(median(&[2.9, 3.1, 0.4]), 2.9);
    }

    #[test]
    fn median_has_no_value_for_empty_or_non_finite_input() {
        assert_eq!(median(&[]), None);
        assert_eq!(median(&[1.0, f64::NAN, 3.0]), None);
        assert_eq!(median(&[1.0, f64::INFINITY]), None);
    }

    #[test]
    fn median_does_not_depend_on_the_input_order() {
        assert_close(median(&[1.0, 2.0, 3.0, 4.0, 5.0]), 3.0);
        assert_close(median(&[5.0, 3.0, 1.0, 4.0, 2.0]), 3.0);
    }
}
