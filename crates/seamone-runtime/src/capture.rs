//! The input thread owns the stream from open through release. IPC only handles session state.
use crate::{
    audio::{AudioLibrary, AudioWriter, StagedAudio},
    normalize::{Normalizer, channel_average, check_input, finite},
    training::ForegroundGuard,
};
use cpal::{
    Device, FromSample, Sample, SampleFormat, SizedSample, Stream, StreamConfig, StreamError,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use rtrb::{Consumer, Producer, RingBuffer};
use seamone_core::{
    capture::{CaptureStatus, MAX_RECORDING_SECONDS},
    review::{RecordingInfo, SAMPLE_RATE},
};
use std::{
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const RUNNING: u8 = 0;
const SAVE: u8 = 1;
const DISCARD: u8 = 2;
const PAUSE_TO_CLOSE: u8 = 3;
const OVERFLOW: u8 = 1;
const DISCONNECTED: u8 = 2;
const INPUT_ERROR: u8 = 3;
const WINDOW_CLOSED: &str = "창이 닫혀 녹음을 저장하지 않았습니다.";
const CLOSE_PAUSED: &str = "창을 닫기 전에 녹음을 멈췄습니다.";
const STALLED: &str =
    "마이크에서 음성 샘플을 받지 못했습니다. 기본 입력 장치와 녹음 권한을 확인하세요.";
const QUEUE_FRAMES: usize = 65_536;
const INPUT_TIMEOUT: Duration = Duration::from_secs(3);
const INPUT_POLL: Duration = Duration::from_millis(5);

#[derive(Default)]
pub struct Recorder {
    state: Mutex<State>,
    #[cfg(test)]
    fake: bool,
}

#[derive(Default)]
struct State {
    closed: bool,
    session: Option<Session>,
}

struct Session {
    control: Arc<Control>,
    pending: Pending,
}

enum Pending {
    Input(JoinHandle<Result<Option<StagedAudio>, String>>),
    Retry(StagedAudio),
    Finishing,
}

struct Control {
    id: String,
    request: AtomicU8,
    fault: AtomicU8,
    has_audio: AtomicBool,
    seconds: AtomicU32,
    status: Mutex<CaptureStatus>,
}

impl Control {
    fn new() -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        Self {
            status: Mutex::new(CaptureStatus::Starting {
                session_id: id.clone(),
            }),
            id,
            request: AtomicU8::new(RUNNING),
            fault: AtomicU8::new(0),
            has_audio: AtomicBool::new(false),
            seconds: AtomicU32::new(0),
        }
    }

    fn status(&self) -> CaptureStatus {
        let status = self
            .status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &*status {
            CaptureStatus::Recording { session_id, .. } => CaptureStatus::Recording {
                session_id: session_id.clone(),
                elapsed_seconds: self.seconds.load(Ordering::Relaxed),
            },
            _ => status.clone(),
        }
    }

    fn set_status(&self, status: CaptureStatus) {
        *self
            .status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = status;
    }

    fn discarded(&self) -> bool {
        self.request.load(Ordering::Acquire) == DISCARD
    }

    fn input_fault(&self) -> Option<&'static str> {
        match self.fault.load(Ordering::Acquire) {
            OVERFLOW => {
                Some("마이크 입력을 제때 저장하지 못했습니다. 다른 작업을 닫고 다시 녹음하세요.")
            }
            DISCONNECTED => Some("마이크 연결이 끊겼습니다. 다시 연결한 뒤 녹음하세요."),
            INPUT_ERROR => Some("마이크 입력에 실패했습니다. 입력 장치와 녹음 권한을 확인하세요."),
            _ => None,
        }
    }
}

impl Recorder {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn status(&self) -> CaptureStatus {
        let state = self.state();
        let Some(session) = &state.session else {
            return CaptureStatus::Idle;
        };
        if matches!(session.pending, Pending::Finishing) {
            CaptureStatus::Finishing {
                session_id: session.control.id.clone(),
            }
        } else {
            session.control.status()
        }
    }

    pub fn start(
        &self,
        library: AudioLibrary,
        foreground: ForegroundGuard,
    ) -> Result<CaptureStatus, String> {
        #[cfg(test)]
        if self.fake {
            return self.start_using(library, foreground, |_| {
                let (mut producer, consumer) = RingBuffer::new(48_000);
                for _ in 0..48_000 {
                    producer.push(0.25).unwrap();
                }
                Ok(Some(Input {
                    stream: (),
                    consumer,
                    rate: 48_000,
                    channels: 2,
                }))
            });
        }
        self.start_using(library, foreground, open_native_input)
    }

