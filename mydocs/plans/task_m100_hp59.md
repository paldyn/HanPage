# Task #59 (M100) — 데스크톱 자동 업데이트 UX 개선 수행계획서

- **이슈**: [paldyn/HanPage#59](https://github.com/paldyn/HanPage/issues/59)
- **브랜치**: `local/task59` (devel 분기)
- **컨셉**: Claude 데스크톱 앱 방식 — **막지 않고, 미리 받아두고, 한 번만 물어본다**

## 1. 문제와 원인

작업지시자 보고: "팝업이 너무 투박하고, 업데이트할 때 로딩바나 아무것도 안 뜨고 몇 초 동안 가만히 있어서 버그 걸린 줄 알았다."

`HanPage-Desktop/src-tauri/src/lib.rs:342`

```rust
match update.download_and_install(|_, _| {}, || {}).await {
```

- 사용자가 '지금 설치'를 누른 **뒤에** 40MB 다운로드가 시작됨 → 그 시간 동안 화면 정지
- 진행률 콜백 2개가 모두 no-op → 신호 0
- 알림이 OS 네이티브 dialog → 앱과 이질적

## 2. 컨셉 전환 (핵심)

기존은 **묻고 → 받는다**. 그래서 사용자가 기다린다.
Claude 앱 방식은 **받아두고 → 묻는다**. 그래서 기다림이 구조적으로 사라진다.

| | 지금 | 개선 후 |
|---|------|--------|
| 새 버전 발견 | 네이티브 모달로 작업 중단 | **조용히 백그라운드 다운로드** (아무 것도 안 띄움) |
| 다운로드 | 사용자가 승인한 뒤 시작, 무표시 대기 | 이미 끝나 있음 |
| 알림 | OS 모달 | **비침습 토스트**: `새 버전 0.8.3 준비됨` + [지금 다시 시작] [나중에] |
| 클릭 후 | 40MB 받는 동안 정지 | **즉시 설치 + 재시작** (받아둔 바이트 사용) |

기술적 근거(확인 완료): `download()` 는 `Result<Vec<u8>>` 를 반환하고 `install(bytes)` 는 별도 동기 함수다
(`tauri-plugin-updater-2.10.1/src/updater.rs:652,718`). 따라서 다운로드와 설치를 분리해 미리 받아둘 수 있다.

## 3. 구현 범위

### 3.1 Rust (`HanPage-Desktop/src-tauri/src/lib.rs`)

- 전역 보관소 `READY_UPDATE: LazyLock<Mutex<Option<(Update, Vec<u8>)>>>` — 받아둔 업데이트(≈40MB, 재시작 전까지 메모리 유지)
- **시작 시 흐름**(자동): `check()` → 새 버전이면 **조용히** `download()` → 완료 시 `hanpage://update-ready` emit `{ version, currentVersion, notes }`
  - 실패·최신은 무표시(현행 유지) — 로그만
- 커맨드
  - `cmd_update_apply()` → 보관 중인 바이트로 `install()` → macOS 는 `app.restart()`
  - `cmd_update_status()` → 수동 확인용 `{ state: 'checking'|'downloading'|'ready'|'uptodate'|'error', version, downloaded, total }`
- 진행률 이벤트 `hanpage://update-progress`(120ms throttle) — 평소엔 UI 미표시, **수동 확인 시에만** 상태 안내에 사용
- 기존 네이티브 dialog 경로 제거

### 3.2 프런트 (`rhwp-studio`)

- `src/core/desktop-bridge.ts` — `onUpdateReady(cb)` / `applyUpdate()` / `getUpdateStatus()` 추가 (web-inert 유지)
- `src/main.ts` — 브리지에 업데이트 준비 콜백 연결
- **UI 는 기존 토스트 재사용**(신규 모달 없음):

```ts
showToast({
  message: '새 버전 0.8.3 이(가) 준비되었습니다.\n다시 시작하면 바로 적용됩니다.',
  durationMs: 0,                       // 사용자가 닫을 때까지 유지
  action: { label: '지금 다시 시작', onClick: applyUpdate },
  confirmLabel: '나중에',
});
```

- 메뉴 `업데이트 확인`(수동): 상태에 따라 토스트로 즉답
  - 확인 중/내려받는 중 → `새 버전을 내려받고 있습니다… 준비되면 알려드릴게요.`
  - 준비됨 → 위 토스트 재표시
  - 최신 → `현재 최신 버전입니다. (0.8.2)`
  - 오류 → `업데이트 확인에 실패했습니다.`

### 3.3 정책

- '나중에'를 눌러도 **받아둔 바이트는 유지** → 다음에 눌러도 즉시 적용
- 다운로드는 조용히. 사용자가 굳이 궁금해서 메뉴로 확인할 때만 진행 상황을 알려줌
- **Windows**: `install()` 이 설치 프로그램 실행 후 프로세스를 종료(반환하지 않음) → 토스트 문구를 `설치 프로그램이 실행됩니다`로 분기, 이후 UI 없음을 전제
- 시작 직후 즉시 다운로드하지 않고 웹뷰 준비 후 여유를 두어 초기 로딩과 대역폭 경합 방지

## 4. 검증

1. `tsc --noEmit` + studio 테스트
2. 데스크톱 빌드
3. **실측**: 로컬 앱에 진단 로그를 붙여 `check → download(진행률 누적) → update-ready emit → 토스트 표시 → apply` 전 구간이 실제로 흐르는지 확인
4. 웹 무영향(web-inert) 확인

## 5. 리스크

- 받아둔 40MB 메모리 상주 — 재시작 전까지 유지(허용). 필요 시 후속으로 임시 파일 전환 검토
- Windows 는 설치 이후 UI 불가 → 재시작 안내를 install 이전에 마무리
- 진행률 이벤트 과다 → Rust 측 throttle
