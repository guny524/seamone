//! Issues storage authorization after applying the domain's playback requirements.

use crate::store::Store;
use base64::{Engine, engine::general_purpose::STANDARD};
use seamone_core::review::{Error, PlaybackReply, Request};
use std::{collections::HashMap, sync::Mutex};
use uuid::Uuid;

struct Listening {
    segment_id: String,
    revision: u64,
    complete: bool,
}

/// Opaque proof tied to the exact immutable request that passed playback validation.
/// Its fields and construction stay inside this module, including in crate-local callers.
pub(super) struct PlaybackProof<'a> {
    request: &'a Request,
}

enum Authority<'a> {
    Played(PlaybackProof<'a>),
    PlaybackNotRequired(&'a Request),
}

/// The only input accepted by `Store::mutate`, neither variant can be forged by siblings.
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
    /// A newly created or changed verified snapshot requires completed playback authority.
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
    listening: Mutex<HashMap<String, Listening>>,
}

impl Playbacks {
    pub(super) fn new() -> Self {
        Self {
            listening: Mutex::new(HashMap::new()),
        }
    }

    /// Preparing the exact current audio is the only way to issue a token.
    pub(super) fn issue_playback_token(
        &self,
        store: &Store,
        id: &str,
        revision: u64,
    ) -> Result<PlaybackReply, String> {
        let segment = store.segment(id)?;
        if segment.revision != revision {
            return Err(Error::StaleRevision.to_string());
        }
        let recording = store.recording(&segment.recording_id)?;
        let wav = store
            .audio
            .range_wav(&segment.recording_id, segment.range, recording.samples)?;
        let token = self.begin(id, revision)?;
        Ok(PlaybackReply {
            segment_id: id.into(),
            revision,
            wav_base64: STANDARD.encode(wav),
            token,
        })
    }

    fn begin(&self, id: &str, revision: u64) -> Result<String, String> {
        let token = Uuid::new_v4().to_string();
        let mut listening = self.listening.lock().map_err(|e| e.to_string())?;
        if listening.len() >= 128 {
            listening.clear();
        }
        listening.insert(
            token.clone(),
            Listening {
                segment_id: id.into(),
                revision,
                complete: false,
            },
        );
        Ok(token)
    }

    pub(super) fn mark_playback_ended(&self, token: &str) -> Result<(), String> {
        let mut listening = self.listening.lock().map_err(|e| e.to_string())?;
        let item = listening
            .get_mut(token)
            .ok_or("재생이 바뀌었습니다. 이 조각을 다시 들으세요.")?;
        item.complete = true;
        Ok(())
    }

    pub(super) fn authorize_review<'a>(
        &self,
        request: &'a Request,
        token: Option<&str>,
    ) -> Result<AuthorizedReview<'a>, String> {
        let authority = if request.action.required_evidence().requires_playback() {
            let listening = self.listening.lock().map_err(|e| e.to_string())?;
            let valid = token.and_then(|t| listening.get(t)).is_some_and(|item| {
                item.complete
                    && item.segment_id == request.segment_id
                    && item.revision == request.expected_revision
            });
            if !valid {
                return Err("현재 수정본의 음성을 끝까지 들은 뒤 확인하세요.".into());
            }
            Authority::Played(PlaybackProof { request })
        } else {
            Authority::PlaybackNotRequired(request)
        };
        Ok(AuthorizedReview { authority })
    }
}

/// Storage tests prepare current audio through the production issuer before authorization.
#[cfg(test)]
pub(super) fn mutate_after_playback(
    store: &mut Store,
    request: &Request,
) -> Result<seamone_core::review::Segment, String> {
    let playbacks = Playbacks::new();
    let token = if request.action.required_evidence().requires_playback() {
        let reply = playbacks.issue_playback_token(
            store,
            &request.segment_id,
            request.expected_revision,
        )?;
        playbacks.mark_playback_ended(&reply.token)?;
        Some(reply.token)
    } else {
        None
    };
    store.mutate(playbacks.authorize_review(request, token.as_deref())?)
}

#[cfg(test)]
mod tests {
    use super::Playbacks;
    use crate::store_tests::Fixture;
    use seamone_core::review::{Action, Request, Verdict};

    fn request(segment: &seamone_core::review::Segment, action: Action) -> Request {
        Request {
            event_id: "event".into(),
            segment_id: segment.id.clone(),
            expected_revision: segment.revision,
            action,
        }
    }

