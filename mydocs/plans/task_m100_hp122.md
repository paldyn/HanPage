# Task #122 (M100) — Windows 설치 파일 Authenticode 정식 서명 수행계획서

- **이슈**: [paldyn/HanPage#122](https://github.com/paldyn/HanPage/issues/122)
- **브랜치**: `claude/desktop-windows-signing` (`origin/devel` `5685104a56a6502b57a8f210f5ea14e8d45b6e88` 분기)
- **작업지시자 지시(2026-10-07)**: "부족한 부분 손보고 윈도우도 설치파일 정식으로 할수있게 진행하자", 이후 "둘다 응응"(이슈·PR 승인)
- **이전 결정**: 2026-07-10 "Windows 코드 서명 — 진행 안 함"([orders 20260710](../orders/archives/20260710_hanpage.md))을 위 지시로 번복

## 1. 현재 상태와 부족한 부분

| 항목 | 상태 |
|---|---|
| macOS | Developer ID 서명·공증(#4). CI 에 서명·공증 결과 검증 단계 없음(수동 감사만) |
| Windows | Authenticode 없음. 설치 시 "알 수 없는 게시자"·SmartScreen 경고 |
| workflow 주석 | "코드 서명·공증은 범위 밖", "updater 키 없으면 아티팩트만 생략" — 둘 다 사실과 다름 |
| 운영 가이드 | 릴리스 절차(버전 파일 2개·v0.7.14 예시)와 `latest.json` 예시가 낡음, 코드 서명 절 없음, 문서 지도 미등록 |
| `tauri-action` | `@v0` 부동 태그(github_operations §9.2: 변경 시 SHA 고정) |

## 2. 서명 방식 결정

조사·반박 검증(2026-10-07) 결과 **Azure Artifact Signing** 을 택한다.

- 2026-07-23 Microsoft 문서 갱신으로 한국 **조직**이 Public Trust 대상에 포함(개인은 미국·캐나다만).
- Basic 월 $9.99·월 5,000회 서명, 키는 Microsoft HSM, GitHub Actions 비대화식 서명 가능.
- CA OV 인증서는 2023-06 이후 HSM 보관 의무로 클라우드 HSM 구독이 필요해 연 $300~900 수준.
- 2024년부터 EV 도 SmartScreen 즉시 평판을 주지 않으므로 EV 의 이점이 없다.

## 3. 구현 원칙

- 서명은 `tauri build` 안에서 `bundle.windows.signCommand` 로 한다. tauri-cli 는 NSIS 번들(앱 exe·플러그인 DLL·
  uninstaller·setup.exe 서명) 뒤에 updater `.sig` 를 만든다. 빌드 뒤 서명은 `latest.json` 서명을 깨므로 금지.
- 설정은 CI 가 `--config` 조각으로만 주입한다. `tauri.windows.conf.json` 은 로컬 빌드에도 병합되므로 쓰지 않는다.
  빈 문자열 `signCommand` 도 서명을 켜므로 설정이 없을 때는 키 자체를 넣지 않는다.
- 비밀값은 명령줄에 넣지 않는다(tauri 가 명령을 로그·NSIS 스크립트에 남김). dlib 은 EnvironmentCredential 로 읽는다.
- 설정 전 릴리스는 지금처럼 미서명으로 진행할 수 있어야 하고, 서명 개시 뒤에는 미서명 통과를 막는다.

## 4. 검증 계획

- workflow YAML 파싱, CLI 2.11.5 macOS 번들 빌드와 샘플 `signCommand` 조각 스키마 수용 확인.
- updater 키 없는 빌드의 실패 메시지 재현(주석 정정 근거).
- Tauri NSIS 템플릿으로 제거 항목·조용한 설치·파일 연결 명령 확인.
- 문서 링크 검사, 메타데이터 검사 기준선 비교.
- Windows 서명·검증 단계 실제 실행은 Artifact Signing 계정 준비 후로 남긴다.

## 5. 결정 변경 (2026-10-08)

- 작업지시자 확인: PALDYN 은 사업자등록이 없는 1인 개발("팔딘 그냥 내혼자 하는건데", "사업자 등록안되어있어").
- 재조사 결과 Azure Artifact Signing 은 개인(한국) 불가, 게시자에 PALDYN 을 넣는 방법은 사업자등록 없이는 없음.
  실명 서명 최저 현실안(SSL.com IV + eSigner)은 연 약 $309 로 작업지시자가 과하다고 판단("연 약 42만원 너무한데").
- 작업지시자 선택 "1로하자": **Windows 는 미서명 배포를 유지**하고 설치 안내를 추가한다.
- 범위 조정: Azure 서명 준비 단계·Windows Authenticode 검증 단계·관련 설정과 문서를 제거한다(구현은 커밋
  [`39710e814`](https://github.com/paldyn/HanPage/commit/39710e814bbb938df679517d6b6ae3ede51e32c3)에 보존). macOS 검증 단계, CLI 2.11.5, `tauri-action` SHA 고정,
  단일 릴리스 조건, 낡은 주석·문서 정정은 유지한다. Windows 는 서명 상태를 CI 요약에 보고한다.
