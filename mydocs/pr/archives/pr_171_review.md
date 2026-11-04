# PR #171 리뷰 — Desktop 릴리스 검증 강화 (#170)

## 판정

**승인·병합 완료.** 검토 head `a42c1589ba639698b41c47210156d89be90fe524` 의 GitHub CI 33 성공·5 건너뜀(`Desktop check` macOS·Windows 포함), MERGEABLE·CLEAN 을 확인하고
2026-11-04 작업지시자 승인("응" — CI 가 모두 통과하면 병합, 기록 푸시, 브랜치 정리)으로 squash 병합했다(merge `b04f1bf9bef97306fbfbc561e9fde85f3be03cd7`). 병합 commit tree `9fa8ec5c4f` 가
검토 head 와 같다. #170 은 devel push 워크플로가 닫는다.

## 접수 정보

| 항목 | 값 |
| --- | --- |
| PR·작성자·base | [#171](https://github.com/paldyn/HanPage/pull/171) / enxec / paldyn/HanPage devel |
| 고정 base | `144377b6882eae2b4fcaa6985c1bd4ac80982cbb` |
| 커밋 | `3879a3c95` 구현·수행계획서, `55bb75695` 결과 보고서, `a159d1995` 번들 재시도·verbose, `7fd214510` 공개 릴리스 확인·감사 스크립트 보강, `a42c1589b` 보고서·DMG 원인 |
| 관련 이슈 | [#170](https://github.com/paldyn/HanPage/issues/170) |
| 경로 | maintainer 본인 작업. 0.8.12 공개 전 독립 검증의 후속 제안 → 작업지시자 "필요한거면해"(2026-11-03) |

## 구현 주장과 검증

[결과 보고서](../../report/task_m100_hp170_report.md)를 정본으로 한다.

| 주장 | 근거 | 판정 |
| --- | --- | --- |
| 고정 Team ID·세 `.app`·CDHash·아카이브·DMG 컨테이너 검사가 `macos-15` 에서 통과한다 | 수동 실행 41605141433·41609738093 의 macOS 로그: `The validate action worked!` 3회, `Notarized Developer ID` 3회, DMG 체크섬 VALID, annotation 에 `macOS 미서명` 없음 | 충족 |
| 각 검사가 위반을 잡는다 | 단계 스크립트를 YAML 에서 꺼내 공개된 0.8.12 자산으로 로컬 재현: 음성 경우가 해당 `::error` 로 실패 | 충족(로컬 재현) |
| `macos-15` 이전이 지원 최소 macOS 를 바꾸지 않는다 | 산출 DMG 의 LSMinimumSystemVersion 10.13, minos 11.0(0.8.12 와 같음), SDK 만 14.5 → 15.5 | 충족 |
| 감사 스크립트가 정상 릴리스를 통과시키고 위반만 잡는다 | 공개된 0.8.11·0.8.12 61/61, 음성 26가지, 인자 오류 6가지 | 충족 |
| 태그 빌드의 공개 릴리스 확인·초안 여부·digest 대조 | workflow_dispatch 에서 돌지 않는다. 실제 API·`gh` 대역으로 로컬 재현만 했다 | 미검증(실제 태그 빌드) |
| DMG 단계 재시도 | 실제 실행에서 첫 시도가 통과해 쓰이지 않았다. 설정이 같음은 YAML 로 대조했다 | 미검증(실제 실행) |

독립 반박 검토 3회(에이전트 11·9·4, 중간 이상 발견마다 검증자 2명)에서 서명 없는 재포장 DMG 가 감사를 통과하는 문제(high)와 서명 디코딩, 문서 명령, 태그 전 시험 순서, `macos-14`
은퇴, 공개본과 감사본 대조 누락, 이미 마운트된 DMG, `--release` 인자 무시(medium)를 확인해 모두 반영했다(low 24건 포함). 첫 수동 실행의 `bundle_dmg.sh` 실패는 verbose 로그로
tauri-action 이 CI 에서도 Finder AppleScript 를 실행함을 확인했고, 같은 source 가 다음 실행에서 통과해 일시 실패로 판단했다.

## 조판 원칙·시각 증적

비해당 — workflow·수동 감사 도구·문서만 바꿨다.

## 병합 후

- 남은 확인: 다음 태그 빌드(0.8.13)에서 공개 릴리스 확인의 통과 경로와 초안 여부·digest 대조를 실제 결과로 본다. DMG 단계가 실패하면 재시도와 verbose 로그를 확인한다.
- 원격 브랜치 `claude/desktop-release-hardening`·`claude/desktop-release-0.8.12` 는 작업지시자 승인으로 지운다.
- 본인 PR 이며 별도 댓글 지시가 없어 PR 댓글은 게시하지 않았다.