    #[test]
    fn playback_required_actions_reject_missing_unfinished_and_wrong_scope_tokens() {
        for action in [
            Action::Judge {
                verdict: Verdict::Yes,
            },
            Action::Judge {
                verdict: Verdict::No,
            },
            Action::CorrectAndConfirm {
                text: "교정".into(),
            },
            Action::ConfirmCorrection,
        ] {
            let fixture = Fixture::new();
            let (store, segment) = fixture.transcribed();
            let playbacks = Playbacks::new();
            let request = request(&segment, action);
            assert!(playbacks.authorize_review(&request, None).is_err());
            let token = playbacks
                .issue_playback_token(&store, &segment.id, segment.revision)
                .unwrap()
                .token;
            assert!(playbacks.authorize_review(&request, Some(&token)).is_err());
            playbacks.mark_playback_ended(&token).unwrap();
            let authorized = playbacks.authorize_review(&request, Some(&token)).unwrap();
            assert_eq!(authorized.request(), &request);
            let stale = Request {
                expected_revision: segment.revision + 1,
                ..request.clone()
            };
            assert!(playbacks.authorize_review(&stale, Some(&token)).is_err());
            let other = Request {
                segment_id: "other".into(),
                ..request
            };
            assert!(playbacks.authorize_review(&other, Some(&token)).is_err());
        }
    }

    #[test]
    fn issuance_requires_a_current_segment_and_readable_exact_audio() {
        let fixture = Fixture::new();
        let (store, segment) = fixture.transcribed();
        let playbacks = Playbacks::new();
        assert!(
            playbacks
                .issue_playback_token(&store, "missing", segment.revision)
                .is_err()
        );
        assert_eq!(
            playbacks
                .issue_playback_token(&store, &segment.id, segment.revision + 1)
                .unwrap_err(),
            seamone_core::review::Error::StaleRevision.to_string()
        );
        assert!(playbacks.listening.lock().unwrap().is_empty());
        let reply = playbacks
            .issue_playback_token(&store, &segment.id, segment.revision)
            .unwrap();
        assert_ne!(reply.wav_base64, "");
        assert_eq!(reply.segment_id, segment.id);
        assert_eq!(reply.revision, segment.revision);
        let path = store.audio.existing_path(&segment.recording_id).unwrap();
        std::fs::write(path, b"unreadable WAV").unwrap();
        assert!(
            playbacks
                .issue_playback_token(&store, &segment.id, segment.revision)
                .is_err()
        );
        assert_eq!(playbacks.listening.lock().unwrap().len(), 1);
    }

    #[test]
    fn proof_free_actions_cannot_attach_a_verified_snapshot() {
        let fixture = Fixture::new();
        let (store, segment) = fixture.transcribed();
        let playbacks = Playbacks::new();
        let draft = request(
            &segment,
            Action::SaveDraft {
                text: "교정".into(),
            },
        );
        let authority = playbacks.authorize_review(&draft, None).unwrap();
        let mut next = segment.apply(&draft, 16_000).unwrap();
        playbacks
            .authorize_review(&draft, None)
            .unwrap()
            .admit(&segment, &next)
            .unwrap();
        next.verified = Some(
            serde_json::from_value(serde_json::json!({
                "revision": next.revision, "text": next.text, "source": "corrected"
            }))
            .unwrap(),
        );
        assert_eq!(
            authority.admit(&segment, &next).unwrap_err(),
            "확인 전사의 변경에는 완료된 재생 승인이 필요합니다."
        );

        let yes = request(
            &segment,
            Action::Judge {
                verdict: Verdict::Yes,
            },
        );
        let reply = playbacks
            .issue_playback_token(&store, &segment.id, segment.revision)
            .unwrap();
        playbacks.mark_playback_ended(&reply.token).unwrap();
        let accepted = segment.apply(&yes, 16_000).unwrap();
        playbacks
            .authorize_review(&yes, Some(&reply.token))
            .unwrap()
            .admit(&segment, &accepted)
            .unwrap();

        let same = request(
            &accepted,
            Action::SaveDraft {
                text: accepted.text.clone(),
            },
        );
        let unchanged = accepted.apply(&same, 16_000).unwrap();
        playbacks
            .authorize_review(&same, None)
            .unwrap()
            .admit(&accepted, &unchanged)
            .unwrap();
        let invalidate = request(&accepted, Action::Invalidate);
        let cleared = accepted.apply(&invalidate, 16_000).unwrap();
        playbacks
            .authorize_review(&invalidate, None)
            .unwrap()
            .admit(&accepted, &cleared)
            .unwrap();
    }

    #[test]
    fn edits_without_playback_requirements_keep_their_proof_free_path() {
        let fixture = Fixture::new();
        let (_store, segment) = fixture.transcribed();
        let playbacks = Playbacks::new();
        for action in [
            Action::SaveDraft {
                text: "초안".into(),
            },
            Action::DismissErrorMark,
            Action::Invalidate,
        ] {
            let request = request(&segment, action);
            assert_eq!(
                playbacks
                    .authorize_review(&request, None)
                    .unwrap()
                    .request(),
                &request
            );
        }
        assert!(playbacks.mark_playback_ended("unknown").is_err());
    }
}
