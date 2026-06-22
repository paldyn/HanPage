# Task M100 #1481 Stage 3

- 이슈: #1481 표 줄/칸 편집 회귀 및 한컴식 줄/칸 메뉴 정합
- 브랜치: `task_m100_1481`
- 작성일: 2026-06-22
- 상태: 구현 및 로컬 검증 완료. IAB 새로고침 후 수동 확인 대기.

## 목표

Stage 2 커밋 이후 남은 macOS `Option+Insert` 입력 인식 문제를 실제 입력 이벤트 기준으로 진단하고, macOS 단축키를 `Option+Enter`로 변경한다.

## 배경

- `Option+C`, `Option+Delete`는 인식된다.
- `Option+Insert`는 메뉴 표시는 되지만 실제 입력이 대표 `table:insert-row-col` 명령으로 연결되지 않는다.
- 작업지시자 확인 결과, `Option+Enter`를 처음부터 입력하면 줄/칸 추가 대화상자가 열린다.
- `Option+Enter` 동작은 `Option+Insert` 해결로 보지 않는다. 실제 요구는 한컴 단축키인 `Option+Insert` 자체가 동작하는 것이다.
- macOS/브라우저/키보드 조합에 따라 Insert 계열 키가 `Insert`가 아닌 다른 `key`/`code` 조합으로 들어오거나, 브라우저까지 전달되지 않을 수 있으므로 실제 이벤트를 먼저 확인해야 한다.

## 범위

- 실제 입력 이벤트 매핑 경로를 보정한다.
- macOS에서는 줄/칸 추가 단축키를 `Option+Enter`로 변경한다.
- Windows/Linux에서는 기존 `Alt+Insert`를 유지한다.
- 기존 `Option+Delete` 및 다른 Alt/Option 단축키 동작은 유지한다.
- 표 줄/칸 메뉴 표시 규칙은 Stage 2 상태를 유지한다.

## 검증 계획

```bash
cd rhwp-studio && node --test tests/shortcut-map.test.ts tests/navigation-keymap.test.ts tests/menu-shortcut-labels.test.ts
cd rhwp-studio && npx tsc --noEmit
cd rhwp-studio && npm test
git diff --check
```

## 진단 계획

1. IAB에서 임시 keydown 로거를 붙여 `Option+Insert` 물리 입력의 `key`, `code`, modifier, keyCode를 확인한다.
2. 실제 이벤트가 앱에 도달하면 `shortcut-map.ts`에 해당 변형만 추가하고 회귀 테스트로 고정한다.
3. 실제 이벤트가 앱에 도달하지 않으면 브라우저/OS 입력 한계로 분리하고 한컴식 메뉴 표기와 가능한 대체 입력을 별도 UX 보정 대상으로 기록한다.

## 현재 확인된 사실

- `Option+Enter`는 줄/칸 추가 대화상자를 열 수 있다.
- 이 사실만으로 `Option+Insert`가 해결되었다고 판단하지 않는다.
- IAB에서 임시 이벤트 로거를 붙이고 작업지시자가 실제 `Option+Insert`를 입력했을 때, 페이지에 도달한 이벤트는 `AltLeft`의 `keydown`/`keyup`뿐이었다.
  - `key=Alt`, `code=AltLeft`, `altKey=true`/`false`
  - `Insert`, `Help`, `Enter`, `beforeinput`, `input` 이벤트는 도달하지 않았다.
- 같은 포커스 상태에서 합성 `Alt+Insert`를 보내면 `key=Insert`, `code=Insert`, `altKey=true` 이벤트가 도달했고 줄/칸 추가 대화상자가 열렸다.
- 따라서 현재 실패는 `shortcut-map.ts`의 Insert 매핑 누락이 아니라, macOS/IAB 물리 입력이 브라우저 페이지까지 `Insert` 이벤트를 전달하지 않는 문제로 분리된다.

## 구현 결과

- `rhwp-studio/src/command/shortcut-map.ts`
  - 단축키 정의에 플랫폼 조건을 추가했다.
  - macOS에서는 `Option+Enter`를 `table:insert-row-col`로 매핑한다.
  - Windows/Linux에서는 기존 `Alt+Insert`/`Alt+Help` 계열을 유지한다.
- `rhwp-studio/src/engine/navigation-keymap.ts`
  - macOS에서 `Alt+Insert` 표시 문자열을 실제 동작 단축키인 `⌥Enter`로 표시하도록 보정했다.
- `rhwp-studio/src/ui/dialog.ts`
  - `ModalDialog` 닫힘 후 훅 `afterClose`를 추가했다.
- `rhwp-studio/src/command/commands/table.ts`
  - 줄/칸 추가·지우기 대화상자가 `Esc`, 취소, 닫기 버튼, 확인 후 닫힐 때 편집 textarea 포커스를 복원한다.
  - `Option+Enter → Esc → Option+Enter`, `Option+Delete → Esc → Option+Delete` 반복 입력 회귀를 막는다.
- `rhwp-studio/tests/*`
  - macOS `Option+Enter`/Windows `Alt+Insert` 플랫폼 분기 테스트를 추가했다.
  - macOS 메뉴 표시 `⌥Enter` 회귀 테스트를 추가했다.
  - 표 대표 대화상자 닫힘 후 포커스 복원 연결을 정적 회귀 가드로 고정했다.

## 검증 결과

```bash
cd rhwp-studio && node --test tests/shortcut-map.test.ts tests/navigation-keymap.test.ts tests/menu-shortcut-labels.test.ts
# 30 passed

cd rhwp-studio && npx tsc --noEmit

cd rhwp-studio && npm test
# 120 passed
```

## IAB 확인 메모

- 현재 IAB 탭은 변경 전 번들을 유지하고 있어 표 메뉴 단축키가 아직 `⌥Insert`로 보인다.
- 변경분 기준 수동 확인을 위해서는 IAB 탭 새로고침이 필요하다.
