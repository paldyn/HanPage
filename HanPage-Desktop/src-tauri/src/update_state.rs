use serde::Serialize;
use std::time::{Duration, Instant};

/// 첫 청크는 즉시 알리고, 이후 진행 이벤트만 100ms 간격으로 전달한다.
/// 상태 저장과 단계 전환 알림에는 이 제한을 적용하지 않는다.
pub(crate) fn should_emit_download_progress(last_emit: &mut Option<Instant>, now: Instant) -> bool {
    if last_emit.is_none_or(|previous| now.duration_since(previous) >= Duration::from_millis(100)) {
        *last_emit = Some(now);
        true
    } else {
        false
    }
}

/// 네이티브 업데이트 작업의 현재 단계. 웹뷰에는 같은 값을 조회·이벤트로 전달한다.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub(crate) enum UpdateState {
    Idle,
    Checking,
    Downloading { downloaded: u64, total: Option<u64> },
    Verifying,
    Ready { version: String },
    Applying { version: String },
    UpToDate { version: String },
    Error { message: String, retryable: bool },
}

impl UpdateState {
    fn is_busy(&self) -> bool {
        matches!(
            self,
            Self::Checking | Self::Downloading { .. } | Self::Verifying | Self::Applying { .. }
        )
    }
}

/// 상태와 받아둔 파일을 같은 락으로 보호하여 확인·적용 시작을 원자적으로 결정한다.
pub(crate) struct UpdateSlot<T> {
    pub state: UpdateState,
    pub ready: Option<T>,
}

impl<T> UpdateSlot<T> {
    pub fn new() -> Self {
        Self {
            state: UpdateState::Idle,
            ready: None,
        }
    }

    pub fn begin_check(&mut self) -> bool {
        if self.ready.is_some() || self.state.is_busy() {
            return false;
        }
        self.state = UpdateState::Checking;
        true
    }

    pub fn finish_download(&mut self, ready: T, version: String) {
        self.ready = Some(ready);
        self.state = UpdateState::Ready { version };
    }

    pub fn begin_apply(&mut self, version: String) -> Result<T, String> {
        if self.state.is_busy() {
            return Err("업데이트가 이미 진행 중입니다.".to_string());
        }
        let ready = self
            .ready
            .take()
            .ok_or_else(|| "받아둔 업데이트가 없습니다.".to_string())?;
        self.state = UpdateState::Applying { version };
        Ok(ready)
    }

    pub fn fail_apply(&mut self, ready: T, message: String) {
        self.ready = Some(ready);
        self.state = UpdateState::Error {
            message,
            retryable: true,
        };
    }
}
