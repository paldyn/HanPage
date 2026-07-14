# Task #26 Stage 3 완료 보고서 — CI 서명·매니페스트 (desktop-release.yml)

- **이슈**: [paldyn/HanPage#26](https://github.com/paldyn/HanPage/issues/26) (M100)
- **단계**: Stage 3 / 4 · **계획서**: `plans/task_m100_hp26_impl.md` §5
- **일자**: 2026-06-09

## 1. 단계 목표

`desktop-release.yml` tauri-action 스텝에 서명 시크릿 env 주입 → 릴리스 시 서명·`latest.json` 자동 생성·첨부.

## 2. 변경 내용

`.github/workflows/desktop-release.yml` — tauri-action 스텝 `env:`에 2개 추가:
```yaml
env:
  GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
  TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
```

## 3. 동작 (tauri-action 자동 처리)

- `tauri.conf.json`의 `createUpdaterArtifacts: true`(Stage 1) + 위 서명 env →
  `tauri-apps/tauri-action@v0`이 **서명된 updater 아티팩트 + `latest.json` 생성 + 릴리스 첨부**를 자동 수행. 추가 스크립트 불필요.
- 엔드포인트(`.../releases/latest/download/latest.json`)가 이 `latest.json`을 가리킴 → 앱이 확인.
- **graceful**: 시크릿 미설정(fork·외부 기여자) 시 updater 아티팩트만 생략, 일반 `dmg`/`nsis` 번들은 정상 생성(빌드 안 깨짐).

## 4. 검증

| 항목 | 결과 |
|------|------|
| 워크플로 YAML 유효 | ✅ (ruby YAML 파싱) |
| tauri-action env 키 | **3개**(GITHUB_TOKEN + 서명 2개) ✅ |
| 시크릿 등록 | `TAURI_SIGNING_PRIVATE_KEY`·`_PASSWORD` 2개 ✅ |
| 개인키 저장소 추적 | 0 (env는 시크릿 참조만) ✅ |

> 실제 서명·매니페스트 생성은 **태그 릴리스 시점**(`hanpage-desktop-v*`) 또는 dispatch 테스트 빌드에서 동작. 본 단계는 CI 배선까지(실제 릴리스 발행은 비범위).

## 5. 다음 단계

- **Stage 4** — 검증(모의 `latest.json`로 체크 흐름 가능 범위) + 문서(시크릿/키 백업 가이드·부트스트랩·릴리스 절차) + 최종 보고.
