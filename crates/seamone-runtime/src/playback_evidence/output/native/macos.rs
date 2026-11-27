//! `CoreAudio` latency belongs to the pinned output device, in its nominal sample rate.

use coreaudio::{
    audio_unit::{Scope, macos_helpers},
    sys,
};
use cpal::traits::DeviceTrait;
use num_traits::ToPrimitive;
use std::{mem::size_of, ptr, time::Duration};

pub(super) fn open() -> Result<(cpal::Device, Duration), String> {
    let device = super::output_device()?;
    let name = device.name().map_err(|error| error.to_string())?;
    let ids = macos_helpers::get_audio_device_ids_for_scope(Scope::Output)
        .map_err(|error| error.to_string())?;
    let id = output_id(
        ids,
        &name,
        |id| {
            macos_helpers::get_audio_device_supports_scope(id, Scope::Output)
                .map_err(|error| error.to_string())
        },
        |id| macos_helpers::get_device_name(id).map_err(|error| error.to_string()),
    )?;
    Ok((device, read_latency(id)?))
}

fn output_id(
    ids: Vec<sys::AudioDeviceID>,
    name: &str,
    mut supports_output: impl FnMut(sys::AudioDeviceID) -> Result<bool, String>,
    mut device_name: impl FnMut(sys::AudioDeviceID) -> Result<String, String>,
) -> Result<sys::AudioDeviceID, String> {
    let mut selected = None;
    for id in ids {
        if !supports_output(id)? || device_name(id)? != name {
            continue;
        }
        if selected.replace(id).is_some() {
            return Err("출력 장치 이름이 중복되어 지연을 확인할 수 없습니다.".into());
        }
    }
    selected.ok_or_else(|| "출력 장치 지연을 읽을 수 없습니다.".into())
}

fn read_latency(device: sys::AudioDeviceID) -> Result<Duration, String> {
    // SAFETY: CoreAudio receives live property addresses, aligned initialized buffers and their exact byte lengths; returned sizes are checked before reading values.
    unsafe {
        let read = |object, selector, scope| -> Result<Vec<u32>, String> {
            let address = sys::AudioObjectPropertyAddress {
                mSelector: selector,
                mScope: scope,
                mElement: sys::kAudioObjectPropertyElementMaster,
            };
            let mut size = 0;
            let status = sys::AudioObjectGetPropertyDataSize(
                object,
                &raw const address,
                0,
                ptr::null(),
                &raw mut size,
            );
            if status != 0 || size == 0 || !size.is_multiple_of(4) {
                return Err(format!(
                    "CoreAudio 지연 속성 {selector} 크기를 읽지 못했습니다: {status}"
                ));
            }
            let mut values =
                vec![0_u32; usize::try_from(size / 4).map_err(|error| error.to_string())?];
            let capacity = size;
            let status = sys::AudioObjectGetPropertyData(
                object,
                &raw const address,
                0,
                ptr::null(),
                &raw mut size,
                values.as_mut_ptr().cast(),
            );
            if status != 0 || size != capacity {
                return Err(format!(
                    "CoreAudio 지연 속성 {selector}를 읽지 못했습니다: {status}"
                ));
            }
            Ok(values)
        };
        let output = sys::kAudioObjectPropertyScopeOutput;
        let scalar = |selector| -> Result<u64, String> {
            let values = read(device, selector, output)?;
            match values.as_slice() {
                [value] => Ok(u64::from(*value)),
                _ => Err("CoreAudio 지연 속성의 형식이 잘못됐습니다.".into()),
            }
        };
        let frames = scalar(sys::kAudioDevicePropertyLatency)?
            + scalar(sys::kAudioDevicePropertySafetyOffset)?;
        let streams = read(device, sys::kAudioDevicePropertyStreams, output)?;
        let stream_latency = streams
            .into_iter()
            .map(|stream| {
                let values = read(
                    stream,
                    sys::kAudioStreamPropertyLatency,
                    sys::kAudioObjectPropertyScopeGlobal,
                )?;
                match values.as_slice() {
                    [value] => Ok(u64::from(*value)),
                    _ => Err("CoreAudio stream 지연의 형식이 잘못됐습니다.".to_string()),
                }
            })
            .collect::<Result<Vec<_>, String>>()?
            .into_iter()
            .max()
            .ok_or("출력 stream 지연이 없습니다.")?;
        let address = sys::AudioObjectPropertyAddress {
            mSelector: sys::kAudioDevicePropertyNominalSampleRate,
            mScope: sys::kAudioObjectPropertyScopeGlobal,
            mElement: sys::kAudioObjectPropertyElementMaster,
        };
        let mut rate = 0.0_f64;
        let mut size = u32::try_from(size_of::<f64>()).map_err(|error| error.to_string())?;
        let status = sys::AudioObjectGetPropertyData(
            device,
            &raw const address,
            0,
            ptr::null(),
            &raw mut size,
            ptr::from_mut(&mut rate).cast(),
        );
        if status != 0
            || usize::try_from(size).ok() != Some(size_of::<f64>())
            || !rate.is_finite()
            || rate <= 0.0
        {
            return Err("CoreAudio 장치 sample rate를 읽지 못했습니다.".into());
        }
        let frames = (frames + stream_latency)
            .to_f64()
            .ok_or("출력 지연 범위가 잘못됐습니다.")?;
        let nanos = (frames * 1_000_000_000.0 / rate)
            .ceil()
            .to_u64()
            .ok_or("출력 지연 범위가 잘못됐습니다.")?;
        Ok(Duration::from_nanos(nanos))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_input_only_devices_do_not_hide_the_output_device() {
        let selected = output_id(
            vec![11, 22, 33],
            "headset",
            |id| Ok(id != 11),
            |id| {
                assert_ne!(id, 11, "input-only device names must not be queried");
                Ok(if id == 22 { "headset" } else { "speaker" }.into())
            },
        )
        .unwrap();
        assert_eq!(selected, 22);
    }

    #[test]
    fn duplicate_output_names_and_unknown_capabilities_are_rejected() {
        assert!(
            output_id(
                vec![11, 22],
                "headset",
                |_| Ok(true),
                |_| Ok("headset".into())
            )
            .is_err()
        );
        assert!(
            output_id(
                vec![11],
                "headset",
                |_| Err("unknown output scope".into()),
                |_| Ok("headset".into())
            )
            .is_err()
        );
        assert!(output_id(vec![11], "headset", |_| Ok(false), |_| Ok("headset".into())).is_err());
    }
}
