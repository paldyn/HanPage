# 트러블슈팅 — 최근 문서 메뉴가 계속 비활성화 (#2285)

- **이슈**: edwardkim/rhwp#2285 / PR #2286
- **작성일**: 2026-07-15

## 증상

파일 메뉴의 "최근 문서" 서브메뉴가 문서를 열어도 **계속 비활성화(disabled)** 상태로 남아 열리지 않음.

## 원인

최근 문서 기록 훅(`addRecentDoc`)이 `loadBytes()`에서 **`await initializeDocument()` 뒤**에 위치했다.
`initializeDocument()`는 내부 마지막에 `promptLocalFontsIfNeeded()`(로컬 글꼴 안내 **모달**)를 `await`한다.

- 문서가 미설치 글꼴을 사용해 모달이 뜨는 경우(또는 자동화/헤드리스처럼 모달이 즉시 닫히지 않는 경우), `initializeDocument()` 프라미스가 그 지점에서 대기 → **그 뒤의 기록 훅이 실행되지 않음**.
- 결과적으로 IndexedDB에 최근 문서가 저장되지 않아 목록이 비고, `renderRecentSubmenu()`가 서브메뉴를 계속 disabled로 유지.

## 해결

기록 훅을 **문서 로드 성공 직후·블로킹 UI(폰트/모달) 이전**으로 이동.
`wasm.loadDocument()` 성공 + `wasm.currentFileHandle = fileHandle` 설정 직후에 `addRecentDoc`를 호출한다(파일명/형식은 이 시점에 이미 유효).

```ts
wasm.currentFileHandle = fileHandle;
if (fileHandle) {
  void addRecentDoc({ fileName: wasm.fileName, sourceFormat: wasm.getSourceFormat(), handle: fileHandle })
    .catch((err) => console.warn('[recent] 최근 문서 기록 실패:', err));
}
await autosaveManager.beginDocument(...);
await initializeDocument(...);
```

## 재발 방지

- "부수 효과 기록"은 **가능한 한 이른 시점**(핵심 상태 확정 직후)에 배치하고, 사용자 상호작용을 동반하는 `await`(모달/프롬프트) **뒤에 두지 않는다**.
- puppeteer headless 진단으로 `open-document-bytes` → `loadBytes` → `addRecentDoc` → 렌더 체인을 재현하여 확인.
