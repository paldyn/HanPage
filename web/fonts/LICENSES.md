# 번들 웹폰트 라이선스 고지 (HanPage)

이 디렉터리(`web/fonts/`, 웹 배포 시 `/fonts/`)에 포함되어 재배포되는
웹폰트의 라이선스 목록이다. 모든 폰트는 **재배포가 명시적으로 허용된** 폰트만
포함한다. 비상업 한정·재배포 금지 폰트(예: 한컴 함초롬체)는 포함하지 않으며,
해당 자형은 OFL 폰트로 폴백 대체한다(`src/core/font-loader.ts`).

## 라이선스별 분류

| 라이선스 | 전문 위치 | 적용 폰트 |
|----------|-----------|-----------|
| SIL Open Font License 1.1 | `OFL.txt` | Noto Sans/Serif KR, 나눔고딕/명조/고딕코딩, 고운바탕/돋움, Pretendard, 스포카 한 산스, D2Coding |
| SIL Open Font License 1.1 | `SourceHanSerifK-OFL.txt` | Source Han Serif K (옛한글 서브셋) |
| GUST Font License (GFL) | 하단 참조 | Latin Modern Math |
| Cafe24 무료 폰트 라이선스 | 하단 참조 | Cafe24 써라운드, Cafe24 슈퍼매직 |
| 행복나눔 무료 배포 | 하단 참조 | 행복고딕 (Happiness Sans) |

OFL 1.1 폰트의 저작권 고지·Reserved Font Name·출처는 `OFL.txt` 상단 목록을 참조한다.

## 비-OFL 폰트 상세

### Latin Modern Math — GUST Font License (GFL)

- 파일: `LatinModernMath-Regular.woff2`
- 저작권: © GUST e-foundry
- 라이선스: GUST Font License (LaTeX Project Public License 1.3c 기반)
- 출처/전문: https://www.gust.org.pl/projects/e-foundry/lm-math
- 조건: 자유 재배포 허용. 수정본은 다른 폰트명을 사용해야 함.

### Cafe24 써라운드 / Cafe24 슈퍼매직 — Cafe24 무료 폰트 라이선스

- 파일: `Cafe24Ssurround-v2.0.woff2`, `Cafe24Supermagic-Regular-v1.0.woff2`
- 저작권: © Cafe24 Corp.
- 출처/전문: https://fonts.cafe24.com
- 조건: 상업적 이용 및 재배포 허용. 폰트 자체의 유료 판매 금지.

### 행복고딕 (Happiness Sans) — 행복나눔 무료 배포

- 파일: `Happiness-Sans-Regular.woff2`, `Happiness-Sans-Bold.woff2`,
  `Happiness-Sans-Title.woff2`, `HappinessSansVF.woff2`
- 저작권: © 행복나눔
- 조건: 무료 배포·상업적 이용 허용. 폰트 자체의 유료 판매 금지.
  (공식 배포처의 라이선스 고지를 따른다.)
