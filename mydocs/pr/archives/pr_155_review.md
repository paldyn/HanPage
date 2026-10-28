# PR #155 리뷰 — 최신 upstream 0.8.7 통합 (#154)

## 판정

**승인·병합 완료.** 검토 head `bfd3a242185cb7d31435bc307dcb12f748d13448` 의 GitHub CI 34 성공·6 건너뜀·CodeQL 1 실패(아래),
MERGEABLE 을 확인하고 2026-10-28 작업지시자 지시("너의 추천대로하자")로 squash 병합했다
(merge `4844857b6db82797b4852f9e65213c7c5a0072a6`, 트리 `9d8058df` = 검토 head 트리). #154 는 병합으로 닫혔다.

## 접수 정보

| 항목 | 값 |
| --- | --- |
| PR·작성자·base | [#155](https://github.com/paldyn/HanPage/pull/155) / enxec / paldyn/HanPage devel |
| 고정 base | `9b4f20a69c5eec46ce8b3f2235b8ecc0b93375a0` |
| upstream 고정점 | `faa885e8b54ffc7cdf4196cd43a3103f362499a2` (지난 `02530b9ed`) |
| 커밋 | 분할 push 4개(`5f6f50d01` 소스 0.074 GiB, `2b6d5dd21` PDF 0.194, `94717699` 문서 1.165, `dc5a2dcd9` 문서 0.896) + CI 보정 `bfd3a2421` |
| 관련 이슈 | [#154](https://github.com/paldyn/HanPage/issues/154) |
| 경로 | maintainer 통합 작업, 작업지시자 승인 "응 고고"(2026-10-27) |

## 구현 주장과 검증

[결과 보고](../../report/task_m100_hp154_report.md)를 정본으로 한다. 요약:

| 주장 | 근거 | 판정 |
| --- | --- | --- |
| 엔진은 upstream 그대로 | `src/`·`crates/`·`Cargo.lock` upstream 대비 diff 0, `Cargo.toml` 저장소 URL 한 줄, `tests/` HanPage 3경로 | 충족 |
| HanPage 델타 보존 | 512경로 blob·mode 일치, 3-way 33경로(수동 5), upstream 삭제 475경로 미복원 | 충족 |
| 의미 검토 보정 9건 | 영역 5곳 독립 검토·반박 검증, 각 보정의 회귀·계약 테스트 | 충족(늦은 CDN 글꼴은 수정 전 FAIL 확인) |
| 큰 PR 정책 검사 ENOBUFS 보정 | 커밋된 head 에서 실패 재현 → 64 MiB 버퍼 → manifest `--check` 통과, 계약 23/23, CI Lint 통과 | 충족 |
| 로컬 회귀 | Rust lint 묶음, nextest 11,707/0(신규 samples 212 보안 스윕), Studio 2,603/0, E2E 140·2,666·13, 웹/Desktop 빌드, Desktop crate | 충족 |
| Desktop 번들 | dispatch run 40469070576 양 플랫폼 성공, macOS 서명·공증 검증 통과(head `dc5a2dcd9`; 이후 변경은 CI 스크립트·문서) | 충족 |
| 실제 Desktop 앱 OS 대화상자·PDF 열기/HWPX 저장 확장자·macOS 편집 메뉴 | 단위 테스트·대역만 | 미검증 |

## 조판 원칙·시각 증적

비해당 — HanPage 는 조판·렌더링 코드를 바꾸지 않았고 엔진은 upstream 트리와 같다. upstream 각 PR 의 시각 증적은 upstream 기록에 있다.

## 실제 CI

head `bfd3a2421` 기준 34 성공·6 건너뜀. Lint·Build & Test·Native Skia·Canvas visual diff·Frontend package gates 성공.
건너뜀: Full Gym benchmark validation, Workflow promotion preflight, Frontend unit gates, release-operations-contracts,
WASM Build, cancel-stale-runs. 첫 head `dc5a2dcd9` 의 Lint 실패(ENOBUFS)는 위 보정으로 해소.

CodeQL 은 `crates/rhwp-password-crypto` 의 경고 7건(#53·#54·#55 테스트 고정 비밀번호, #57·#58 고정 키, #59 SHA-256
비밀번호 키 유도, #60 DES)을 새 경고로 집계했다. HWP3·HWP5·HWPX 비밀번호 형식 호환에 필요한 upstream 코드라 수정 대상이
아니다. 사유를 남겨 닫으려 했으나 보안 설정 변경 권한이 없어 작업지시자 처리로 남겼다.

## 병합 후

- 통합 작업 트리는 저장소 밖(`../HanPage-sync-20261027`)에 있었다. 메인 체크아웃 안에 두면 `cargo fmt --all` 이 vendor
  크레이트에서 바깥 `Cargo.toml` 을 workspace 로 오인한다.
- 남은 확인: 다음 Desktop 릴리스에서 PDF 열기·HWPX 저장 확장자·macOS 편집 메뉴 실기기 확인.
- 본인 PR 이며 별도 댓글 지시가 없어 PR 댓글은 게시하지 않았다.
