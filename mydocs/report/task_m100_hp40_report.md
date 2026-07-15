# Task #40 (M100) — 엔진 업스트림 재동기화 0.7.15 → 0.7.18 최종 보고서

- **이슈**: [paldyn/HanPage#40](https://github.com/paldyn/HanPage/issues/40) · 계획서 `plans/task_m100_hp40.md`
- **기준**: upstream/devel `dbb2d9ff` (0.7.18) ← 지난 고정점 `9172e3a2` (0.7.15). 신규 커밋 2,139 · 3,465파일.

## 단계 요약

- **Stage 1** (`78458523`+보정 `d5b492b7`): 재구축 브랜치 + paldyn 레이어 재적용. comm 기반 분리(전용 87 / 통째 25 / 수술 23) + 3-way merge로 충돌 12건 해소. file.ts는 업스트림 신형 저장 흐름(save-target/HML)에 데스크톱 네이티브 분기 재이식. .gitignore 업스트림 루트 lock 추적 방침 채택. 오분류 잔재(object_ops.rs — 업스트림 모듈 분해의 옛 파일) 제거 + 전용 목록 전수 검사.
- **Stage 2** (`eed23155`): pdf-large filter-repo strip(잔존 0). 검증 — 엔진 src diff=0, cargo test lib **2214/2214**, WASM(Docker), studio 빌드, 데스크톱 check 전부 통과. 데스크톱 lock 0.7.18(brotli-decompressor 5.0.1 핀 유지).
- **Stage 3**: 백업(태그 2 + 번들 320MB) → devel force-push(GH008 없음) → main take-devel merge(트리 일치 검증) → Pages 배포(웹 0.7.18 라이브) → `hanpage-desktop-v0.7.18` 릴리스.

## 릴리스 이슈와 해결

1. **Apple 공증 403** (약관 갱신 미동의): 작업지시자가 developer.apple.com에서 신규 약관 동의 후 해소.
2. **Task #41 동반 수정** (macOS 드래그&드롭 + 콤보 잘림): 수정 반영 후 태그 이동 재빌드.

## 최종 검증

- v0.7.18 자산 6종 완비, latest.json 4플랫폼(darwin 포함).
- Gatekeeper: `accepted / Notarized Developer ID` + stapler OK (릴리스 DMG 실측).
- 구 v0.7.15 릴리스+태그 삭제 (latest = v0.7.18).
