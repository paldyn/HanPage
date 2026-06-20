# Task M100 #1443 Stage 18 작업 기록

- 이슈: #1443
- 브랜치: `local/task_m100_1443`
- 시작일: 2026-06-20
- 선행 커밋: `31005d6b task 1443: 표 이동과 속성 동기화 보정`

## 1. 목표

메뉴와 우클릭 컨텍스트 메뉴에 `모양 붙여넣기`를 명시적으로 추가한다.

## 2. 배경

현재 `edit:format-copy`는 내부 상태에 따라 다음 두 역할을 한다.

- 복사된 모양이 없거나 대상 선택이 없으면 현재 커서 위치의 모양 복사
- 복사된 모양이 있고 대상 선택이 있으면 선택 대상에 모양 적용

하지만 UI에는 `모양 복사`만 보여 사용자가 붙여넣기 동작을 찾기 어렵다.

## 3. 수정 계획

- `edit:format-paste` 커맨드 추가
- `InputHandler.performFormatPaste()` 추가
- `EditorContext`에 모양 복사 상태 여부 추가
- 편집 메뉴에 `모양 붙여넣기` 추가
- 기본/표/개체 컨텍스트 메뉴에 `모양 복사`, `모양 붙여넣기` 추가

## 4. 검증 계획

- `cd rhwp-studio && npx tsc --noEmit`
- `git diff --check`
- 수동 확인
  - 편집 메뉴에 `모양 붙여넣기` 표시
  - 우클릭 메뉴에 `모양 복사`, `모양 붙여넣기` 표시
  - 모양 복사 전 붙여넣기 비활성
  - 모양 복사 후 대상 선택 시 붙여넣기 활성

## 5. 수정 결과

- `EditorContext.hasCopiedFormat`을 추가했다.
- `InputHandler.hasCopiedFormat()`을 추가했다.
- `InputHandler.performFormatPaste()`를 추가했다.
- 기존 `performFormatCopy()`의 적용 로직을 `applyCopiedFormatToCurrentTarget()`으로 분리했다.
  - 기존 `Alt+C` 토글 동작은 유지한다.
  - 새 `모양 붙여넣기` 메뉴는 붙여넣기만 수행한다.
- `edit:format-paste` 커맨드를 추가했다.
- 편집 메뉴에 `모양 붙여넣기` 항목을 추가했다.
- 기본 컨텍스트 메뉴와 표 셀 컨텍스트 메뉴에 `모양 복사`, `모양 붙여넣기` 항목을 추가했다.

## 6. 검증 결과

- `cd rhwp-studio && npx tsc --noEmit`
  - 통과
- `git diff --check`
  - 통과
