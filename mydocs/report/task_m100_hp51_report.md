# Task #51 (M100) — 엔진 재동기화 0.7.18 → 0.8.2 + v0.8.2 릴리스 최종 보고서

- **이슈**: [paldyn/HanPage#51](https://github.com/paldyn/HanPage/issues/51) · 계획서 `plans/task_m100_hp51.md`
- **기준**: upstream/devel `cf5d462dc`(0.8.2) ← 지난 고정점 `dbb2d9ff`(0.7.18). 신규 커밋 2,552 · 4,512파일.

## 단계 요약

- **Stage 1** (`0a2c83a4`): 재구축 + paldyn 레이어 재적용. 3분할(전용 90 / 통째 19 / 수술 25) + 파일별 3-way merge로 충돌 9건 해소.
- **Stage 2** (`1c941bb6`): pdf-large strip(잔존 0) + 전 검증.
- **Stage 3**: 백업(태그 2 + 번들 596MB) → devel force-push → main take-devel merge(트리 일치 0) → Pages 배포 → v0.8.2 서명·공증 릴리스 → 구 v0.7.18 삭제.

## 이번 회차 신규 판별 규칙 (중요)

업스트림이 **파일 260개를 삭제**(web/ 레거시 에디터 53, e2e 디버그, mydocs 정리 199 등)했다.
단순 comm 비교는 이를 "paldyn 전용"으로 오분류해 삭제된 레거시를 되살린다.
→ **판별식**: paldyn 전용 후보 중 *옛 고정점에 존재하던 파일* = 업스트림 파일 → 업스트림이 지운 것 → 복원 금지.
이 규칙으로 350개 후보를 진짜 paldyn 90개 / 업스트림 삭제 260개로 정확히 분리했다.

## 충돌 해소 원칙 적용 사례

| 파일 | 처리 |
|------|------|
| `file.ts` | 업스트림 신구조(openFileViaPicker·암호 저장) 채택 후 데스크톱 네이티브 분기 3종 재이식. `createSaveBlob` 과 동일 암호 규칙의 `exportBytesForNativeSave` 도입 |
| `style-bar.css` | **업스트림이 같은 콤보 잘림을 더 크게 수정**(88/64/160px) → 업스트림 채택(#41 상위 호환) |
| `deploy-pages.yml` | 액션 SHA 핀은 업스트림, gh-pages 배포 방식은 paldyn 유지 |
| `font-loader.ts` | 업스트림은 CanvasKit 폰트 계획 함수만 추가 → **#50 타임아웃 패치와 자동 병합 성공** |
| `CLAUDE.md`/`CONTRIBUTING.md` | 업스트림 전문 채택 + 우리 저장소 URL만 치환 |

## 검증 결과

| 항목 | 결과 |
|------|------|
| 엔진 `src/`·`tests/` diff vs upstream | **0** (Cargo.toml repo URL만) |
| `cargo test --release --lib` | **3241 / 3241** |
| studio 테스트 | **759 / 759** |
| WASM · studio · 데스크톱 빌드 | 전부 성공 (0.8.2) |
| #50 회귀(CDN 차단 A/B) | 통과 — 폰트 지연이 문서 열기를 막지 않음 |
| 웹 라이브(hanpage.paldyn.com) | 0.8.2, 콘솔 에러 0, 다운로드 버튼 정상 |
| 릴리스 자산 | 6종 + latest.json 4플랫폼 |
| Gatekeeper | `accepted / Notarized Developer ID` + stapler OK |

## 신규 발견·수정

업스트림 0.8.2 가 루트 `Cargo.toml` 에 `[workspace]` 도입 → 데스크톱 crate 흡수 실패.
`HanPage-Desktop/src-tauri/Cargo.toml` 에 빈 `[workspace]` 명시로 독립 워크스페이스 유지.
