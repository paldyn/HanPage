# 최종 결과보고서 — #2019 부동 폼 다수 별지 서식 과분할·조각 렌더

- 이슈: #2019 [Rendering/Layout] / 브랜치: `fix/2019-through-wrap-overlay-vpos`
- 검출: hwpdocs 8차 10k 서베이. 재현: `74312 벤처투자 시행규칙(안).hwpx` (rhwp 81p vs 한글 18p, 4.5×)

## 결과 요약

**74312: 81페이지 → 18페이지 (한글 2022 오라클과 완전 일치) + 서식 렌더 정상화(표 격자·라벨 복원).**
덤으로 서베이 최대 과분할 문서 **3690000(노인복지관 타당성): 310 → 176페이지**(한글 177) 동반 해소.
**무회귀 0건** (300 랜덤 + 80 baseline + 전 테스트 스위트).

## 근본원인 (3층, 계측 확정)

부동 폼(별지 서식)의 stored LINE_SEG vpos 는 텍스트 흐름 좌표가 아니라 **개체의 섹션 절대
위치·높이(≈17p 캔버스)**를 인코딩한다. rhwp 는 이 앵커 문단을 흐름 콘텐츠로 취급해 3경로 과분할:

| 층 | 지점 | 현상 |
|----|------|------|
| ① 높이 | `format_paragraph` (typeset.rs) | stored line_height(=개체높이 51.3mm)를 흐름 예약→오버플로 |
| ② 단나누기 | 단나누기 핸들러 (typeset.rs) | 폼 구분자(빈문단+단나누기+같은 1단 ColumnDef)를 단일단서 페이지로 변환 |
| ③ zone 오프셋 | `process_multicolumn_break` vpos_zone_height (typeset.rs) | 섹션 절대 vpos(2204px)를 zone 오프셋→candidate_offset>page→1단↔2단 전환(71회)마다 새 페이지 |

배제: vpos-reset 트리거(0회), 명시적 쪽나누기(17개=한글 정합), 스타일 브레이크(false).

## 수정 (통합 게이트)

공통 술어 **`para_is_floating_overlay_anchor`**(layout.rs 신설): 빈 텍스트 + 전 컨트롤이 부동
비-TAC (Shape/Picture: 통과·글앞·글뒤 / Table: +어울림). 이 술어로 3경로에서 흐름 footprint 를 0 처리:
- ① 게이트 시 line_heights 를 빈문단 fallback 으로 대체.
- ② 게이트(또는 빈 텍스트 + ColumnDef 단독) 시 단일 단 단나누기 억제.
- ③ 이전 zone 마지막 문단이 게이트 대상이거나 **max_vpos_end 가 본문 높이 초과(누적 vpos 신호)**면
  zone 높이를 `st.current_height`(page-상대 흐름 누적값)로 대체.

수정 파일: `src/renderer/layout.rs`(+헬퍼), `src/renderer/typeset.rs`(①②③).

## 검증

### 시각 (한글 2022 대조)
- 74312 페이지 4 "투자목적회사에 관한 사항" 표 = 라벨·격자 완전 복원, 한글 페이지 5와 일치(export-png).

### 무회귀 (Stage 3)
- **80 baseline**: 변동 2건뿐(74312 81→18, 3690000 310→176 — 둘 다 한글 수렴), 78 불변.
- **300 랜덤(수정 전 pipage 대비)**: 변동 **0건** → 부동 폼 문서에만 국소 작용.
- **MORE 클러스터 44**: 개선 2, 동일 42, **악화 0** (#1937/#1921 표/줄 누적 클래스는 불변).
- `cargo test --lib`: **2143 passed / 0 failed**. `hwpx_roundtrip_baseline`: 4/4.
- **다단 통합 테스트**(process_multicolumn_break 경로): exam_eng_multicolumn·issue_1082/1156/1375/1488 전부 통과.
- `svg_snapshot` 8/0, `opengov_corpus_snapshot` 2/0.

### 회귀테스트
- `tests/issue_2019_floating_form_overpagination.rs`: 74312 페이지수 ≤20 assert.
- 픽스처 `samples/hwpx/issue2019_floating_form_74312.hwpx` (roundtrip PASS, baseline 자동포함).

## 결론

부동 폼 앵커의 흐름 footprint 를 0 으로 통합 처리하여 74312 를 한글 정합(18p)으로 수렴시키고
서식 렌더를 정상화했다. 저장(roundtrip) 무영향, 다단·부동개체 문서 전 회귀 스위트 그린,
무회귀 0건. **#2019 해소.**
