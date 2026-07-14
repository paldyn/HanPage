# PR #2260 검토 — rhwp-vscode 배율 메뉴 통합 (planet6897, closes #2259)

- 검토일: 2026-07-14 / base: devel / 9파일 +766/−28 (소스 3: provider·viewer·CHANGELOG,
  나머지는 내부 규약 문서) / CI 12 green / BEHIND (merge 시 #2257 방식 적용 가능)
- 요지: 상태 표시줄의 보기 배치+배율을 단일 드롭다운으로 통합
  (폭 맞춤 / 쪽 맞춤 / 두 쪽 맞춤 / % 프리셋), ResizeObserver 맞춤 재계산,
  구현 중 결함 2건(레이아웃 재구성 조기 반환, padding 이중 차감) 동반 수정.

## 구조 검토

- `zoomMode(manual/fitWidth/fitPage)` 분리 + `currentZoom` 은 실제 배율
  유지 — 렌더·썸네일 하위 경로 무수정 (파급 최소화 설계 적절).
- 맞춤 기준 = 문서 전체 최대 폭·높이 (쪽 크기 혼합 문서의 배율 요동 방지)
  + 스크롤바 진동 2중 차단(contentRect 기준 + 1% 히스테리시스) — 설계
  요점이 코드와 일치함을 확인.
- CSP 인라인 핸들러 0건, VSCode 테마 변수 사용, `stb-` 접두어 규약 준수.
- 범위 격리: rhwp-vscode 만. studio/브라우저 확장 비접촉.

## 게이트 (로컬)

| 게이트 | 결과 |
|--------|------|
| `npm ci` + `tsc --noEmit` (extension) | 클린 |
| `npm run compile` (webpack production) | 성공 (webview+ext) |
| 인라인 script/핸들러 스캔 | 0건 |

## 잔여 — 실사용 판정 (컨트리뷰터 명시 요청)

rhwp-vscode 는 자동화 하네스가 없어 시각 검증 미완을 컨트리뷰터가 명시.
확장 개발 호스트(F5)에서 시나리오 7건 확인 필요:
①두 쪽 맞춤 가로 스크롤 없음 ②폭 맞춤 세로 스크롤만 ③쪽 맞춤 한 쪽
전체 표시 ④패널 크기 변화 시 배율 추종 ⑤사이드바 토글 재계산
⑥−/+·Ctrl+휠 시 맞춤 해제 ⑦혼합 쪽 크기 문서 배율 진동 없음.

## 판단

구조·게이트 결격 없음. **실사용 판정(F5) 통과 시 merge 수용 권고.**
merge 시 "closes #2259" 로 이슈 자동 close 됨.
