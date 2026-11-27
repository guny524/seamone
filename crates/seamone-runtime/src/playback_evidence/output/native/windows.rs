//! Query the default WASAPI endpoint using the same shared mix format as CPAL.

use ::windows::Win32::{
    Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
    Media::Audio::{
        AUDCLNT_SHAREMODE_SHARED, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator, eConsole,
        eRender,
    },
    System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree, STGM_READ},
};
use cpal::traits::DeviceTrait;
use std::time::Duration;

pub(super) fn open() -> Result<(cpal::Device, Duration), String> {
    let device = super::output_device()?;
    let name = device.name().map_err(|error| error.to_string())?;
    let latency = read_latency(&name)
        .map_err(|error| format!("WASAPI 출력 지연을 읽지 못했습니다: {error}"))?;
    Ok((device, latency))
}

fn read_latency(name: &str) -> Result<Duration, String> {
    // SAFETY: CPAL initializes COM on this thread; COM interfaces remain owned here, the mix-format allocation is freed after Initialize, and property values own their returned strings.
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(std::ptr::from_ref(&MMDeviceEnumerator), None, CLSCTX_ALL)
                .map_err(|error| error.to_string())?;
        let endpoint = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|error| error.to_string())?;
        let properties = endpoint
            .OpenPropertyStore(STGM_READ)
            .map_err(|error| error.to_string())?;
        let value = properties
            .GetValue(std::ptr::from_ref(&PKEY_Device_FriendlyName))
            .map_err(|error| error.to_string())?;
        let actual = ::windows::core::BSTR::try_from(&value)
            .map_err(|error| error.to_string())?
            .to_string();
        if actual != name {
            return Err("출력 장치가 바뀌었습니다.".into());
        }
        let client: IAudioClient = endpoint
            .Activate(CLSCTX_ALL, None)
            .map_err(|error| error.to_string())?;
        let format = client.GetMixFormat().map_err(|error| error.to_string())?;
        if format.is_null() {
            return Err("출력 mix 형식이 없습니다.".into());
        }
        let initialized = client.Initialize(AUDCLNT_SHAREMODE_SHARED, 0, 0, 0, format, None);
        CoTaskMemFree(Some(format.cast()));
        initialized.map_err(|error| error.to_string())?;
        let latency = client
            .GetStreamLatency()
            .map_err(|error| error.to_string())?;
        let mut period = 0;
        client
            .GetDevicePeriod(Some(&raw mut period), None)
            .map_err(|error| error.to_string())?;
        if latency < 0 || period <= 0 {
            return Err("출력 지연이나 장치 period가 잘못됐습니다.".into());
        }
        let nanos = u64::try_from(
            latency
                .checked_add(period)
                .ok_or("출력 지연 범위를 넘었습니다.")?,
        )
        .map_err(|error| error.to_string())?
        .checked_mul(100)
        .ok_or("출력 지연 범위를 넘었습니다.")?;
        Ok(Duration::from_nanos(nanos))
    }
}
