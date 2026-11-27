//! Checked timestamp conversion shared by native callbacks and deterministic device tests.

use super::renderer::Timing;
use std::time::Duration;

pub(super) fn timing(
    callback: Option<Duration>,
    playback: Option<Duration>,
    latency: Option<Duration>,
) -> Option<Timing> {
    let callback = callback?;
    let playback = playback?;
    if playback < callback {
        return None;
    }
    Some(Timing {
        callback: callback.as_nanos().try_into().ok()?,
        playback: playback.checked_add(latency?)?.as_nanos().try_into().ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_latency_delays_playback_without_moving_the_callback_clock() {
        let stamp = timing(
            Some(Duration::from_millis(20)),
            Some(Duration::from_millis(40)),
            Some(Duration::from_millis(250)),
        )
        .unwrap();
        assert_eq!(stamp.callback, 20_000_000);
        assert_eq!(stamp.playback, 290_000_000);
    }

    #[test]
    fn equal_timestamps_are_valid_but_missing_reversed_and_overflowing_times_are_not() {
        let zero = Some(Duration::ZERO);
        assert!(timing(zero, zero, zero).is_some());
        for (callback, playback, latency) in [
            (None, zero, zero),
            (zero, None, zero),
            (zero, zero, None),
            (
                Some(Duration::from_nanos(1)),
                zero,
                Some(Duration::from_secs(1)),
            ),
            (Some(Duration::MAX), Some(Duration::MAX), zero),
            (
                zero,
                Some(Duration::from_nanos(u64::MAX)),
                Some(Duration::from_nanos(1)),
            ),
            (zero, Some(Duration::MAX), Some(Duration::from_nanos(1))),
        ] {
            assert!(timing(callback, playback, latency).is_none());
        }
    }
}
