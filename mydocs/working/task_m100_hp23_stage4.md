# Task #23 Stage 4 완료 보고서 — 검증

- **이슈**: [paldyn/HanPage#23](https://github.com/paldyn/HanPage/issues/23) (M100)
- **단계**: Stage 4 / 5
- **계획서**: `mydocs/plans/task_m100_hp23_impl.md` §3 Stage 4 · §4 검증 상세
- **일자**: 2026-06-02
- **검증 대상**: `local/task23-rebase` @ `5e921ade` (기준 upstream `f6ffe9d6`)

## 1. 단계 목표 (구현 계획서 §3·§4)

> cargo build/test(네이티브) · **엔진 src diff vs 고정 upstream = 0** · 누락 33 PR 흡수 확인 · 브랜딩/데스크톱/무손실. (studio 빌드 = Stage 3에서 이월)

## 2. 검증 결과 요약

| 검증 항목 | 결과 |
|----------|------|
| 엔진 `src/` diff vs upstream `f6ffe9d6` | **0 파일** ✅ |
| `cargo build`(네이티브) | ✅ rhwp v0.7.13, 9.05s |
| `cargo test`(네이티브) | ✅ **1933 passed / 0 failed** (전 스위트 ok) |
| studio `tsc` 타입체크 | 변경 파일 **0 에러** ✅ (잔여 4건 = stale pkg drift, 후술) |
| 누락 33 PR 흡수 | ✅ 대표 Task #1220/1221/1222/1228 + `cell_path_json` |
| paldyn 레이어 무손실 | ✅ (§5) |
| 엔진 식별자 보존 | ✅ crate rhwp·@rhwp/editor·edwardkim |
| 시크릿/개인키 잔존 | ✅ 추적 0 |

## 3. 엔진 동기화 증명

- `git diff f6ffe9d6 local/task23-rebase -- src/` = **0** → 엔진은 upstream/devel 고정 tip과 **byte-identical**.
- `cargo build` 성공 + `cargo test` **1933 passed / 0 failed** → 흡수한 upstream 엔진이 로컬 네이티브에서 健全(엔진 무결성 역검증).
- triage(§1-1, 엔진fix 10건 전부 DROP)의 타당성 = 엔진 diff=0이 역증명(fork 엔진fix가 upstream에 이미 반영됐으므로 재적용 불필요했음이 빌드·테스트 통과로 확인).

## 4. studio 타입체크 — 데스크톱 글루 검증

`rhwp-studio`에서 `tsc --noEmit` 실행:
- **내 변경 파일(`main.ts` 배선·`desktop-bridge.ts`·`commands/file.ts`·`about-dialog.ts`·`vite.config.ts`) → 타입 에러 0** ✅. Stage 3 데스크톱 글루 배선·take-main 코드가 upstream 신버전 위에서 정상 타입체크됨.
- 잔여 에러 4건은 전부 `src/core/wasm-bridge.ts`(내가 미변경, upstream 현행)의 `copyControl`/`exportControlHtml`/`getControlImageData`/`getControlImageMime` 4-arg 호출.
  - **원인**: 로컬 `pkg/rhwp.d.ts`(gitignore 빌드 산출물)가 **구 엔진 기준**으로 3-arg 선언. 현재 엔진 src(`wasm_api.rs:5341`)는 `cell_path_json` 4번째 인자 보유(검증: `test_clipboard_copy_control_cell_path_json_arg` 존재).
  - **해소**: WASM을 현재 엔진으로 재생성(wasm-pack, CI `deploy-pages.yml`/Docker)하면 `rhwp.d.ts`가 4-arg로 갱신 → 4건 소멸. `pkg/`는 gitignore라 devel 산출물에 포함되지 않고 CI가 매 배포 시 재빌드.
  - 즉 **재베이스가 유발한 문제가 아니라**, 구 로컬 pkg와 신 엔진의 drift이며 CI 정상 경로에서 자동 해소.

> 데스크톱 풀빌드(Tauri 번들)·WASM 풀빌드는 계획서 §4대로 CI/릴리스 시점(비범위). 본 단계는 글루 타입 정합성까지 검증.

## 5. 무손실 검증 (paldyn 레이어 완전성)

| 항목 | 상태 |
|------|------|
| `HanPage-Desktop/` tracked | **28파일** ✅ |
| `rhwp-studio/public/CNAME` | `hanpage.paldyn.com` ✅ |
| `rhwp-studio/public/LICENSE` | 존재 ✅ |
| `web/fonts/{LICENSES.md,OFL.txt}` | 둘 다 존재 ✅ |
| H-마크 로고(`assets/logo/`) | 8파일 ✅ |
| studio 데스크톱 글루(`desktop-bridge.ts`) + `desktop-release.yml` | 존재 ✅ |
| Pages 격리(`deploy-pages.yml` paths-ignore `HanPage-Desktop/**`) | 존재 ✅ |
| HanPage / hanpage.paldyn.com 서비스 브랜드 | 22 / 9파일 ✅ |
| crate `rhwp`·`@rhwp/editor`·publisher `edwardkim` | 보존 ✅ |
| 추적 시크릿/개인키(`.pem/.key/.p12/.env` 등) | **0** ✅ |

> mydocs paldyn 문서는 Stage 5에서 product 브랜치로 일괄 반영(§2-4) — 현 단계 미해당.

## 6. 누락 33 PR 흡수 (대표 확인)

| 대표 | rebase 히스토리 | origin/main | 판정 |
|------|----------------|-------------|------|
| Task #1221 | 3건 | 0 | ✅ 흡수 |
| Task #1222(문단 id 전역 유니크) | 2건 | 0 | ✅ 흡수 |
| Task #1220(wrap=Square 커서) | 3건 | 0 | ✅ 흡수 |
| Task #1228(표 셀 그림 복사) | 2건 | 0 | ✅ 흡수 |
| `cell_path_json`(src/wasm_api.rs) | 24건 | 0 | ✅ 흡수 |

## 7. 다음 단계

- **Stage 5** — 최종 보고서 + orders 갱신 → **승인** → 백업 태그 `backup/devel-pre-task23` 생성 → `devel` force-push 후 검증.
  - mydocs paldyn 문서 product 브랜치 반영(§2-4) 포함.
  - main은 본 작업 비범위(후속 릴리스 PR).
- **승인 대기** — 본 보고서 승인 후 Stage 5 착수.
