# 시각 검증 인프라 — 본문 클리핑 검출기 (`tools/detect_table_clipping.py`)

- 도입: #1658 라운드1 / 목적: 페이지네이션 용량 정합 작업의 **클리핑 회귀**를 페이지수 게이트와
  독립적으로 검출.

## 왜 필요한가
페이지수 게이트(`render_page_gate`)는 페이지 **개수**만 본다. 그러나 페이지당 용량을 늘리는 변경은
표/텍스트가 본문 영역(body) 아래로 흘러 **body-clip 에 잘려 보이지 않는(데이터 손실) 클리핑**을
유발할 수 있고, 이는 페이지수로는 검출되지 않는다. 본 도구가 그 시각 회귀를 잡는다.

## 원리
rhwp SVG 는 **transform 없는 절대좌표**. 본문 영역은
`<clipPath id="body-clip-N"><rect y h .../></clipPath>` 로 정의되고, 같은 N 의
`<g clip-path="url(#body-clip-N)">` 안 콘텐츠가 그 영역에 그려진다.
→ 그룹 내 요소(text/rect/line) 하단 Y 가 `body_bottom(=rect.y+h) + eps` 를 넘으면 **클리핑**으로 판정.

## 사용
```
# 단일/다중 파일
python tools/detect_table_clipping.py <file.hwp|hwpx> ... --exe <rhwp 바이너리>
# 배치 + 샘플
python tools/detect_table_clipping.py --batch <폴더> --sample N --seed S --exe <바이너리>
```
- `--eps`(기본 1.0px): 허용 overflow. 출력 `CLIP {clipped}/{pages}p max_overflow={px} {파일}`.
- 종료코드: 클리핑 1건↑ → 1 (게이트로 사용 가능).
- 주의: `--exe` 는 **절대경로** 권장(상대경로 CreateProcess 실패 회피).

## 검증 (도입 시)
- `법무부 별표1`(주차장법): 클리핑 0 (정상).
- `산업통상부 별표4`(LPG): **CLIP 1/28p max_overflow=23.5px** — 거대 표 fragment 가 본문 23.5px 초과.
  - **upstream(무수정)도 동일(1/33p, 23.5px)** → 행분할 수정과 무관한 **기존 클리핑**(렌더 측 fragment
    높이 초과). per-page 용량 정합 후속 작업의 대상.
- hwpdocs 랜덤 25 표본: 클리핑 0 (false-positive 없음, 클리핑은 대형표 한정 희귀).

## 후속 활용
- per-page 용량 정합(별표4 Δ+3 등) 변경 시: render_page_gate(페이지수) + 본 도구(클리핑) **양쪽**으로
  검증해야 안전. 용량을 늘려 페이지수를 줄이되 클리핑(max_overflow)이 증가하지 않아야 한다.
- 향후 확장: 한글 PDF(`pdf/`) 대비 픽셀 diff(SVG→PNG vs PDF→PNG)로 시각 충실도 정량화(별도).
