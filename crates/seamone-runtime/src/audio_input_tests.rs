use crate::{
    audio::{MAX_RECORDING_SAMPLES, canonical_spec},
    audio_ingest,
    normalize::Normalizer,
    store::Store,
};
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use std::{
    f32::consts::TAU,
    fs,
    io::{Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

pub(crate) fn fixture() -> tempfile::TempDir {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/native-tests");
    fs::create_dir_all(&root).unwrap();
    tempfile::tempdir_in(root).unwrap()
}

fn encoded(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

fn wav_samples(store: &Store, id: &str) -> Vec<i16> {
    let mut wav = WavReader::open(store.audio.existing_path(id).unwrap()).unwrap();
    assert_eq!(wav.spec(), canonical_spec());
    wav.samples().collect::<Result<_, _>>().unwrap()
}

#[test]
fn canonical_wav_preserves_signed_samples_and_reopens_through_sqlite() {
    let root = fixture();
    let source = root.path().join("canonical.wav");
    let app = root.path().join("app");
    let samples = [i16::MIN, -22_222, -1, 0, 1, 22_222, i16::MAX];
    let mut wav = WavWriter::create(&source, canonical_spec()).unwrap();
    for sample in samples {
        wav.write_sample(sample).unwrap();
    }
    wav.finalize().unwrap();
    let recording = Store::open(&app).unwrap().import(&source).unwrap();
    let store = Store::open(&app).unwrap();
    assert_eq!(store.recording(&recording.id).unwrap(), recording);
    assert_eq!(wav_samples(&store, &recording.id), samples);
    let (_, audio) = store.prepare_transcription(&recording.id).unwrap();
    assert_eq!(audio, samples.map(|value| f32::from(value) / 32_768.0));
}

#[test]
fn stereo_float_wav_downmixes_and_resamples_to_exact_pcm16_values() {
    let root = fixture();
    let source = root.path().join("stereo.wav");
    let mut wav = WavWriter::create(
        &source,
        WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 32,
            sample_format: SampleFormat::Float,
        },
    )
    .unwrap();
    for _ in 0..4800 {
        wav.write_sample(1.5_f32).unwrap();
        wav.write_sample(-0.5_f32).unwrap();
    }
    wav.finalize().unwrap();
    let mut store = Store::open(&root.path().join("app")).unwrap();
    let info = store.import(&source).unwrap();
    let pcm = wav_samples(&store, &info.id);
    assert_eq!(info.samples, 1600);
    assert_eq!(pcm.len(), 1600);
    assert!(pcm[32..1568].iter().all(|&sample| sample == 16_384));
}

#[test]
fn normalizer_clamps_and_rejects_aliasing_above_nyquist() {
    let mut direct = Normalizer::new(16_000, 1).unwrap();
    let mut pcm = Vec::new();
    for sample in [-4.0, -1.0, -0.5, f32::NAN, 0.5, 1.0, 4.0] {
        direct
            .push(sample, |sample| {
                pcm.push(sample);
                Ok(())
            })
            .unwrap();
    }
    direct.finish(|_| unreachable!()).unwrap();
    assert_eq!(
        pcm,
        [i16::MIN, i16::MIN, -16384, 0, 16384, i16::MAX, i16::MAX]
    );

    for rate in [44_100_u16, 48_000] {
        let convert = |frequency: f32| {
            let mut normalizer = Normalizer::new(u32::from(rate), 1).unwrap();
            let mut pcm = Vec::new();
            let mut collect = |sample| {
                pcm.push(sample);
                Ok(())
            };
            for frame in 0..(rate / 10) {
                let sample = 0.8 * (TAU * frequency * f32::from(frame) / f32::from(rate)).sin();
                normalizer.push(sample, &mut collect).unwrap();
            }
            normalizer.finish(collect).unwrap();
            assert_eq!(pcm.len(), 1600);
            pcm[64..1536]
                .iter()
                .map(|&s| i32::from(s).abs())
                .max()
                .unwrap()
        };
        assert!(convert(1000.0) > 24_000);
        assert!(convert(10_000.0) < 100);
    }
}

#[test]
fn mp3_and_aac_lc_m4a_save_readable_audio_on_the_canonical_timeline() {
    let root = fixture();
    let app = root.path().join("app");
    let mut store = Store::open(&app).unwrap();
    for filename in ["stereo-44100.mp3", "stereo-48000.m4a"] {
        let info = store
            .import(&encoded(filename))
            .unwrap_or_else(|e| panic!("{filename}: {e}"));
        assert!((4600..=5600).contains(&info.samples), "{info:?}");
        let pcm = wav_samples(&store, &info.id);
        assert_eq!(pcm.len() as u64, info.samples);
        assert!(pcm[400..4400].iter().any(|&sample| sample > 6500));
        let (saved, samples) = Store::open(&app)
            .unwrap()
            .prepare_transcription(&info.id)
            .unwrap();
        assert_eq!(saved, info);
        assert_eq!(samples.len() as u64, info.samples);
    }
}

#[test]
fn untagged_mp3_accepts_a_whole_last_frame_and_rejects_a_truncated_frame() {
    let root = fixture();
    let mut store = Store::open(&root.path().join("app")).unwrap();
    let source = encoded("stereo-44100-no-xing.mp3");
    let saved = store.import(&source).unwrap();
    assert!(saved.samples > 4800);
    let mp3 = fs::read(source).unwrap();
    let truncated = root.path().join("short.mp3");
    fs::write(&truncated, &mp3[..mp3.len() - 32]).unwrap();
    let error = store.import(&truncated).unwrap_err();
    assert!(error.contains("잘렸"), "{error}");
    assert_eq!(store.library(false).unwrap().recordings, vec![saved]);
}

#[test]
fn bad_audio_and_uncommitted_saves_leave_no_library_files_or_rows() {
    let root = fixture();
    let app = root.path().join("app");
    let mut store = Store::open(&app).unwrap();
    let empty = root.path().join("empty.wav");
    WavWriter::create(&empty, canonical_spec())
        .unwrap()
        .finalize()
        .unwrap();
    let corrupt = root.path().join("unknown.mp3");
    fs::write(&corrupt, b"this is not an audio file").unwrap();
    let long = root.path().join("long.wav");
    let mut header = fs::read(&empty).unwrap();
    let data_bytes = u32::try_from((MAX_RECORDING_SAMPLES + 1) * 2).unwrap();
    header[4..8].copy_from_slice(&(data_bytes + 36).to_le_bytes());
    header[40..44].copy_from_slice(&data_bytes.to_le_bytes());
    fs::write(&long, &header).unwrap();
    let file = fs::OpenOptions::new().write(true).open(&long).unwrap();
    file.set_len(u64::from(data_bytes) + 44).unwrap();
    let large = root.path().join("large.wav");
    fs::File::create(&large)
        .unwrap()
        .set_len(audio_ingest::MAX_FILE_BYTES + 1)
        .unwrap();
    let truncated = root.path().join("truncated.wav");
    fs::write(&truncated, &header).unwrap();
    let truncated_mp3 = root.path().join("truncated.mp3");
    let mp3 = fs::read(encoded("stereo-44100.mp3")).unwrap();
    fs::write(&truncated_mp3, &mp3[..mp3.len() - 150]).unwrap();
    let truncated_m4a = root.path().join("truncated.m4a");
    let m4a = fs::read(encoded("stereo-48000.m4a")).unwrap();
    fs::write(&truncated_m4a, &m4a[..m4a.len() - 150]).unwrap();

    for (path, reason) in [
        (empty.clone(), "음성이 없습니다"),
        (corrupt, "지원하지 않는 형식"),
        (long, "30분"),
        (large, "512MiB"),
        (truncated, "잘렸습니다"),
        (truncated_mp3, "잘렸거나 손상"),
        (truncated_m4a, "잘렸거나 손상"),
        (encoded("video-only.m4a"), "오디오 트랙이 없습니다"),
    ] {
        let error = store.import(&path).unwrap_err();
        assert!(error.contains(reason), "{}: {error}", path.display());
        assert_eq!(fs::read_dir(app.join("audio")).unwrap().count(), 0);
        assert_eq!(store.library(false).unwrap().recordings, []);
    }

    let valid = root.path().join("valid.wav");
    let mut file = fs::File::create(&valid).unwrap();
    file.write_all(&fs::read(empty).unwrap()).unwrap();
    file.seek(SeekFrom::Start(4)).unwrap();
    file.write_all(&38_u32.to_le_bytes()).unwrap();
    file.seek(SeekFrom::Start(40)).unwrap();
    file.write_all(&[2, 0, 0, 0, 123, 0]).unwrap();
    let mut audio =
        audio_ingest::decode(&store.audio, store.audio.open_source(&valid).unwrap()).unwrap();
    assert!(
        store
            .save_audio("취소".into(), &mut audio, || false)
            .is_err()
    );
    let connection = rusqlite::Connection::open(app.join("seamone.sqlite3")).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_save BEFORE INSERT ON recordings BEGIN SELECT RAISE(FAIL, 'test save failure'); END;").unwrap();
    assert!(
        store
            .import(&valid)
            .unwrap_err()
            .contains("test save failure")
    );
    assert_eq!(fs::read_dir(app.join("audio")).unwrap().count(), 0);
    assert_eq!(store.library(false).unwrap().recordings, []);
    connection
        .execute_batch("DROP TRIGGER reject_save;")
        .unwrap();
    let saved = store.import(&valid).unwrap();
    assert_eq!(wav_samples(&store, &saved.id), [123]);
}