    fn start_using<S: 'static>(
        &self,
        library: AudioLibrary,
        foreground: ForegroundGuard,
        open: impl FnOnce(Arc<Control>) -> Result<Option<Input<S>>, String> + Send + 'static,
    ) -> Result<CaptureStatus, String> {
        let mut state = self.state();
        if state.closed {
            return Err(WINDOW_CLOSED.into());
        }
        if state.session.as_ref().is_some_and(|session| {
            !matches!(session.pending, Pending::Input(_))
                || !matches!(session.control.status(), CaptureStatus::Failed { .. })
        }) {
            return Err("진행 중인 녹음을 저장하거나 취소하세요.".into());
        }
        let control = Arc::new(Control::new());
        let worker = Arc::clone(&control);
        let thread = thread::Builder::new()
            .name("seamone-input".into())
            .spawn(move || {
                let result = record(&library, &worker, MAX_RECORDING_SECONDS, open);
                drop(foreground);
                if let Err(message) = &result {
                    worker.set_status(CaptureStatus::Failed {
                        message: message.clone(),
                    });
                }
                result
            })
            .map_err(|e| format!("녹음을 시작하지 못했습니다. {e}"))?;
        state.session = Some(Session {
            control: Arc::clone(&control),
            pending: Pending::Input(thread),
        });
        Ok(control.status())
    }

    pub fn finish(
        &self,
        id: &str,
        save: bool,
        commit: impl FnOnce(&mut StagedAudio, &dyn Fn() -> bool) -> Result<RecordingInfo, String>,
    ) -> Result<Option<RecordingInfo>, String> {
        let (pending, control) = {
            let mut state = self.state();
            let session = state
                .session
                .as_mut()
                .filter(|session| session.control.id == id)
                .ok_or("이 녹음은 이미 끝났습니다.")?;
            if matches!(session.pending, Pending::Finishing) {
                return Err("이 녹음을 이미 마무리하고 있습니다.".into());
            }
            let status = session.control.status();
            if save
                && matches!(
                    status,
                    CaptureStatus::Starting { .. } | CaptureStatus::Failed { .. }
                )
            {
                return Err(match status {
                    CaptureStatus::Failed { message } => message,
                    _ => "마이크가 시작될 때까지 기다리세요.".into(),
                });
            }
            session
                .control
                .request
                .store(if save { SAVE } else { DISCARD }, Ordering::Release);
            (
                std::mem::replace(&mut session.pending, Pending::Finishing),
                Arc::clone(&session.control),
            )
        };
        let mut staged = match pending {
            Pending::Input(thread) => thread
                .join()
                .map_err(|_| "녹음을 마무리하지 못했습니다. 다시 시작하세요.".to_owned())
                .and_then(|result| result),
            Pending::Retry(audio) => Ok(Some(audio)),
            Pending::Finishing => unreachable!(),
        };
        let result = if save {
            match &mut staged {
                Ok(Some(audio)) if !control.discarded() => {
                    commit(audio, &|| !control.discarded()).map(Some)
                }
                Err(error) => Err(error.clone()),
                Ok(_) => Err(WINDOW_CLOSED.into()),
            }
        } else {
            Ok(None)
        };
        let mut state = self.state();
        if let Some(session) = state
            .session
            .as_mut()
            .filter(|session| session.control.id == id)
        {
            if let (Err(error), Ok(Some(audio))) = (&result, staged)
                && !control.discarded()
            {
                control.set_status(CaptureStatus::Interrupted {
                    session_id: id.into(),
                    elapsed_seconds: u32::try_from(audio.samples / SAMPLE_RATE)
                        .unwrap_or(MAX_RECORDING_SECONDS),
                    reason: format!(
                        "보관함에 녹음을 저장하지 못했습니다. 다시 저장하거나 취소하세요. {error}"
                    ),
                });
                session.pending = Pending::Retry(audio);
            } else {
                state.session = None;
            }
        }
        result
    }

    pub fn close(&self) {
        Self::discard_on_close(&mut self.state());
    }

    pub fn request_close(&self) -> bool {
        let mut state = self.state();
        if let Some(session) = &state.session {
            let control = &session.control;
            if control.has_audio.load(Ordering::Acquire)
                && !control.discarded()
                && !matches!(control.status(), CaptureStatus::Failed { .. })
            {
                let _ = control.request.compare_exchange(
                    RUNNING,
                    PAUSE_TO_CLOSE,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );
                return false;
            }
        }
        Self::discard_on_close(&mut state);
        true
    }

    fn discard_on_close(state: &mut State) {
        state.closed = true;
        if let Some(session) = state.session.take() {
            session.control.request.store(DISCARD, Ordering::Release);
        }
    }

    #[cfg(test)]
    pub fn fake() -> Self {
        Self {
            fake: true,
            state: Mutex::default(),
        }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.close();
    }
}

