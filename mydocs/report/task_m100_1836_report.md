# Task #1836 최종 보고 — seoul_0776 라운드트립 회귀: 빈-앵커 host_line_spacing 억제 소스 무관화

## 결론

`typeset.rs format_table` 의 빈-앵커 host_line_spacing 억제(#1147)가 `is_hwpx_source`
게이트라 HWPX 만 억제하고 HWP5 재파스는 미억제(+12px phantom) → 라운드트립 pagination
divergence. 렌더 경로(layout.rs)는 이미 소스 무관 억제이므로, typeset 을 대칭으로
소스 무관화하여 seoul_0776 STRUCT_MISMATCH 를 해소.

## 진단 (이등분 + 계측)

- `git bisect run`: 첫 BAD `e0c471a7`(#1763 clamp). 소거 검증으로 #1763 은 트리거,
  근본은 #1147 게이트로 규명 (clamp 트레이스 A/B 동일, TABLE_DRIFT host_sp A=0/B=12).
- 한글 대조: 넘침 줄 한글 p4 = B(미억제). 단, 렌더는 layout 이 소스 무관 억제로
  833px(p3) 자기정합 — 46px/p4 fidelity 는 별개 선존 축.

## 수정

`typeset.rs`: `is_hwpx_source` 게이트 제거, `is_topbottom_empty_anchor` 소스 무관화.
미사용 `is_hwpx_source: bool` 파라미터 제거(시그니처+호출 2곳). #1147(HWPX) 억제 유지,
#1133(next=table anchor) 보존 조건 유지.

## 검증

- seoul_0776: STRUCT_MISMATCH 646.65 → **PASS 0.00**
- issue_1133_nested_table_valign / svg_snapshot 골든: 통과
- big_hwpx 2,500 render-diff: 직전 스택(1841) 대비 **회귀 0 / 개선 11** (seoul_0776 STRUCT→PASS 포함; 나머지는 최신 devel merge + 본 수정 기여)
- big_hwp 2,500 네이티브 render-diff: **회귀 0 / 개선 2**, PASS 2494 / STRUCT 0 (본 수정이 HWP5 억제로 전환하는 핵심 게이트 — 네이티브 회귀 없음 확인)

## 비고

- 트리거였던 #1763 clamp 는 정상 동작(한글 정합 개선)이라 불변 유지.
- 계측 도구(RHWP_TABLE_DRIFT host_sp, RHWP_FLOW_DBG cur_h)로 pi=20 12px divergence 확인.
