# PR #174 리뷰 — Desktop 릴리스 태그 경로 리허설과 후속 정리 (#172)

## 판정

**병합 완료.** 검토 head `28d9db73ce0d9fcad0a3ae87e66084d5245e5c49` 에서 리허설 2회차와 초안 감사가 통과했다. GitHub CI 는 31 성공·2 실패·5 건너뜀이고, 실패는 이 PR 과 무관한
러너 이미지 문제다(아래 CI). 2026-11-10 작업지시자 결정("응 알아서해줘")으로 사유를 남기고 squash 병합했다(merge `80465772f25074d14a87824d7d0be00e61b2946c`). #172 는 초안 삭제를
확인한 뒤 닫는다(PR 은 `Refs #172`).

## 접수 정보

| 항목 | 값 |
| --- | --- |
| PR·작성자·base | [#174](https://github.com/paldyn/HanPage/pull/174) / enxec / paldyn/HanPage devel |
| 고정 base | `39ca2a7fc8` |
| 커밋 | `0f93f796a` 재시도 단순화·리허설 절차·인증서 만료 경고·감사 `--rehearsal`·계획서·보고서, `28d9db73c` `stapler` 재시도(검증 단계·감사·`xcrun` 대역) |
| 관련 이슈 | [#172](https://github.com/paldyn/HanPage/issues/172), 인증서 갱신 [#173](https://github.com/paldyn/HanPage/issues/173), 닫은 Dependabot PR [#137](https://github.com/paldyn/HanPage/pull/137) |
| 경로 | maintainer 본인 작업. #170 후속 네 가지 → 작업지시자 "이거다하자"(2026-11-04) |

## 구현 주장과 검증

[결과 보고서](../../report/task_m100_hp172_report.md)를 정본으로 한다.

| 주장 | 근거 | 판정 |
| --- | --- | --- |
| `retryAttempts: 1` 로 바꿔도 `tauri build` 가 실패하면 한 번 더 시도한다 | 일회용 브랜치 run 41766893337·41781166794: 강제 실패 뒤 `Attempt 1 failed, retrying...`, 두 번째 시도 성공 | 충족(재시도 배선과 이미지가 붙은 채 남은 상태. 실제 DMG 일시 실패의 복구는 미검증) |
| 공개 릴리스 확인이 릴리스가 없을 때와 초안이 있을 때 통과한다 | 2회차 태그 run 두 job 과 macOS 재실행의 단계 결론, 재실행의 `Found draft release` | 충족 |
| 공개 릴리스가 있으면 멈춘다 | job 토큰의 `releases/tags` 조회가 공개 200·초안 404, 로컬 재현 | 미검증(실제 실행) |
| 검증 단계가 태그 빌드에서 초안 여부와 digest 를 대조한다 | 2회차 첫 실행·재실행 로그의 `초안 자산 대조 통과` 2줄씩 | 충족 |
| 재실행이 같은 초안의 macOS 자산만 바꾼다 | 재실행 전후 자산 id(macOS 3개·`latest.json` 교체, Windows 2개 그대로), 감사 62/62 | 충족 |
| 감사 스크립트 `--rehearsal` 이 리허설 초안을 감사하고 오용을 막는다 | 초안 감사 62/62, 로컬 음성(번호 불일치·실제 릴리스에 사용) | 충족 |
| `stapler` 가 Apple 서버 연결 끊김에 다섯 번까지 시도한다 | 로컬 대역 시험(검증 단계 25가지 재현, 감사 음성·양성), 러너에서 대역 사용(`stapler staple try 1: exit 0`) | 충족(로직). 실제 끊김에서의 복구는 미검증(2회차에 끊김 없음) |
| 인증서 만료 90일 전 경고 | 2회차 두 실행의 annotation, 감사 `warnings` 와 stderr | 충족 |
| 리허설이 공개 릴리스와 `latest.json` 을 건드리지 않는다 | 회차별 스냅샷 전후 비교 | 충족 |
| tauri-action 1.0 을 올리지 않는다 | 1.0.0 소스(`latest.json` 의 API 주소, 아카이브 이름), 미인증 API 한도 실측. PR #137 은 이유와 `@dependabot ignore this major version` 으로 닫음 | 충족 |

독립 반박 검토 3회(에이전트 24·12·15, 중간 이상 발견마다 검증자 2명). 1차는 리허설 설계(대역이 하나뿐인 재시도를 써 버림)와 문서의 단정을, 2차는 명령표의 `--repo`
누락과 판정 문구를, 3차는 빌드 안의 `stapler staple` 이 재시도 없이 실패를 숨기는 문제를 짚었고 모두 반영했다.

## CI

`test-archive-b-shard-1` 의 `issue_6612_sample14_wmf_pictures_reach_pdf` 가 실패했다(기대 21, 실제 101). 이 PR 은 엔진을 바꾸지 않았고 직전 head `0f93f796ab` 에서는 같은 테스트가
통과했다. 통과한 job 은 `ubuntu-26.04` 20261025.179.1, 실패한 job 은 20261101.186.1 이미지였고, 실패한 쪽에만 `svg2pdf SubsetError` 와 `embed_text=false` 재시도 경고가 있다.
2026-11-07 Dependabot PR 7개의 Archive B 도 같은 이미지와 경고로 실패한다. 재실행해도 같은 이미지에서는 실패하므로 재실행하지 않고 병합했다. 원인 수정은 별도 작업이다.

## 조판 원칙·시각 증적

비해당 — workflow·수동 감사 도구·문서만 바꿨다.

## 병합 후

- 리허설 초안 2개(433493223·433572957)와 그 태그, v0.8.6 초안(401464978)은 되돌릴 수 없어 작업지시자가 지운다. v0.8.6 의 git 태그는 남긴다. 재조회로 확인한 뒤 #172 를 닫는다.
- 원격 브랜치 `claude/desktop-release-rehearsal`·`claude/desktop-release-retry-shim` 은 작업지시자 결정으로 지운다. 일회용 브랜치의 commit 과 run 기록은 공개 저장소에 남는다.
- 본인 PR 이며 별도 댓글 지시가 없어 PR 댓글은 게시하지 않았다.