struct Input<S> {
    stream: S,
    consumer: Consumer<f32>,
    rate: u32,
    channels: u16,
}

fn record<S>(
    library: &AudioLibrary,
    control: &Arc<Control>,
    max_seconds: u32,
    open: impl FnOnce(Arc<Control>) -> Result<Option<Input<S>>, String>,
) -> Result<Option<StagedAudio>, String> {
    if control.discarded() {
        return Ok(None);
    }
    let mut output = AudioWriter::new(library)?;
    let Some(Input {
        stream,
        mut consumer,
        rate,
        channels,
    }) = open(Arc::clone(control))?
    else {
        return Ok(None);
    };
    let mut converter = Normalizer::new(rate, usize::from(channels))?;
    let mut write = |sample| output.write(sample);
    let started = Instant::now();
    let mut last_input = started;
    let mut frames = 0;
    let limit = u64::from(rate) * u64::from(max_seconds);
    let mut interruption = None;
    while control.request.load(Ordering::Acquire) == RUNNING
        && frames < limit
        && started.elapsed() < Duration::from_secs(u64::from(max_seconds))
    {
        if let Some(reason) = control.input_fault() {
            interruption = Some(reason);
            break;
        }
        let mut count = 0;
        // Yield to stop and fault checks even when the producer keeps the ring full.
        for _ in 0..(limit - frames).min(1024) {
            let Ok(sample) = consumer.pop() else { break };
            converter.push(sample, &mut write)?;
            count += 1;
        }
        if count > 0 {
            if frames == 0 {
                control.has_audio.store(true, Ordering::Release);
                control.set_status(CaptureStatus::Recording {
                    session_id: control.id.clone(),
                    elapsed_seconds: 0,
                });
            }
            frames += count;
            control.seconds.store(
                u32::try_from(frames / u64::from(rate)).unwrap_or(MAX_RECORDING_SECONDS),
                Ordering::Relaxed,
            );
            last_input = Instant::now();
        } else if last_input.elapsed() >= INPUT_TIMEOUT {
            interruption = Some(STALLED);
            break;
        } else {
            thread::sleep(INPUT_POLL);
        }
    }
    drop(stream);
    if control.discarded() {
        return Ok(None);
    }
    interruption = interruption.or_else(|| control.input_fault());
    // Draining after release keeps the last accepted samples in the saved recording.
    while frames < limit && !control.discarded() {
        let Ok(sample) = consumer.pop() else { break };
        converter.push(sample, &mut write)?;
        frames += 1;
    }
    if control.discarded() {
        return Ok(None);
    }
    if frames == 0
        && let Some(reason) = interruption
    {
        return Err(reason.into());
    }
    control.has_audio.store(frames > 0, Ordering::Release);
    converter.finish(write)?;
    let recorded = output.finish()?;
    let request = control.request.load(Ordering::Acquire);
    if matches!(request, RUNNING | PAUSE_TO_CLOSE) {
        let session_id = control.id.clone();
        let status = match interruption.or((request == PAUSE_TO_CLOSE).then_some(CLOSE_PAUSED)) {
            Some(reason) => CaptureStatus::Interrupted {
                session_id,
                elapsed_seconds: u32::try_from(frames / u64::from(rate))
                    .unwrap_or(MAX_RECORDING_SECONDS),
                reason: reason.into(),
            },
            None => CaptureStatus::LimitReached { session_id },
        };
        control.set_status(status);
    }
    Ok(Some(recorded))
}

struct NativeStream(Stream);

impl Drop for NativeStream {
    fn drop(&mut self) {
        // The same input thread pauses and drops the non-Send, non-Sync CPAL stream.
        let _ = self.0.pause();
    }
}

