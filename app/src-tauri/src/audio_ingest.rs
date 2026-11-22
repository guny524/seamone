use crate::{
    audio::{AudioLibrary, AudioWriter, StagedAudio, canonical_spec},
    normalize::{EMPTY_ERROR, LENGTH_ERROR, Normalizer, channel_average, check_input, finite},
};
use hound::{SampleFormat, WavReader};
use seamone_core::capture::MAX_RECORDING_SECONDS;
use std::{
    fs::File,
    io::{BufReader, ErrorKind, Read, Seek, SeekFrom},
    path::Path,
};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::{CODEC_TYPE_AAC, CODEC_TYPE_MP3, DecoderOptions},
    errors::Error,
    formats::FormatOptions,
    io::{MediaSourceStream, MediaSourceStreamOptions},
    meta::MetadataOptions,
    probe::Hint,
};

pub const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
pub const SIZE_ERROR: &str = "가져올 음성 파일은 512MiB 이하여야 합니다.";
const DAMAGED_AUDIO: &str = "오디오 데이터가 중간에서 잘렸거나 손상됐습니다.";

pub fn display_name(source: &Path) -> String {
    source.file_name().map_or_else(
        || "가져온 음성".into(),
        |name| name.to_string_lossy().chars().take(200).collect(),
    )
}

pub fn decode(library: &AudioLibrary, mut input: File) -> Result<StagedAudio, String> {
    let bytes = input.metadata().map_err(|e| e.to_string())?.len();
    if bytes > MAX_FILE_BYTES {
        return Err(SIZE_ERROR.into());
    }
    input.rewind().map_err(|e| e.to_string())?;
    let mut prefix = [0; 12];
    input
        .read_exact(&mut prefix)
        .map_err(|e| format!("음성 파일이 비어 있거나 헤더가 잘렸습니다. {e}"))?;
    input.rewind().map_err(|e| e.to_string())?;
    let mut output = AudioWriter::new(library)?;
    if &prefix[..4] == b"RIFF" && &prefix[8..] == b"WAVE" {
        let riff_bytes = u64::from(u32::from_le_bytes(prefix[4..8].try_into().unwrap())) + 8;
        if riff_bytes > bytes {
            return Err("WAV가 중간에서 잘렸습니다.".into());
        }
        decode_wav(input, &mut output)?;
    } else {
        decode_compressed(input, &mut output)?;
    }
    output.finish()
}

#[allow(clippy::cast_precision_loss)]
fn decode_wav(input: File, output: &mut AudioWriter) -> Result<(), String> {
    let wav_error = |error| format!("WAV가 손상됐거나 지원하지 않는 인코딩입니다. {error}");
    let mut wav = WavReader::new(BufReader::new(input)).map_err(wav_error)?;
    let spec = wav.spec();
    let channels = usize::from(spec.channels);
    check_input(spec.sample_rate, channels)?;
    if u64::from(wav.duration()) > u64::from(spec.sample_rate) * u64::from(MAX_RECORDING_SECONDS) {
        return Err(LENGTH_ERROR.into());
    }
    if spec == canonical_spec() {
        for sample in wav.samples::<i16>() {
            output.write(sample.map_err(wav_error)?)?;
        }
        return Ok(());
    }

    let mut converter = Normalizer::new(spec.sample_rate, channels)?;
    let mut write = |sample| output.write(sample);
    let mut sum = 0.0;
    let mut channel = 0;
    let mut push = |sample| {
        sum += f64::from(finite(sample));
        channel += 1;
        if channel == spec.channels {
            converter.push(channel_average(sum, spec.channels), &mut write)?;
            sum = 0.0;
            channel = 0;
        }
        Ok::<(), String>(())
    };
    match spec.sample_format {
        SampleFormat::Int => {
            let scale = 2_f32.powi(i32::from(spec.bits_per_sample) - 1);
            for sample in wav.samples::<i32>() {
                push(sample.map_err(wav_error)? as f32 / scale)?;
            }
        }
        SampleFormat::Float => {
            for sample in wav.samples::<f32>() {
                push(sample.map_err(wav_error)?)?;
            }
        }
    }
    converter.finish(write)
}

