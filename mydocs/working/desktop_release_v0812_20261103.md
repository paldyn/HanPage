# HanPage Desktop 0.8.12 배포 검증

- 사용자 지시: 2026-11-03 "0.8.12 릴리스 준비해" — 버전 준비, 태그 전 시험 빌드, 초안 검증까지 진행하고 **공개는 검증
  결과를 보고한 뒤 별도 승인**을 받는다. 릴리스 PR 병합과 태그 생성도 승인을 받아 진행한다.
- 포함 변경(0.8.11 이후 devel 병합):
  - [PR #155](https://github.com/paldyn/HanPage/pull/155) — upstream rhwp 0.8.7 통합과 HanPage 기능 보정(#154)
  - [PR #157](https://github.com/paldyn/HanPage/pull/157) — 문서마다 창 하나(#156)
  - [PR #168](https://github.com/paldyn/HanPage/pull/168) — Windows 혼합 배율 모니터의 새 창 위치(#159)
  - [PR #160](https://github.com/paldyn/HanPage/pull/160) — 데스크톱 크레이트 PR 검사(#158, CI 만 바뀜)
- 엔진·Studio·root 버전(upstream 0.8.7)은 그대로이고 Desktop 5파일의 6개 버전 필드만 0.8.12 로 맞춘다.

## 버전 필드

`HanPage-Desktop/package.json`, `package-lock.json`(2곳), `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`(rhwp-desktop),
`src-tauri/tauri.conf.json`. `cargo check --locked` 통과로 잠금 파일 일치를 확인했다(남은 0.8.11 은 의존 크레이트
`regex-syntax`). 변경 이력 `0.8.12 — 2026-11-03`.

## 공개 게이트

0.8.11 과 같다. 정상 병합 SHA 에 annotated 태그 `hanpage-desktop-v0.8.12` 를 만들고 desktop-release 초안을 다음으로 확인한다.

1. macOS·Windows job 성공, `Verify macOS signature and notarization` 성공(두 `.app` Notarized Developer ID·stapler),
   Windows 요약 `Authenticode NotSigned (미서명 배포, #122)`.
2. 자산 6개와 크기·digest, `latest.json` 플랫폼 4개·같은 태그 URL·버전 0.8.12.
3. updater 서명(minisign)이 실제 자산으로 검증되고 1바이트 변조를 거부한다.
4. 번들 메타데이터 버전이 0.8.12 이다(updater `.app` 의 `CFBundleShortVersionString`, setup.exe 버전 문자열, `latest.json`).
5. 릴리스 노트에 Windows 설치 안내("Windows의 PC 보호" → 추가 정보 → 실행)가 있고, 실기에서 확인하지 않은 플랫폼 동작을
   확인된 것처럼 쓰지 않는다.

태그 전에 `claude/desktop-release-0.8.12` 로 desktop-release dispatch 를 실행해 양 플랫폼 빌드를 확인한다. 태그는 옮길 수 없으므로
빌드 실패를 태그 전에 걸러낸다. 공개 전에는 0.8.11 이 latest 로 남는다. 실패하면 태그를 옮기거나 secret 을 바꾸지 않고 같은
source 의 실패 job 만 복구한다.

## 공개 전 실기 확인(초안 설치본)

이번 변경 중 실기에서 확인하지 못한 동작이다(#156·#154·#159 보고서). 초안 설치본으로 작업지시자가 확인한 뒤 공개를 결정한다.

| 항목 | 플랫폼 |
| --- | --- |
| 다른 한글 파일은 새 창, 같은 파일은 그 창으로, 빈 창 재사용(Finder·탐색기 더블클릭·파일 > 열기·macOS 최근 문서 메뉴) | Windows·macOS(macOS 는 파일 연결 경로만 실기 확인) |
| 끌어다 놓은 파일은 빈 창이면 그 창, 아니면 새 창(같은 파일 판단 없음) | Windows·macOS |
| 창 제목에 문서 이름 | Windows·macOS |
| 저장하지 않은 창 닫기 확인, Cmd+Q 차례 확인 | macOS·Windows(창 닫기) |
| 배율이 다른 모니터를 함께 쓸 때 새 창 위치 | Windows(해당 환경이 있을 때) |
| 파일 > 열기의 PDF·HML, HWPX 저장 확장자, macOS 편집 메뉴와 AI 입력창 | macOS·Windows |
| AI 도우미: 도구 메뉴, AI 설정의 키 등록, 문서 내용 전송 동의, 질문·답변, 자동 적용 끄고 켜기 | macOS·Windows |
| 0.8.11 에서 0.8.12 로 자동 업데이트 | macOS·Windows |

## 알려진 한계

- 0.8.11 → 0.8.12 업데이트는 0.8.11 의 단일 창 업데이트 흐름으로 진행된다. 여러 창의 업데이트 사전 확인·알림 1회는 0.8.12 를
  설치한 다음 업데이트부터 적용된다.
- 다중 창: 끌어다 놓은 파일은 경로가 없어 같은 파일 판단을 하지 않는다(이미 열린 파일도 새 창). 업데이트를 적용할 때는
  그 창의 AI 도우미만 먼저 멈추고, 다른 창은 적용 중 카드와 입력 보호만 띄운다. macOS Dock 종료·로그아웃과 Windows 로그오프는 창에 묻지 않는다. 두 번째 이후 창이 비정상 종료되면 그 창에서는 자동 저장
  복구를 제안하지 않는다. 설정은 마지막으로 바꾼 창의 값이 남는다.
- macOS 에서 다른 앱 뒤에 가려진 채 시작한 창은 WebKit 이 웹 프로세스를 일시 정지해, 앞으로 가져올 때까지 문서를 열지 않을 수 있다
  (터미널 실행 실기 관측, #156 보고서 3.3). 사용자가 직접 연 경우에는 앱이 앞으로 나온다.
- AI 도우미는 rhwp 0.8.7 통합으로 0.8.12 에 처음 들어오는 기능이다. 데스크톱 실기 확인은 위 표로 한다.
- 앱 정보 화면의 `Version` 은 Studio `package.json` 을 표시한다. 기존과 같은 동작이다.

## 결과

| 단계 | 결과 |
| --- | --- |
| 공개 문구 독립 대조 | 변경 이력·릴리스 노트·게이트 기록을 출시 코드·보고서와 대조(에이전트 28, 발견마다 검증자 2명). 확인 3건 반영: 끌어다 놓기에는 같은 파일 규칙이 적용되지 않음(문구 범위 정정·README 한계 추가), AI 도우미가 새 기능인데 소개 없이 결함 수정처럼 씀(새 기능 항목 추가 — 작업지시자 결정 '포함하고 소개'), 실기 확인 표 누락(macOS 열기 경로·창 제목·AI 도우미 추가). 절반 인정된 'AI 정지 범위' 도 문구를 그 창으로 좁힘 |
| 태그 전 시험 | desktop-release dispatch [run 41558815960](https://github.com/paldyn/HanPage/actions/runs/41558815960) (`73ff09f63`): 양 플랫폼 번들 성공, `Verify macOS signature and notarization` 성공, Windows 서명 상태 보고 실행. 이후 커밋은 문서(변경 이력·README·이 기록)만 바꾼다 |
| 릴리스 PR·태그·초안 | (진행 후 기록) |
