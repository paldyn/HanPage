#[path = "../../HanPage-Desktop/src-tauri/src/update_state.rs"]
mod update_state;

use std::time::{Duration, Instant};
use update_state::{should_emit_download_progress, UpdateSlot, UpdateState};

#[test]
fn download_progress_emits_first_chunk_then_at_most_every_hundred_milliseconds() {
    let start = Instant::now();
    let mut last_emit = None;
    assert!(should_emit_download_progress(&mut last_emit, start));
    assert!(!should_emit_download_progress(
        &mut last_emit,
        start + Duration::from_millis(99)
    ));
    assert!(should_emit_download_progress(
        &mut last_emit,
        start + Duration::from_millis(100)
    ));
    assert!(!should_emit_download_progress(
        &mut last_emit,
        start + Duration::from_millis(199)
    ));
    assert!(should_emit_download_progress(
        &mut last_emit,
        start + Duration::from_millis(200)
    ));
}

#[test]
fn update_check_claim_prevents_duplicate_startup_and_manual_checks() {
    let mut slot = UpdateSlot::<Vec<u8>>::new();
    assert!(slot.begin_check());
    assert_eq!(slot.state, UpdateState::Checking);
    assert!(!slot.begin_check());
}

#[test]
fn every_busy_phase_blocks_check_and_apply_without_consuming_ready_bytes() {
    for state in [
        UpdateState::Checking,
        UpdateState::Downloading {
            downloaded: 24,
            total: Some(48),
        },
        UpdateState::Verifying,
        UpdateState::Applying {
            version: "0.8.8".to_string(),
        },
    ] {
        let mut slot = UpdateSlot {
            state: state.clone(),
            ready: Some(vec![1, 2, 3]),
        };
        assert!(!slot.begin_check());
        assert!(slot.begin_apply("0.8.8".to_string()).is_err());
        assert_eq!(slot.state, state);
        assert_eq!(slot.ready, Some(vec![1, 2, 3]));
    }
}

#[test]
fn deferred_download_is_reused_and_only_one_apply_can_take_it() {
    let mut slot = UpdateSlot::new();
    slot.finish_download(vec![11, 22, 33], "0.8.8".to_string());
    assert!(
        !slot.begin_check(),
        "나중에 이후 재확인은 받은 파일을 버리지 않는다"
    );
    assert_eq!(
        slot.begin_apply("0.8.8".to_string()).unwrap(),
        vec![11, 22, 33]
    );
    assert_eq!(
        slot.state,
        UpdateState::Applying {
            version: "0.8.8".to_string()
        }
    );
    assert!(
        !slot.begin_check(),
        "적용 도중 새 다운로드를 시작하면 안 된다"
    );
    assert!(slot.begin_apply("0.8.8".to_string()).is_err());
}

#[test]
fn failed_apply_restores_the_same_bytes_and_allows_direct_retry() {
    let mut slot = UpdateSlot::new();
    slot.finish_download(vec![11, 22, 33], "0.8.8".to_string());
    let bytes = slot.begin_apply("0.8.8".to_string()).unwrap();
    slot.fail_apply(bytes, "권한 요청이 취소되었습니다.".to_string());
    assert_eq!(
        slot.state,
        UpdateState::Error {
            message: "권한 요청이 취소되었습니다.".to_string(),
            retryable: true,
        }
    );
    assert!(
        !slot.begin_check(),
        "적용 실패 뒤에는 이미 검증된 다운로드를 유지한다"
    );
    assert_eq!(
        slot.begin_apply("0.8.8".to_string()).unwrap(),
        vec![11, 22, 33]
    );
}

#[test]
fn failed_download_can_be_checked_again_but_cannot_be_applied() {
    let mut slot = UpdateSlot::<Vec<u8>>::new();
    slot.state = UpdateState::Error {
        message: "네트워크 연결 없음".to_string(),
        retryable: false,
    };
    assert!(slot.begin_apply("0.8.8".to_string()).is_err());
    assert!(slot.begin_check());
    assert_eq!(slot.state, UpdateState::Checking);
}

#[test]
fn update_status_payload_preserves_download_bytes_and_exposes_honest_phases() {
    for (state, expected) in [
        (
            UpdateState::Downloading {
                downloaded: 24,
                total: None,
            },
            serde_json::json!({ "state": "downloading", "downloaded": 24, "total": null }),
        ),
        (
            UpdateState::Verifying,
            serde_json::json!({ "state": "verifying" }),
        ),
        (
            UpdateState::Applying {
                version: "0.8.8".to_string(),
            },
            serde_json::json!({ "state": "applying", "version": "0.8.8" }),
        ),
        (
            UpdateState::Error {
                message: "취소됨".to_string(),
                retryable: true,
            },
            serde_json::json!({ "state": "error", "message": "취소됨", "retryable": true }),
        ),
        (
            UpdateState::UpToDate {
                version: "0.8.7".to_string(),
            },
            serde_json::json!({ "state": "upToDate", "version": "0.8.7" }),
        ),
    ] {
        assert_eq!(serde_json::to_value(state).unwrap(), expected);
    }
}
