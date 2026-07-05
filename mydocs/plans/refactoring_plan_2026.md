# rhwp 리팩토링 마스터 플랜 (2026, 0.8/v1.0) — **v2 (리뷰 취합 확정안)**

- 이슈: #1883 (계획 수립) / umbrella: #1582 / 작성일: 2026-07-04, v2 개정: 2026-07-05
- 상태: **v2 — @jangster77·@postmelee 리뷰 취합 반영, 작업지시자 확정 대기**
- 거버넌스: **SOLID + 복잡도** (복잡도 공식 측정 = 코드 품질 대시보드 `scripts/metrics.sh`,
  임계값: 파일 1,200줄 / CC 목표 ≤15·경고 >25. SOLID 채점: `manual/solid_scoring_guide.md`)
- v1.1 → v2 변경: 단계 재편 반영(1차=복잡도 해체), **금지 목록 §1 신설**(postmelee 보강 1),
  Phase 0 gate 이원화(보강 2), 성공 기준 산식 고정(보강 3 + jangster77 보완 3),
  PR inventory 라운드 반복(보강 4), PR 단위·설명 규격(보강 5 + jangster77).

## 0. 대원칙 — 한 번에 하지 않는다: 리팩토링 3단계 (작업지시자 확정)

선행 리팩토링 경험(3회 모두 분할 진행: Task 146 → Task 149 → 4차 후속 정리)에 근거해,
**빅뱅 리팩토링 대신 3개의 순차 리팩토링 단계**로 나눈다. **작업지시자 재편(2026-07-04)에
따라 1차 = 복잡도 해체**이며, Provenance/Profile 분리는 2차로 이동했다:

| 리팩토링 단계 | = 본 계획 Phase | 성격 | 상태 |
|---|---|---|---|
| **1차** | Phase C (복잡도 해체 — 분기 비접촉 한정) | 중위험을 §1 금지 목록으로 저위험화 | **진행 중** — 라운드 1(#1904)·2(#1925) 완료 |
| **2차** | Phase P (SourceProvenance + LayoutCompatibilityProfile) | 저위험 — behavior 불변 소유권 분리 | 대기 |
| **3차** | Phase D (Document 축 분리) | 고위험 — roundtrip 영향 | 대기 |

**단계-관문(stage-gate) 규칙** (v1.1 §0 유지):
1. 각 단계·라운드는 **완료 → 게이트 전수 통과 → 대시보드 스냅샷 → 중간 재평가 →
   작업지시자 승인** 후에만 다음에 착수한다. 단계 간 병행·중첩 금지.
2. 각 단계는 **독립적으로 종료 가능한 지점** — 다음 단계를 하지 않아도 devel이 안정 상태.
3. 중간 재평가에서 위험이 크면 다음 단계를 축소·연기·중단할 수 있다.
4. Phase 0(baseline freeze)은 1차의 전제, Phase 4(6차 리뷰)는 전체의 마감.

## 1. 금지 목록 — 1차에서 "아직 하면 안 되는 것" (postmelee 보강 1, 기계적 guardrail)

1차(복잡도 해체)의 허용 범위는 **"소스-포맷 분기 비접촉 순수 추출"로 고정**한다.
원래 안전 순서(#1582/v1.1: Provenance/Profile을 먼저 세워 분기를 policy 뒤로 모은 뒤 해체)를
조정한 결정이므로, 아래를 **PR 체크 기준**으로 기계 적용한다:

- **금지 A — 분기 이산**: `is_hwp3_variant` / `is_hwpx_source` / `is_hwp3_origin` /
  provenance 계열 판단을 **새 helper·새 함수로 옮기거나 복제하지 않는다**. 추출 경계는
  이 분기들이 **caller에 남도록** 긋는다 (실행 전례: #1925 D 블록 제외, #1904 미주 블록의
  분기 0 확인 선행).
- **금지 B — 분기 밀집 구간 해체**: 아래 함수/구간은 Provenance/Profile(2차) 이후로 보류:
  - `typeset.rs` 소스분기 밀집 구간 (HWP3 변형 흐름 계산 계열)
  - `paragraph_layout.rs` D 블록(tac 개체 라인, hwp3_variant 스케일 분기) 및
    `hwp3_variant_flow_spacing_before` 호출 경계
  - `layout.rs` / `height_cursor.rs` 의 HWP3-origin 예외 경로 (#1912의 skip_spacing_before_prededuct 등)
- **금지 C — 의미 변경 동반 추출**: 추출과 동작 수정(버그 픽스 포함)을 같은 커밋/PR에
  섞지 않는다. 게이트 무변동이 판정 불가능해진다.
- 판단이 애매한 구간은 **보류가 기본값** — 라운드 재평가에서 작업지시자 결정.

## 2. 현황 재진단 요약 (상세: `mydocs/tech/task_m100_1883_diagnosis.md` + 라운드 실적)

- 대시보드 영점(2026-07-04): 1,200줄 초과 .rs 70개(잠정 모집단), CC>25 80개(최대 288).
- **1차 실적**: 라운드 1(#1904) `typeset_section_with_variant` CC 282→104 ·
  object_ops 8모듈 분할, 라운드 2(#1925) `layout_composed_paragraph` CC 288→226.
  행동 회귀 0. 현 최대 CC 234(`parse_paragraph_list`, HWP3), CC>25 = 82개
  (**분할 과도기 +1~2/라운드 실증** — §5 산식의 예외 정책 근거).

## 3. Phase 0 — Baseline Freeze (완료분 + 보강)

**완료 (라운드 1, manifest `tech/task_m100_1904_baseline_manifest.md`, 기준 00014ecf)**:
golden SVG·페이지 오라클·roundtrip baseline·OVR baseline 5샘플·대시보드 영점.

**보강 (postmelee 보강 2 — 1차 실행 gate와 전체 gate의 이원화)**:

| 자산 | 등급 | 근거 |
|---|---|---|
| 전체 테스트 + OVR + golden + roundtrip | **1차 실행 gate** (매 추출 PR 강제) | 렌더/layout 행동 고정 |
| public Rust API 표면 목록 | **advisory** (전체 리팩토링 gate — 2·3차 진입 전 스냅샷 필수) | jangster77 보완 4: advisory 시작 |
| WASM JSON schema 스냅샷 | advisory → **3차(Document 축) 진입 전 gate 승격** | wasm_api 반환 계약 |
| CLI output 계약 (39 명령 대표 출력) | advisory → 3차 진입 전 gate 승격 | 직렬화/CLI 영향 |

1차는 렌더 산출물 gate만으로 충분(추출은 관측 가능 행동 불변)하나, 2차(Provenance)부터
API 표면 이동이 생기므로 **2차 착수 전 advisory 3종의 스냅샷 생성을 관문 조건**으로 한다.

## 4. PR inventory — 라운드 착수 시마다 반복 (postmelee 보강 4)

- baseline commit 직전 **render/layout 계열 열린 PR을 merge/hold/rebase 대기로 분류 후
  freeze 진입** — 라운드 1에서 1회 실행(당시 0건)했고, **매 라운드 착수 시 반복**한다
  (라운드 2 실행분: #1923/#1924 serializer 계열 = 비접촉 확인 후 진행).
- **#1900 (render seam RFC, feature=jshook)**: baseline 전 포함하지 않고 **baseline 후
  별도 실험으로 분류** — 렌더 경로 hook seam은 무변동 판정과 간섭하므로 1차 완료 후 재평가.
- 진행 중 신규 렌더링 PR은 hold 라벨 운용 (CONTRIBUTING 공지).

## 5. 성공 기준 — 산식 v2 고정 (jangster77 보완 3 + postmelee 보강 3)

**모집단 정의 (고정)**:
- **추적 지표 모집단** = `src/**/*.rs` 중 **runtime 로직**. 다음은 제외:
  ① 자동 생성 데이터(`font_metrics_data.rs`, `johab_map.rs`, `pua_oldhangul.rs`)
  ② 테스트 코드(`#[cfg(test)]` 인라인, `tests/`, `wasm_api/tests.rs`)
  ③ 진단 도구(`diagnostics/`) ④ CLI dispatch(`main.rs`의 명령 분기부)
- 대시보드 카드(전체 파일, studio .ts 포함)는 **참고 병기** — 목표치 판정에 쓰지 않는다.
  (영점 "70 vs 80" 혼선의 재발 방지)

**CC 목표의 예외 정책 (postmelee: 숫자 맞추기 방지)**:
- `CC>25 = 0`(v1.0)은 **예외 심사제**: 파서 상태 머신·포맷 스펙 직결 분기 등 "쪼개면
  오히려 스펙 대응이 흩어지는" 함수는 작업지시자 승인으로 예외 등재(사유·상한 명기).
  예외 목록은 본 문서 부록으로 관리한다.
- **분할 과도기 허용**: 추출 직후 CC>25 개수 +1~2 는 실패가 아니다(라운드 1·2 실증).
  판정은 **라운드 재평가 시점의 추세**(최대 CC·상위 10 합계)로 한다.

**목표치 (v1.1 §6 유지 + 명시)**:

| 지표 (모집단 = 위 정의) | 영점 | 1차 완료 목표 | v1.0 목표 |
|---|---|---|---|
| 최대 CC | 288 | **< 100** | — |
| CC>25 함수 | 80 | 상위 10 해소 | **0 (예외 등재분 제외)** |
| 1,200줄 초과 | 70 | 로직 파일 상위 7 해소 | ≤ 20 (제외군 0) |
| SOLID (6차 리뷰) | 4차 8.9(×2=89) | — | ≥ 90/100 |
| 행동 회귀 | — | **전 라운드 0건** | 0건 |

## 6. PR(추출) 단위와 설명 규격 (jangster77 + postmelee 보강 5)

- 단위: **한 책임/추출 단위 = 1 PR** (내부 타스크는 1 커밋). "한 giant function = 1 PR" 금지.
- 추출 PR/커밋 설명 필수 3요소:
  1. **분리한 responsibility** (예: "est 사전 폭 추정 패스")
  2. **무변동 behavior gate 목록** (테스트 수치·OVR·golden 등 실행 결과)
  3. **다음 추출 앵커** (남은 구간 중 후속 후보와 보류 사유)
- 의존 폭 임계: 읽기 12개 초과 시 파라미터 struct 도입, mut 다수(≈10↑)면 상태 struct
  설계 선행으로 보류 (실증: `EndnoteFlowState`·`RunEmitState` 이연).

## 7. Phase P — SourceProvenance + LayoutCompatibilityProfile (2차, #1582 합의)

- `is_hwp3_variant`/`is_hwpx_source` 계열 판단의 **소유권만** 내부 policy로 분리, shim 유지,
  observable behavior 불변. §1 금지 목록이 이 단계에서 해제된다.
- 착수 관문: 1차 재평가 승인 + advisory 3종 스냅샷(§3) 생성.
- 게이트: 전체 테스트 + roundtrip/golden/OVR **무변동** (bit-identical 기대).

## 8. Phase D — Document 축 분리 (3차, 고위험) / Phase 4 — 6차 리뷰

v1.1 §Phase 3·4와 동일: `PackagePreservation`/`EmbeddedMediaProjection`/`VisualScene` 분리,
`DocumentCore` 표면 축소는 2차 결과를 보고 범위 재결정. Phase 4 = SOLID 채점 + 대시보드
재측정 + 잔여 부채 목록화.

## 9. Feature Freeze / 일정

- v1.1 §4·§7 유지: Phase 단위 짧은 영역 freeze(renderer/layout·typeset), 그 외 정상 유입.
  0.8 릴리즈 전 Phase 0(완료), 릴리즈 직후 1차 롤링 계속 → 2차. 구체 일자는 0.8 일정 확정 후.

## 부록 A — CC 예외 등재 목록 (§5 예외 심사제)

| 함수 | 사유 | 상한 | 승인 |
|---|---|---|---|
| (등재 없음 — 심사 시 추가) | | | |