fn open_native_input(control: Arc<Control>) -> Result<Option<Input<NativeStream>>, String> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or("기본 마이크가 없습니다. 입력 장치를 연결하고 시스템에서 기본으로 선택하세요.")?;
    #[cfg(target_os = "macos")]
    authorize_macos(&control)?;
    if control.discarded() {
        return Ok(None);
    }
    let supported = device.default_input_config().map_err(|e| {
        format!("기본 마이크 설정을 읽지 못했습니다. 입력 장치와 권한을 확인하세요. {e}")
    })?;
    let config = supported.config();
    let rate = config.sample_rate.0;
    let channels = config.channels;
    check_input(rate, usize::from(channels))?;
    let (producer, consumer) = RingBuffer::new(QUEUE_FRAMES);
    macro_rules! build {
        ($sample:ty) => {
            input_stream::<$sample>(&device, &config, producer, control)
        };
    }
    let stream = match supported.sample_format() {
        SampleFormat::I8 => build!(i8),
        SampleFormat::I16 => build!(i16),
        SampleFormat::I32 => build!(i32),
        SampleFormat::I64 => build!(i64),
        SampleFormat::U8 => build!(u8),
        SampleFormat::U16 => build!(u16),
        SampleFormat::U32 => build!(u32),
        SampleFormat::U64 => build!(u64),
        SampleFormat::F32 => build!(f32),
        SampleFormat::F64 => build!(f64),
        other => return Err(format!("지원하지 않는 마이크 샘플 형식입니다. {other}")),
    }
    .map_err(|e| {
        format!("마이크를 열지 못했습니다. 기본 입력 장치와 녹음 권한을 확인하세요. {e}")
    })?;
    let stream = NativeStream(stream);
    stream.0.play().map_err(|e| {
        format!("마이크를 시작하지 못했습니다. 입력 장치와 녹음 권한을 확인하세요. {e}")
    })?;
    Ok(Some(Input {
        stream,
        consumer,
        rate,
        channels,
    }))
}

fn input_stream<T: Sample + SizedSample>(
    device: &Device,
    config: &StreamConfig,
    mut producer: Producer<f32>,
    control: Arc<Control>,
) -> Result<Stream, cpal::BuildStreamError>
where
    f32: FromSample<T>,
{
    let channels = usize::from(config.channels);
    let channel_count = config.channels;
    let limit = u64::from(config.sample_rate.0) * u64::from(MAX_RECORDING_SECONDS);
    let faults = Arc::clone(&control);
    let mut frames = 0;
    device.build_input_stream(
        config,
        move |input: &[T], _| {
            for frame in input.chunks_exact(channels) {
                if frames == limit
                    || control.request.load(Ordering::Acquire) != RUNNING
                    || control.fault.load(Ordering::Acquire) != 0
                {
                    break;
                }
                let sum = frame
                    .iter()
                    .map(|&sample| f64::from(finite(sample.to_sample::<f32>())))
                    .sum();
                let mono = channel_average(sum, channel_count);
                if producer.push(mono).is_err() {
                    control.fault.store(OVERFLOW, Ordering::Release);
                    break;
                }
                if frames == 0 {
                    control.has_audio.store(true, Ordering::Release);
                }
                frames += 1;
            }
        },
        move |error| {
            faults.fault.store(
                if matches!(error, StreamError::DeviceNotAvailable) {
                    DISCONNECTED
                } else {
                    INPUT_ERROR
                },
                Ordering::Release,
            );
        },
        None,
    )
}

#[cfg(target_os = "macos")]
fn authorize_macos(control: &Control) -> Result<(), String> {
    use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};
    use std::sync::mpsc::{self, RecvTimeoutError};

    const DENIED: &str = "마이크 접근이 허용되지 않았습니다. 시스템 설정의 개인정보 보호 및 보안에서 마이크 권한을 확인하세요.";
    if !objc2::available!(macos = 10.14) || control.discarded() {
        return Ok(());
    }
    // SAFETY: AVFoundation provides this immutable framework constant on the checked macOS version.
    let media = unsafe { AVMediaTypeAudio }.ok_or(DENIED)?;
    // SAFETY: media is the non-null AVFoundation audio media type.
    match unsafe { AVCaptureDevice::authorizationStatusForMediaType(media) } {
        AVAuthorizationStatus::Authorized => Ok(()),
        AVAuthorizationStatus::NotDetermined => {
            let (reply, result) = mpsc::channel();
            let completion = block2::RcBlock::new(move |allowed: objc2::runtime::Bool| {
                let _ = reply.send(allowed.as_bool());
            });
            // SAFETY: AVFoundation copies the heap block for its asynchronous callback; the block owns its sender.
            unsafe {
                AVCaptureDevice::requestAccessForMediaType_completionHandler(media, &completion);
            };
            while !control.discarded() {
                match result.recv_timeout(Duration::from_millis(50)) {
                    Ok(true) => return Ok(()),
                    Err(RecvTimeoutError::Timeout) => {}
                    Ok(false) | Err(RecvTimeoutError::Disconnected) => return Err(DENIED.into()),
                }
            }
            Ok(())
        }
        _ => Err(DENIED.into()),
    }
}

#[cfg(test)]
mod tests;
