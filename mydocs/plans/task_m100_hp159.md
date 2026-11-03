# Task #159 (M100) — Windows 혼합 배율 모니터의 새 창 위치 수행계획서

- **이슈**: [paldyn/HanPage#159](https://github.com/paldyn/HanPage/issues/159)
- **브랜치**: `claude/desktop-window-placement` (#157 병합 뒤 `origin/devel` 기준)
- **근거**: #156 의 2차 독립 검토에서 확인하고 후속으로 남긴 결함. 작업지시자 지시(2026-10-28) "제안된 작업 2개도 처리하자"

## 1. 원인

- `cascade_position` 이 앞 창 위치·작업 영역을 각자의 배율로 논리 좌표로 바꿔 창 설정의 x/y(논리)로 넘긴다.
- tao 0.35.3 은 Windows 에서 창을 만들 때 모니터를 나열 순서대로 보며 '논리 좌표 × 그 모니터 배율' 이 그 모니터 안에 드는
  첫 모니터를 고른다(`platform_impl/windows/window.rs` 창 생성부). 배율이 섞이면 한 논리 좌표가 여러 모니터에 해당해,
  먼저 나열된 모니터에 열릴 수 있다. 생성 뒤 `set_outer_position(Physical)` 은 그 물리 좌표로 정확히 옮긴다.
- macOS 는 전역 좌표가 point 하나뿐이라 해당하지 않는다.

## 2. 설계

- 좌표계를 플랫폼별로 고른다(`Space`): macOS 는 point(논리), Windows 는 가상 화면의 물리 픽셀. 모든 창의 위치·작업 영역을
  그 좌표계로 바꿔 비켜 놓기·넘침 판단을 한다. 간격(28 논리 px)·창 크기·같은 자리 허용 오차(4 논리 px)는 기준 창 화면의
  배율을 곱한다. Tauri 값 읽기(`cascade_position`)와 계산(`plan_cascade` → `next_cascade_position`)을 나눠 계산을 단위 테스트한다.
- Windows 에서는 새 창을 처음부터 대상 모니터(계산한 자리를 포함하는 모니터)에서 만든다. tao 는 창을 만든 모니터의 배율로
  크기를 정하고, 만든 뒤 다른 배율의 모니터로 옮기면 크기가 맞지 않을 수 있다(배율 변경은 창이 주로 걸친 모니터와 Windows
  설정에 달렸다 — 독립 검토 3.4).
  - 그 자리의 논리 좌표가 tao 규칙으로 대상 모니터만 가리키면 창 설정의 x/y 로 바로 만든다(이전 동작과 같다).
  - 먼저 나열된 다른 모니터를 가리키면, 대상 모니터 안에서 논리 좌표가 그 모니터만 가리키고 창이 들어가는 자리를 골라 숨긴
    채 만들고, `set_position(PhysicalPosition)` 으로 같은 모니터 안의 계산한 자리로 옮긴 뒤 `show()`·`set_focus()`.
  - tao 규칙(`tao_creation_display`)과 판단(`windows_placement`)은 순수 함수로 두고 모니터 목록은 `available_monitors()`
    (tao 와 같은 나열 순서)에서 읽는다.
- macOS 는 지금처럼 창 설정의 x/y(point).
- `#[cfg]` 블록 대신 `cfg!(windows)` 로 고른다 — Windows 경로 코드도 macOS 빌드에서 타입 검사를 받는다.

## 3. 검증 계획

- 단위 테스트: 혼합 배율 예(주 1920x1080 100%, 오른쪽 3840x2160 150%, 앞 창 물리 (2000,100))에서 논리 방식은 tao 규칙으로
  주 모니터, 물리 방식은 보조 모니터 (2042,142) 이고 보조 모니터에서 만든 뒤 옮긴다. 200% 노트북 왼쪽의 100% 외부 모니터처럼
  논리 좌표가 하나만 가리키는 배치는 바로 만든다. 배율을 따른 허용 오차·넘침 되돌림, macOS point 계산 유지.
- 변형 검사: 배율 곱하기·point 변환·허용 오차·바로 만들기/옮기기 판단·tao 규칙의 배율을 하나씩 되돌려 새 테스트가 FAIL 하는지 본다.
- 데스크톱 크레이트 fmt·Clippy·테스트(로컬), PR 의 `Desktop check`(#158, macOS·Windows Clippy·테스트), Windows 번들
  (`desktop-release` workflow_dispatch).
- Windows 혼합 배율 실기는 작업지시자 환경에서 확인한다(이 환경은 macOS).
