# 최종 보고서 — #1658 페이지네이션 엔진 개선 (라운드 1)

- 마일스톤: M100 / 브랜치: local/task1658 (upstream/devel 기준) / 일자: 2026-06-29
- 성격: 다세션 연구·개선 프로젝트의 **라운드 1**. 실질 수정 1건 landing + 대규모 측정 기반 구축.
- 선행: #1643(검증 도구), #1648(경계 빈 문단), #1653(RCA·통제 하네스, closed). #1658 은 **존속**(잔여 연구).
- 오라클: OLE 한글 PageCount (Windows+한컴) — 모든 변경의 심판.

## 1. 목표
rhwp TypesetEngine 페이지네이션을 OLE 한글 기준에 수렴(정합률↑), **무회귀**.

## 2. 구축한 측정 기반 (재사용 자산)
- `tools/build_page_oracle.py` → `tests/fixtures/render_page_oracle_1658.tsv`:
  hwpdocs 랜덤 452개 한글 PageCount 정답지. **rhwp 실제 정합 97.6%**(controlset 81.5%는 난케이스 집합).
- `tools/render_page_gate.py --fixture <oracle>`: 소형(92)·대형(452) 양 게이트로 수정 전/후 결정론 비교.
- 합격 기준: over↓ & under/일치 무회귀(신규 −1=0) + lib/hwpx_roundtrip 통과.

## 3. LANDED 수정 — 법령 표 행분할 미세 fragment 낭비 페이지
`src/renderer/layout/table_layout.rs` `advance_row_cut`/`advance_row_block_cut` (`tiny_fragment_waste`, +11줄):
- 거대 셀이 페이지를 가로질러 분할될 때, 셀 내용 vpos reset(hard_break_before)이 촘촘하면 잔여공간이
  충분한데도 reset 마다 페이지를 끊어 ≤2줄짜리 낭비 페이지 양산 → over-pagination.
- 현재 fragment 가 ≤2 유닛이고 잔여공간이 tolerance 초과면 그 reset break 를 흡수.
- #1488(가시 문단 사이 reset 을 3 유닛 소비 후 보존, rowbreak-problem-pages.hwpx 회귀 방지)은
  j>start+2 라 무영향.

### 검증
| 항목 | 베이스 | 수정 후 |
|------|--------|---------|
| 별표1(국토부) | 5쪽(Δ+1) | **4쪽 일치** ✅ |
| 별표4(산업통상부) | 33쪽(Δ+8) | **28쪽(Δ+3)** |
| 소형 controlset | 75 | **75 무회귀** |
| 대형 오라클(452) | 441 | **442 (+1)** |
| 신규 −1(under) | — | **0** |
| lib / hwpx_roundtrip | 1984 / 4 | **1984 / 4 passed**(#1488 보존) |
| 소스 변경 | — | table_layout.rs **+11줄**(국소) |

## 4. 조사하여 배제한 접근 (근거 자료)
- **하단 고정 표(vert=Page·valign=Bottom) over** — Phase 0~2, 8종 시도 전부 회귀.
  px-level close-call + 페이지 배정 fixpoint(예약↔앵커 진동). 스케일 저ROI(controlset 집중, 대형 무영향).
  `task_m100_1658_phase1/2.md`.
- **vpos-snap 이식** — Paginator snap 은 증가 전용(max)이라 over 못 고침. 하단표 out-of-flow 는
  단일표 −1 회귀. `phase1`.
- **구조적 reset 판별자** — reset 의 vis_start 혼재 → #1488 과 구별 불가(음성). `task_m100_1658_reset_discriminator.md`.
- **(b) reset-snap** — 안전 tolerance(32px)로는 2~4유닛 orphan 못 닿고, 대형 게이트 442→440 회귀 → 비채택.

## 5. 잔여 (후속 라운드)
- **별표4 Δ+3**: 3~4유닛 orphan = per-page 용량 결손(rhwp 가 한글보다 페이지당 ~2~4유닛 적게 적재;
  pagination_tolerance + px 누적). 페이지수 게이트만으로는 안전 해결 불가 → **시각(클리핑) 검증 인프라
  동반 정밀 작업** 필요(별도 라운드).
- **하단 고정 표 over / under(−1) 7건**: 별도 메커니즘. fixpoint·시각 검증 영역.

## 6. 결론
- 페이지수 정합 **97.6→97.8%**(대형 오라클), 법령 표 행분할 1·2유닛 낭비 페이지를 **무회귀로 실제 수정**
  (별표1 완전 일치, 별표4 Δ+8→Δ+3).
- 남은 over 의 성격(용량 결손, reset 분류 아님)을 규명하고, 안전 ceiling(유닛수 가드 ≤2)을 확정.
- 측정 기반(대형 오라클+양 게이트)과 8+종 실패 분석이 후속 라운드의 토대.
- **#1658 은 존속**(잔여 연구 항목). 본 라운드 수정은 landed·무회귀.

## 7. 산출물
- 소스: `src/renderer/layout/table_layout.rs`(+11)
- 도구/오라클: `tools/build_page_oracle.py`, `tests/fixtures/render_page_oracle_1658.tsv`
- 설계·단계: `tech/task_m100_1658_design.md`, `working/task_m100_1658_phase1/2/3.md`,
  `tech/task_m100_1658_rca_table.md`, `tech/task_m100_1658_rca_rowsplit.md`,
  `tech/task_m100_1658_reset_discriminator.md`
