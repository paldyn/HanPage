# Task #51 Stage 2 완료 보고서 — pdf-large strip + 0.8.2 전 검증

## 검증 결과

| 항목 | 결과 |
|------|------|
| pdf-large 히스토리 strip | ✅ 잔존 0 |
| 엔진 `src/`·`tests/` diff vs upstream `cf5d462dc` | ✅ **0** (Cargo.toml repo URL만 paldyn) |
| `cargo build --release` | ✅ rhwp 0.8.2 (4m07s) |
| `cargo test --release --lib` | ✅ **3241 passed / 0 failed** |
| WASM (Docker) | ✅ 5m07s |
| studio `tsc --noEmit` | ✅ |
| studio 빌드 + 테스트 | ✅ **759 / 759** |
| 데스크톱 빌드(0.8.2) | ✅ |
| 데스크톱 lock | ✅ rhwp-desktop 0.8.2, brotli-decompressor **5.0.1 핀 유지** |

## 재적용 확인 (paldyn 레이어)

`desktop-release.yml` APPLE_* 8종 + 툴체인 `@1.93.1` 핀(#4) · `dragDropEnabled:false`(#41) ·
폰트 타임아웃/예산 + 전역 펜딩 큐 + 브리지 재드레인(#50) · 다운로드 버튼(#38) · 데스크톱 Cargo.lock.

## #50 회귀 테스트 (CDN 차단 A/B, 0.8.2 기준)

CDN(jsdelivr) 요청을 영구 미응답으로 막고 실제 사용자 문서를 드롭:
- 부팅 폰트 2건 5초 만료 → "로드 실패(문서 열기는 계속)" → 부팅 완료
- 문서 폰트 7건 중 CDN 2건 만료, 로컬 5건 성공 → `[initDoc] 2 → 7` 정상 진행
- **결론: 0.8.2 에서도 폰트 지연이 문서 열기를 막지 않음(회귀 없음)**

## 신규 발견·수정

업스트림 0.8.2 가 루트 `Cargo.toml` 에 `[workspace]` 를 도입 → 데스크톱 crate 를 멤버로
흡수하려다 실패. `HanPage-Desktop/src-tauri/Cargo.toml` 에 빈 `[workspace]` 를 명시해
독립 워크스페이스로 유지(자체 lock 재현 빌드 보존).
