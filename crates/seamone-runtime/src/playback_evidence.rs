//! Issues storage authorization after applying the domain's playback requirements.

use seamone_core::review::ReviewRequest;
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
    request: &'a ReviewRequest,
}

enum Authority<'a> {
    Played(PlaybackProof<'a>),
    PlaybackNotRequired(&'a ReviewRequest),
}

/// The only input accepted by `Store::mutate`, neither variant can be forged by siblings.
pub(super) struct AuthorizedReview<'a> {
    authority: Authority<'a>,
}

impl<'a> AuthorizedReview<'a> {
    pub(super) fn request(self) -> &'a ReviewRequest {
        match self.authority {
            Authority::Played(proof) => proof.request,
            Authority::PlaybackNotRequired(request) => request,
        }
    }
}

#[derive(Default)]
pub(super) struct Playbacks {
    listening: Mutex<HashMap<String, Listening>>,
}

impl Playbacks {
    pub(super) fn begin(&self, id: &str, revision: u64) -> Result<String, String> {
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

    pub(super) fn ended(&self, token: &str) -> Result<(), String> {
        let mut listening = self.listening.lock().map_err(|e| e.to_string())?;
        let item = listening
            .get_mut(token)
            .ok_or("재생이 바뀌었습니다. 이 조각을 다시 들으세요.")?;
        item.complete = true;
        Ok(())
    }

    pub(super) fn authorize<'a>(
        &self,
        request: &'a ReviewRequest,
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

#[cfg(test)]
mod tests {
    use super::Playbacks;
    use seamone_core::review::{ReviewAction, ReviewRequest, Verdict};

    fn request(action: ReviewAction) -> ReviewRequest {
        ReviewRequest {
            event_id: "event".into(),
            segment_id: "segment".into(),
            expected_revision: 2,
            action,
        }
    }

    #[test]
    fn playback_required_actions_reject_missing_unfinished_and_wrong_scope_tokens() {
        for action in [
            ReviewAction::Judge {
                verdict: Verdict::Yes,
            },
            ReviewAction::Judge {
                verdict: Verdict::No,
            },
            ReviewAction::CorrectAndConfirm {
                text: "교정".into(),
            },
            ReviewAction::ConfirmCorrection,
        ] {
            let playbacks = Playbacks::default();
            let request = request(action);
            assert!(playbacks.authorize(&request, None).is_err());
            let token = playbacks.begin("segment", 2).unwrap();
            assert!(playbacks.authorize(&request, Some(&token)).is_err());
            playbacks.ended(&token).unwrap();
            let authorized = playbacks.authorize(&request, Some(&token)).unwrap();
            assert_eq!(authorized.request(), &request);
            let stale = ReviewRequest {
                expected_revision: 3,
                ..request.clone()
            };
            assert!(playbacks.authorize(&stale, Some(&token)).is_err());
            let other = ReviewRequest {
                segment_id: "other".into(),
                ..request
            };
            assert!(playbacks.authorize(&other, Some(&token)).is_err());
        }
    }

    #[test]
    fn edits_without_playback_requirements_keep_their_proof_free_path() {
        let playbacks = Playbacks::default();
        for action in [
            ReviewAction::SaveDraft {
                text: "초안".into(),
            },
            ReviewAction::DismissErrorMark,
            ReviewAction::Invalidate,
        ] {
            let request = request(action);
            assert_eq!(
                playbacks.authorize(&request, None).unwrap().request(),
                &request
            );
        }
        assert!(playbacks.ended("unknown").is_err());
    }
}