fn decode_compressed(input: File, output: &mut AudioWriter) -> Result<(), String> {
    let source = MediaSourceStream::new(Box::new(input), MediaSourceStreamOptions::default());
    let mut format = symphonia::default::get_probe()
        .format(&Hint::new(), source, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| match e {
            Error::IoError(e) if e.kind() == ErrorKind::UnexpectedEof => DAMAGED_AUDIO.into(),
            _ => format!("파일이 손상됐거나 지원하지 않는 형식입니다. WAV, MP3 또는 AAC-LC M4A를 고르세요. {e}"),
        })?
        .format;
    let track = format
        .tracks()
        .iter()
        .find(|t| matches!(t.codec_params.codec, CODEC_TYPE_AAC | CODEC_TYPE_MP3))
        .ok_or("파일에 지원하는 오디오 트랙이 없습니다. MP3 또는 AAC-LC 오디오를 사용하세요.")?;
    let track_id = track.id;
    let params = &track.codec_params;
    let expected_ticks = params.n_frames.filter(|&count| {
        count > 0
            && (params.codec == CODEC_TYPE_AAC
                || params.delay.is_some()
                || params.padding.is_some())
    });
    let start = params.start_ts;
    // Untagged MP3 duration is estimated. Trim only when the encoder provided
    // both a reliable length and padding information.
    let mp3_trim = (params.codec == CODEC_TYPE_MP3)
        .then_some(expected_ticks)
        .flatten()
        .map(|end| {
            (
                u64::from(params.delay.unwrap_or(0)),
                end.saturating_sub(u64::from(params.padding.unwrap_or(0))),
            )
        });
    let mut untagged_last_packet =
        (params.codec == CODEC_TYPE_MP3 && expected_ticks.is_none()).then(Vec::new);
    if let (Some(ticks), Some(time_base)) = (expected_ticks, params.time_base)
        && time_base.calc_time(ticks).seconds > u64::from(MAX_RECORDING_SECONDS)
    {
        return Err(LENGTH_ERROR.into());
    }
    let mut decoder = symphonia::default::get_codecs()
        .make(params, &DecoderOptions { verify: true })
        .map_err(|e| format!("지원하지 않거나 손상된 오디오 코덱입니다. {e}"))?;
    let mut pipeline = None;
    let mut end = 0;
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(Error::IoError(e)) if e.kind() == ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(format!("오디오 패킷을 읽지 못했습니다. {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let audio = decoder
            .decode(&packet)
            .map_err(|e| format!("오디오를 디코딩하지 못했습니다. {e}"))?;
        let spec = *audio.spec();
        let channels = spec.channels.count();
        if pipeline.is_none() {
            pipeline = Some((spec, Normalizer::new(spec.rate, channels)?));
        }
        let (input_spec, converter) = pipeline.as_mut().unwrap();
        if spec != *input_spec {
            return Err("파일 중간에 오디오 채널이나 샘플링 주파수가 바뀝니다.".into());
        }
        let mut interleaved = SampleBuffer::<f32>::new(audio.capacity() as u64, spec);
        interleaved.copy_interleaved_ref(audio);
        let channel_count = u16::try_from(channels).unwrap();
        let samples = interleaved.samples();
        let frames = (samples.len() / channels) as u64;
        let (first, last) = mp3_trim.map_or((0, frames), |(first, last)| {
            (
                first.saturating_sub(packet.ts()).min(frames),
                last.saturating_sub(packet.ts()).min(frames),
            )
        });
        let last = usize::try_from(last).unwrap() * channels;
        let first = (usize::try_from(first).unwrap() * channels).min(last);
        for frame in samples[first..last].chunks_exact(channels) {
            let sum = frame.iter().copied().map(finite).map(f64::from).sum();
            let mono = channel_average(sum, channel_count);
            converter.push(mono, |sample| output.write(sample))?;
        }
        end = packet.ts().saturating_add(packet.dur());
        if let Some(last) = &mut untagged_last_packet {
            last.clear();
            last.extend_from_slice(packet.buf());
        }
    }
    if expected_ticks.is_some_and(|ticks| end.saturating_sub(start) < ticks)
        || decoder.finalize().verify_ok == Some(false)
    {
        return Err(DAMAGED_AUDIO.into());
    }
    if let Some(last) = untagged_last_packet.filter(|packet| packet.len() >= 4) {
        check_mp3_tail(format.into_inner(), &last)?;
    }
    let (_, converter) = pipeline.ok_or(EMPTY_ERROR)?;
    converter.finish(|sample| output.write(sample))
}

fn check_mp3_tail(mut source: MediaSourceStream, last: &[u8]) -> Result<(), String> {
    // The demuxer reports the same EOF when scanning for a header and when reading
    // a partial frame. Only diagnose truncation if a matching next header at the
    // end of the last known-good frame declares more bytes than are present.
    let end = source.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;
    let count = end.min(4096);
    source
        .seek(SeekFrom::Start(end - count))
        .map_err(|e| e.to_string())?;
    let mut tail = vec![0; usize::try_from(count).unwrap()];
    source.read_exact(&mut tail).map_err(|e| e.to_string())?;
    if tail.len() >= 128 && tail[tail.len() - 128..].starts_with(b"TAG") {
        tail.truncate(tail.len() - 128);
    }
    if let Some(index) = tail.windows(last.len()).rposition(|bytes| bytes == last) {
        let next = &tail[index + last.len()..];
        if let Some(header) = next.first_chunk::<4>()
            && header[1] & 0x18 == last[1] & 0x18
            && header[2] & 0x0c == last[2] & 0x0c
            && mp3_frame_bytes(*header).is_some_and(|size| size > next.len())
        {
            return Err(DAMAGED_AUDIO.into());
        }
    }
    Ok(())
}

fn mp3_frame_bytes([sync, version_layer, bitrate_rate, _]: [u8; 4]) -> Option<usize> {
    if sync != 0xff || version_layer & 0xe6 != 0xe2 {
        return None;
    }
    let base_rate = *[44_100, 48_000, 32_000].get(usize::from((bitrate_rate >> 2) & 3))?;
    let (rate, coefficient, bitrates) = match (version_layer >> 3) & 3 {
        3 => (
            base_rate,
            144_000,
            [
                32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
            ],
        ),
        version @ (0 | 2) => (
            base_rate / if version == 0 { 4 } else { 2 },
            72_000,
            [8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160],
        ),
        _ => return None,
    };
    let bitrate = bitrates.get(usize::from((bitrate_rate >> 4).checked_sub(1)?))?;
    Some(coefficient * bitrate / rate + usize::from((bitrate_rate >> 1) & 1))
}
