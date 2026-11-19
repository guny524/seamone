//! App-owned canonical PCM, with exact bounded range reads for the review player.
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use seamone_core::review::{SAMPLE_RATE, SampleRange};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

pub const MAX_RECORDING_SAMPLES: u64 = SAMPLE_RATE * 60 * 30;

pub struct AudioLibrary {
    root: PathBuf,
}

impl AudioLibrary {
    pub fn new(root: &Path) -> Result<Self, String> {
        fs::create_dir_all(root).map_err(|e| e.to_string())?;
        Ok(Self {
            root: root.canonicalize().map_err(|e| e.to_string())?,
        })
    }

    pub fn path(&self, id: &str) -> Result<PathBuf, String> {
        uuid::Uuid::parse_str(id).map_err(|_| "녹음 ID가 잘못됐습니다.".to_owned())?;
        Ok(self.root.join(format!("{id}.wav")))
    }

    pub fn existing_path(&self, id: &str) -> Result<PathBuf, String> {
        let path = self.path(id)?;
        let canonical = path
            .canonicalize()
            .map_err(|_| "앱의 녹음 원본이 없습니다.".to_owned())?;
        if canonical != path || !canonical.is_file() {
            return Err("앱의 녹음 원본 경로가 바뀌었습니다.".into());
        }
        Ok(canonical)
    }

    /// Input is selected by a native dialog, never supplied by the webview.
    pub fn import(&self, id: &str, source: &Path) -> Result<(u64, String), String> {
        let source = source.canonicalize().map_err(|e| e.to_string())?;
        if source.starts_with(&self.root) || !source.is_file() {
            return Err("앱의 보관 폴더 밖에 있는 WAV 파일을 고르세요.".into());
        }
        let file = File::open(source).map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() > MAX_RECORDING_SAMPLES * 2 + 65_536 {
            return Err("WAV 파일은 30분 이하, 16kHz mono PCM16이어야 합니다.".into());
        }
        let mut reader = WavReader::new(file).map_err(|e| format!("WAV를 읽지 못했습니다: {e}"))?;
        check_spec(reader.spec())?;
        let samples = u64::from(reader.duration());
        if samples == 0 || samples > MAX_RECORDING_SAMPLES {
            return Err("음성은 0초 초과, 30분 이하여야 합니다.".into());
        }
        let path = self.path(id)?;
        let file = File::options()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        let result = (|| {
            let mut writer = WavWriter::new(file, canonical_spec()).map_err(|e| e.to_string())?;
            let mut count = 0_u64;
            for sample in reader.samples::<i16>() {
                if count >= samples {
                    return Err("WAV 길이가 header와 다릅니다.".into());
                }
                writer
                    .write_sample(sample.map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
                count += 1;
            }
            if count != samples {
                return Err("WAV가 중간에서 잘렸습니다.".into());
            }
            writer.finalize().map_err(|e| e.to_string())?;
            File::open(&path)
                .and_then(|f| f.sync_all())
                .map_err(|e| e.to_string())?;
            Ok((samples, digest(&path)?))
        })();
        if result.is_err() {
            let _ = fs::remove_file(&path);
        }
        result
    }

    pub fn matches(&self, id: &str, expected: &str) -> bool {
        self.existing_path(id)
            .and_then(|p| digest(&p))
            .is_ok_and(|actual| actual == expected)
    }

    pub fn read_all(&self, id: &str) -> Result<Vec<f32>, String> {
        let mut reader = WavReader::open(self.existing_path(id)?).map_err(|e| e.to_string())?;
        check_spec(reader.spec())?;
        if u64::from(reader.duration()) > MAX_RECORDING_SAMPLES {
            return Err("녹음이 허용 길이를 넘었습니다.".into());
        }
        reader
            .samples::<i16>()
            .map(|s| {
                s.map(|v| f32::from(v) / 32_768.0)
                    .map_err(|e| e.to_string())
            })
            .collect()
    }

    pub fn range_wav(&self, id: &str, range: SampleRange, total: u64) -> Result<Vec<u8>, String> {
        range.validate(total).map_err(|e| e.to_string())?;
        let mut reader = WavReader::open(self.existing_path(id)?).map_err(|e| e.to_string())?;
        check_spec(reader.spec())?;
        if u64::from(reader.duration()) != total {
            return Err("녹음 길이가 바뀌었습니다.".into());
        }
        reader
            .seek(u32::try_from(range.start).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let mut cursor = Cursor::new(Vec::new());
        let mut writer =
            WavWriter::new(&mut cursor, canonical_spec()).map_err(|e| e.to_string())?;
        // The decoder never loads the complete recording for playback.
        for _ in range.start..range.end {
            let value = reader
                .samples::<i16>()
                .next()
                .ok_or("녹음이 중간에서 잘렸습니다.")?
                .map_err(|e| e.to_string())?;
            writer.write_sample(value).map_err(|e| e.to_string())?;
        }
        writer.finalize().map_err(|e| e.to_string())?;
        Ok(cursor.into_inner())
    }
}

pub fn canonical_spec() -> WavSpec {
    WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    }
}

fn check_spec(spec: WavSpec) -> Result<(), String> {
    if spec != canonical_spec() {
        return Err("16kHz mono PCM16 WAV만 지원합니다. 다른 형식은 먼저 변환하세요.".into());
    }
    Ok(())
}

pub fn digest(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0_u8; 65_536];
    loop {
        let count = file.read(&mut buf).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buf[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
