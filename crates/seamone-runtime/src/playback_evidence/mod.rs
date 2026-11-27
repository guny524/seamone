//! Issues storage authorization only after the runtime device output covers the current revision.

mod output;
mod player;

use crate::store::Store;
use base64::{Engine, engine::general_purpose::STANDARD};
use seamone_core::{
    playback::{Action, Status},
    review::{Error, PlaybackReply, Request, SampleRange},
};
use std::{io::Cursor, sync::Mutex};

pub(super) struct PlaybackProof<'a> {
    request: &'a Request,
}

enum Authority<'a> {
    Played(PlaybackProof<'a>),
    PlaybackNotRequired(&'a Request),
}

pub(super) struct AuthorizedReview<'a> {
    authority: Authority<'a>,
}

impl<'a> AuthorizedReview<'a> {
    pub(super) fn request(&self) -> &'a Request {
        match &self.authority {
            Authority::Played(proof) => proof.request,
            Authority::PlaybackNotRequired(request) => request,
        }
    }
}

impl AuthorizedReview<'_> {
    pub(super) fn admit(
        self,
        current: &seamone_core::review::Segment,
        next: &seamone_core::review::Segment,
    ) -> Result<(), String> {
        if next.verified.is_some()
            && next.verified != current.verified
            && !matches!(&self.authority, Authority::Played(_))
        {
            return Err("확인 전사의 변경에는 완료된 재생 승인이 필요합니다.".into());
        }
        Ok(())
    }
}

pub(super) struct Playbacks {
    player: Mutex<player::Player>,
    #[cfg(test)]
    authorization_started: Option<std::sync::mpsc::Sender<()>>,
}

fn samples(wav: &[u8]) -> Result<Vec<f32>, String> {
    hound::WavReader::new(Cursor::new(wav))
        .map_err(|e| e.to_string())?
        .samples::<i16>()
        .map(|sample| {
            sample
                .map(|sample| f32::from(sample) / 32_768.0)
                .map_err(|e| e.to_string())
        })
        .collect()
}

impl Playbacks {
    pub(super) fn new() -> Self {
        Self {
            player: Mutex::new(player::Player::new()),
            #[cfg(test)]
            authorization_started: None,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn fake() -> Self {
        Self {
            player: Mutex::new(player::Player::fake()),
            #[cfg(test)]
            authorization_started: None,
        }
    }

    pub(super) fn prepare_review(
        &self,
        store: impl std::ops::Deref<Target = Store>,
        id: &str,
        revision: u64,
    ) -> Result<PlaybackReply, String> {
        let segment = store.segment(id)?;
        let recording = store.recording(&segment.recording_id)?;
        if segment.revision != revision || segment.recording_revision != recording.revision {
            return Err(Error::StaleRevision.to_string());
        }
        let wav = store
            .audio
            .range_wav(&segment.recording_id, segment.range, recording.samples)?;
        drop(store);
        let session = self.player.lock().map_err(|e| e.to_string())?.load(
            samples(&wav)?,
            segment.range.start,
            Some((id.into(), revision)),
        )?;
        Ok(PlaybackReply {
            segment_id: id.into(),
            revision,
            wav_base64: STANDARD.encode(wav),
            session,
        })
    }

    pub(super) fn play_recording(
        &self,
        store: impl std::ops::Deref<Target = Store>,
        id: &str,
        revision: u64,
        range: SampleRange,
    ) -> Result<Status, String> {
        let recording = store.recording(id)?;
        if recording.revision != revision {
            return Err(Error::StaleRevision.to_string());
        }
        let wav = store.audio.range_wav(id, range, recording.samples)?;
        drop(store);
        let mut player = self.player.lock().map_err(|e| e.to_string())?;
        let session = player.load(samples(&wav)?, range.start, None)?;
        player.control(&session, Action::Play)
    }

    pub(super) fn control(&self, session: &str, action: Action) -> Result<Status, String> {
        self.player
            .lock()
            .map_err(|e| e.to_string())?
            .control(session, action)
    }

    pub(super) fn status(&self) -> Result<Status, String> {
        Ok(self.player.lock().map_err(|e| e.to_string())?.status())
    }

    pub(super) fn close(&self) {
        if let Ok(mut player) = self.player.lock() {
            player.close();
        }
    }

    pub(super) fn authorize_review<'a>(
        &self,
        request: &'a Request,
        session: Option<&str>,
    ) -> Result<AuthorizedReview<'a>, String> {
        let authority = if request.action.required_evidence().requires_playback() {
            #[cfg(test)]
            if let Some(started) = &self.authorization_started {
                let _ = started.send(());
            }
            if !self
                .player
                .lock()
                .map_err(|e| e.to_string())?
                .authorizes(request, session)
            {
                return Err("현재 수정본의 음성을 끝까지 들은 뒤 확인하세요.".into());
            }
            Authority::Played(PlaybackProof { request })
        } else {
            Authority::PlaybackNotRequired(request)
        };
        Ok(AuthorizedReview { authority })
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn advance(&self, frames: usize) -> Result<(), String> {
        self.player
            .lock()
            .map_err(|e| e.to_string())?
            .advance(frames)
    }

    #[cfg(test)]
    pub(super) fn finish(&self, session: &str) -> Result<(), String> {
        let mut player = self.player.lock().map_err(|e| e.to_string())?;
        player.control(session, Action::Play)?;
        player.finish()
    }
}

#[cfg(test)]
pub(super) fn mutate_after_playback(
    store: &mut Store,
    request: &Request,
) -> Result<seamone_core::review::Segment, String> {
    let playbacks = Playbacks::fake();
    let session = if request.action.required_evidence().requires_playback() {
        let reply =
            playbacks.prepare_review(&*store, &request.segment_id, request.expected_revision)?;
        playbacks.finish(&reply.session)?;
        Some(reply.session)
    } else {
        None
    };
    store.mutate(playbacks.authorize_review(request, session.as_deref())?)
}

#[cfg(test)]
mod tests;
