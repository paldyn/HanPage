/**
 * [Task #1 · HanPage 데스크톱] 네이티브(Tauri)↔웹 파일 핸드오프 브리지.
 *
 * **web-inert 원칙**: 이 모듈은 브라우저(GitHub Pages 웹 빌드)에서 완전한 no-op 이다.
 * - `initDesktopBridge()` 는 데스크톱 런타임이 아니면 즉시 return → 아무 핸들러도 등록 안 함.
 * - `getDesktopOpenHandler()` / `getDesktopSaveHandler()` 는 브라우저에서 항상 `null` →
 *   `file.ts` 의 가드 분기가 건너뛰어 기존 웹 동작이 그대로 유지된다.
 * - `@tauri-apps/*` npm 의존성을 일절 import 하지 않는다. Tauri 웹뷰에만 주입되는 전역
 *   `window.__TAURI__`(tauri.conf.json `withGlobalTauri: true`)를 통해서만 네이티브를 호출한다.
 *   따라서 웹 번들에 Tauri 코드가 정적으로도 동적으로도 포함되지 않는다.
 */

/** Tauri `invoke` 시그니처(필요한 부분만 로컬 선언 — @tauri-apps/api 의존 회피). */
type TauriInvoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
/** Tauri 이벤트 `listen` 시그니처(필요한 부분만 로컬 선언). `target` 이 문자열이면 그 레이블의 창 이벤트만 받는다. */
type TauriListen = <T>(
  event: string,
  handler: (e: { payload: T }) => void,
  options?: { target?: string },
) => Promise<() => void>;

interface TauriGlobal {
  core?: { invoke?: TauriInvoke };
  event?: { listen?: TauriListen };
}

/** 네이티브(Rust)↔웹 이벤트 이름. `src-tauri/src/lib.rs` 의 EVT_* 와 일치해야 한다. */
const EVT_MENU = 'hanpage://menu';
const EVT_DOCS_READY = 'hanpage://documents-ready';
const EVT_UPDATE_READY = 'hanpage://update-ready';
const EVT_UPDATE_STATUS = 'hanpage://update-status';
const EVT_CLOSE_REQUESTED = 'hanpage://close-requested';

/** 설정 파일이 만드는 첫 창. 실행 중에 만드는 문서 창은 `doc-N` 이다(#156). */
const MAIN_WINDOW_LABEL = 'main';

/** [#59] 네이티브 메뉴의 "업데이트 확인" 항목 id. 스튜디오 커맨드가 아니라 브리지가 처리한다. */
export const MENU_CHECK_UPDATE = 'app:check-update';

/** Tauri 웹뷰 여부. 데스크톱 런타임에서만 `__TAURI_INTERNALS__` 가 주입된다. */
export function isDesktopRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

function tauriInvoke(): TauriInvoke | null {
  const g = (window as unknown as { __TAURI__?: TauriGlobal }).__TAURI__;
  return g?.core?.invoke ?? null;
}

function tauriListen(): TauriListen | null {
  const g = (window as unknown as { __TAURI__?: TauriGlobal }).__TAURI__;
  return g?.event?.listen ?? null;
}

/** 이 웹뷰가 들어 있는 창의 레이블. 브라우저에서는 `null`. */
export function currentDesktopWindowLabel(): string | null {
  if (!isDesktopRuntime()) return null;
  const metadata = (window as unknown as {
    __TAURI_INTERNALS__?: { metadata?: { currentWindow?: { label?: string }; currentWebview?: { windowLabel?: string } } };
  }).__TAURI_INTERNALS__?.metadata;
  return metadata?.currentWindow?.label ?? metadata?.currentWebview?.windowLabel ?? null;
}

/**
 * [#156] 실행 중에 새로 만든 문서 창인지. 자동 저장 복구 제안은 처음 뜬 창에서만 한다 — 다른 창이
 * 편집 중인 임시 저장본까지 복구 후보로 보이기 때문이다.
 */
export function isSecondaryDesktopWindow(): boolean {
  const label = currentDesktopWindowLabel();
  return label !== null && label !== MAIN_WINDOW_LABEL;
}

// ─── 창 상태 (#156 문서마다 창 하나) ────────────────────────────────────────
/**
 * 이 창이 지금 문서를 받을 수 있는지와 Rust 에 보고할 상태. 문서는 손대지 않은 빈 창일 때만 이 창에서
 * 열고, 아니면 새 창으로 넘긴다. Rust 도 같은 규칙으로 창을 고르지만, 보고가 늦은 사이 도착한 문서는
 * 여기서 한 번 더 걸러 편집 중인 문서를 덮지 않는다.
 */
type WindowPhase = 'booting' | 'startup' | 'ready';
let windowPhase: WindowPhase = 'booting';
let documentLoaded = false;
/** 지금 문서를 네이티브에서 넘겨받았거나 네이티브로 저장했으면 그 파일 경로(같은 파일 재열기 판단). */
let nativePath: string | null = null;
/** 네이티브 문서를 여는 중이면 그 경로. 문서 교체 신호를 받으면 `nativePath` 로 옮긴다. */
let openingNativePath: string | null = null;
let isDocumentDirty: () => boolean = () => false;

function windowAcceptsDocument(): boolean {
  if (windowPhase === 'booting') return !documentLoaded;
  return windowPhase === 'ready' && !documentLoaded && !isDocumentDirty();
}

/** 데스크톱이면서 이 창이 문서를 받을 수 없으면 `true` — 호출자는 새 창으로 넘긴다. */
export function shouldOpenInNewDesktopWindow(): boolean {
  return isDesktopRuntime() && tauriInvoke() !== null && !windowAcceptsDocument();
}

function reportWindowState(): void {
  if (windowPhase !== 'ready') return;
  const invoke = tauriInvoke();
  if (!invoke) return;
  void invoke('cmd_report_window_state', {
    pristine: !documentLoaded && !isDocumentDirty(),
    dirty: isDocumentDirty(),
    nativePath,
  }).catch((error) => console.warn('[desktop-bridge] 창 상태 보고 실패:', error));
}

/** 문서를 바이트로 연 직후(`document-swapped`). 네이티브에서 넘겨받은 문서면 그 경로를 기억한다. */
export function noteDesktopDocumentSwapped(): void {
  documentLoaded = true;
  nativePath = openingNativePath;
  openingNativePath = null;
  reportWindowState();
}

/** 새 빈 문서를 만든 직후. 다시 손대지 않은 창이 된다. */
export function noteDesktopBlankDocument(): void {
  documentLoaded = false;
  nativePath = null;
  reportWindowState();
}

/** 문서 변경 여부가 바뀌었을 때. */
export function noteDesktopDirtyChanged(): void {
  reportWindowState();
}

/** 시작 흐름(자동 저장 복구 제안, 빈 문서)이 끝났다. 이제 Rust 가 이 창에 문서를 보낼 수 있다. */
export function markDesktopWindowReady(): void {
  if (!isDesktopRuntime()) return;
  windowPhase = 'ready';
  reportWindowState();
}

/** 손대지 않은 창이 아니라서 받을 수 없는 문서를 새 창에서 연다(끌어다 놓기 등). */
export async function openInNewDesktopWindow(bytes: Uint8Array, fileName: string, path: string | null = null): Promise<void> {
  const invoke = tauriInvoke();
  if (!invoke) throw new Error('데스크톱 앱에서만 사용할 수 있습니다.');
  await invoke('cmd_open_in_new_window', { name: fileName, path, data: Array.from(bytes) });
}

/** 네이티브 창 제목을 문서 제목(`document.title`)에 맞춘다. 창 전환·작업 표시줄에서 문서를 구분한다. */
function syncWindowTitle(invoke: TauriInvoke): void {
  if (typeof document === 'undefined') return;
  const titleElement = document.querySelector('title');
  let last = '';
  const sync = () => {
    const title = document.title;
    if (!title || title === last) return;
    last = title;
    void invoke('cmd_set_window_title', { title }).catch(() => undefined);
  };
  if (titleElement) new MutationObserver(sync).observe(titleElement, { childList: true, characterData: true, subtree: true });
  sync();
}

// ─── 열기 ──────────────────────────────────────────────────────────────────
/** 네이티브 열기로 받은 바이트를 기존 문서 오픈 흐름(eventBus)에 흘려보내는 콜백. */
export type EmitOpenDocument = (bytes: Uint8Array, fileName: string) => void;
/** 네이티브 열기 옵션. PDF 는 웹뷰 WASM 이 가져오기를 지원할 때만 대화상자에서 고를 수 있다. */
export interface DesktopOpenOptions {
  pdfImportEnabled?: boolean;
}
/** `file:open` 에서 호출. 네이티브 dialog → 바이트 → `emit` 으로 문서 오픈. */
export type DesktopOpenHandler = (emit: EmitOpenDocument, options?: DesktopOpenOptions) => Promise<void>;

interface OpenedFile {
  name: string;
  path: string;
  data: number[];
  /** 펜딩 큐 전용: 네이티브가 파일을 읽지 못한 사유. 있으면 `data` 는 비어 있다. */
  error?: string;
}

// ─── 저장 ──────────────────────────────────────────────────────────────────
export interface DesktopSaveRequest {
  bytes: Uint8Array;
  suggestedName: string;
  saveAs: boolean;
}
export type DesktopSaveResult =
  | { status: 'saved'; fileName: string }
  | { status: 'cancelled' }
  | { status: 'failed'; message: string };
export type DesktopSaveHandler = (req: DesktopSaveRequest) => Promise<DesktopSaveResult>;

type SaveOutcome =
  | { status: 'saved'; path: string; name: string }
  | { status: 'cancelled' };

// 데스크톱에서만 채워지는 핸들러. 브라우저에서는 영구히 null(= web-inert).
let openHandler: DesktopOpenHandler | null = null;
let saveHandler: DesktopSaveHandler | null = null;

/**
 * 네이티브 → 웹 푸시(파일 연결 열기·메뉴 명령)를 기존 스튜디오 흐름에 연결하는 콜백.
 * `main.ts` 가 `eventBus` / `dispatcher` 에 묶어 1회 주입한다(데스크톱에서만 사용).
 */
export interface DesktopBridgeDeps {
  /** 펜딩 큐에서 꺼낸 문서 바이트를 기존 오픈 흐름으로 흘려보낸다(unsaved-guard 적용됨). */
  openDocument: (bytes: Uint8Array, fileName: string) => void | Promise<void>;
  /** 네이티브 메뉴 명령을 스튜디오 커맨드 디스패처로 전달한다(예: `file:open`). */
  dispatchCommand: (commandId: string) => void;
  /** 파일 연결·최근 문서로 넘어온 파일을 네이티브가 읽지 못했을 때 사용자에게 알린다. */
  notifyOpenFailure?: (fileName: string, message: string) => void;
  /** [#156] 지금 문서에 저장하지 않은 변경이 있는지(창 상태 보고·문서 수용 판단). */
  isDirty?: () => boolean;
  /** [#156] 저장하지 않은 창을 닫으려 할 때 사용자에게 묻는다. `true` 면 창을 닫는다. */
  confirmClose?: () => Promise<boolean>;
}

/**
 * 브리지 초기화. `main.ts` 의 `initialize()` 에서 1줄로 호출한다.
 * 브라우저에서는 즉시 return 하여 완전한 no-op 이다.
 *
 * `deps` 가 주어지면(데스크톱) 네이티브 푸시 2종을 추가로 연결한다:
 * 1. **펜딩 문서 드레인** — 파일 연결/최근 문서/실행 argv 로 큐잉된 바이트를
 *    꺼내 `openDocument` 으로 오픈. 초기 1회(콜드 스타트) + `EVT_DOCS_READY` 수신 시(웜).
 *    네이티브가 읽지 못한 항목은 `notifyOpenFailure` 로 알린다.
 * 2. **메뉴 명령** — `EVT_MENU` 페이로드(커맨드 id)를 `dispatchCommand` 로 디스패치.
 */
export async function initDesktopBridge(deps?: DesktopBridgeDeps): Promise<void> {
  if (!isDesktopRuntime()) return; // 웹: no-op

  const invoke = tauriInvoke();
  if (!invoke) {
    console.warn('[desktop-bridge] __TAURI__ invoke 미가용 — 네이티브 연동 비활성');
    return;
  }

  openHandler = async (emit, options) => {
    // [#156] 이 창이 손대지 않은 빈 창이 아니면 네이티브가 새 창(또는 그 파일을 연 창)으로 보내고 null 을 준다.
    const res = await invoke<OpenedFile | null>('cmd_open_document', { allowPdf: options?.pdfImportEnabled === true });
    if (!res) return; // 사용자 취소 또는 다른 창에서 엶
    const bytes = new Uint8Array(res.data);
    if (!windowAcceptsDocument()) {
      await invoke('cmd_open_in_new_window', { name: res.name, path: res.path, data: res.data });
      return;
    }
    openingNativePath = res.path || null;
    emit(bytes, res.name);
  };

  saveHandler = async (req) => {
    try {
      const res = await invoke<SaveOutcome>('cmd_save_document', {
        suggestedName: req.suggestedName,
        data: Array.from(req.bytes),
      });
      if (res.status === 'cancelled') return { status: 'cancelled' };
      nativePath = res.path;
      reportWindowState();
      return { status: 'saved', fileName: res.name };
    } catch (e) {
      return { status: 'failed', message: e instanceof Error ? e.message : String(e) };
    }
  };

  syncWindowTitle(invoke);
  if (!deps) return;
  if (deps.isDirty) isDocumentDirty = deps.isDirty;

  // 큐 읽기부터 문서 열기 완료까지 같은 체인에 둔다. 등록 중 이벤트가 큐를 먼저
  // 비웠더라도 초기 drain은 그 열기를 기다리며, 미저장 확인·문서 교체도 경합하지 않는다.
  let pendingDrain: Promise<void> = Promise.resolve();
  let initialDrain = true;
  const drainPending = (): Promise<void> => {
    pendingDrain = pendingDrain.then(async () => {
      try {
        // [#156] 대기 문서는 창마다 따로 있다. 첫 호출은 페이지가 (다시) 시작했음을 함께 알린다.
        const initial = initialDrain;
        initialDrain = false;
        const docs = await invoke<OpenedFile[]>('cmd_take_pending_documents', { initial });
        for (const d of docs) {
          if (d.error !== undefined) {
            console.error(`[desktop-bridge] 파일 열기 실패: ${d.path}: ${d.error}`);
            deps.notifyOpenFailure?.(d.name, d.error);
            continue;
          }
          // 한 번에 여러 문서가 왔거나 보고가 늦은 사이 이 창이 문서를 열었으면 새 창으로 넘긴다.
          if (!windowAcceptsDocument()) {
            await invoke('cmd_open_in_new_window', { name: d.name, path: d.path, data: d.data });
            continue;
          }
          openingNativePath = d.path || null;
          try {
            await deps.openDocument(new Uint8Array(d.data), d.name);
          } finally {
            openingNativePath = null;
          }
        }
      } catch (e) {
        console.error('[desktop-bridge] 펜딩 문서 처리 실패:', e);
      }
    });
    return pendingDrain;
  };
  const listen = tauriListen();
  const label = currentDesktopWindowLabel();
  // [#156] 창 전용 이벤트(문서 도착·메뉴·닫기)는 이 창 것만 받는다. 레이블을 모르면(이전 런타임) 전역으로 받는다.
  const windowOnly = label ? { target: label } : undefined;
  if (listen) {
    // 웜 스타트: 이미 실행 중인 창에 새 문서가 큐잉되면 알림 받아 드레인.
    // [#50] listen() 등록 전에 도착한 EVT_DOCS_READY는 유실될 수 있다.
    // 등록 뒤 초기 drain으로 큐를 다시 읽어 콜드 스타트 파일도 빠짐없이 연다.
    try {
      await listen(EVT_DOCS_READY, () => { void drainPending(); }, windowOnly);
    } catch (error) {
      console.warn('[desktop-bridge] 문서 알림 구독 실패, 초기 큐는 계속 엽니다:', error);
    }
    // 네이티브 메뉴 → 스튜디오 커맨드.
    void listen<string>(EVT_MENU, (e) => {
      deps.dispatchCommand(e.payload);
    }, windowOnly);
    // 저장하지 않은 창을 닫으려 한다 → 사용자에게 묻고 답을 돌려준다.
    let closing = false;
    void listen(EVT_CLOSE_REQUESTED, () => {
      if (closing) return;
      closing = true;
      void (async () => {
        try {
          await invoke('cmd_close_ack');
          const confirmed = deps.confirmClose ? await deps.confirmClose() : true;
          await invoke('cmd_close_window', { confirmed });
        } catch (error) {
          console.error('[desktop-bridge] 창 닫기 확인 실패:', error);
          await invoke('cmd_close_window', { confirmed: false }).catch(() => undefined);
        } finally {
          closing = false;
        }
      })();
    }, windowOnly);
  }
  // 새 upstream의 시작 빈 문서가 파일 연결 문서를 덮지 않도록 초기 오픈 완료까지 기다린다.
  await drainPending();
  // 이제 시작 흐름(복구 제안·빈 문서)이 이어진다. 끝날 때까지 이 창에 문서를 받지 않는다.
  if (windowPhase === 'booting') windowPhase = 'startup';
}

// ─── 업데이트 (#59) ────────────────────────────────────────────────────────
/**
 * 업데이트 상태. Rust `UpdateState` 와 1:1 (serde tag = "state").
 * 새 버전은 사용자가 묻기 전에 조용히 내려받으므로, 웹뷰가 볼 상태는 대개
 * `downloading`(진행 중) 또는 `ready`(적용 대기) 다.
 */
export type DesktopUpdateStatus =
  | { state: 'idle' }
  | { state: 'checking' }
  | { state: 'downloading'; downloaded: number; total: number | null }
  | { state: 'verifying' }
  | { state: 'applying'; version: string }
  | { state: 'ready'; version: string }
  | { state: 'upToDate'; version: string }
  | { state: 'error'; message: string; retryable?: boolean };

export interface DesktopUpdateReady {
  version: string;
  currentVersion: string;
  notes: string | null;
}

/** 현재 업데이트 상태 조회. 브라우저에선 null. */
export async function getUpdateStatus(): Promise<DesktopUpdateStatus | null> {
  const invoke = tauriInvoke();
  if (!invoke) return null;
  try {
    return await invoke<DesktopUpdateStatus>('cmd_update_status');
  } catch {
    return null;
  }
}

/** 수동 확인 시작(진행 중이면 네이티브가 무시). 브라우저에선 no-op. */
export async function checkUpdate(): Promise<{ ok: boolean; message?: string }> {
  const invoke = tauriInvoke();
  if (!invoke) return { ok: false, message: '데스크톱 앱에서만 사용할 수 있습니다.' };
  try {
    await invoke('cmd_update_check');
    return { ok: true };
  } catch (e) {
    return { ok: false, message: e instanceof Error ? e.message : String(e) };
  }
}

/**
 * 받아둔 업데이트를 즉시 설치한다(이미 내려받혀 있어 대기 시간이 없다).
 * macOS 는 설치 후 앱이 재시작되고, Windows 는 설치 프로그램이 실행되며 앱이 종료된다.
 * 따라서 성공 시 이 Promise 는 대개 resolve 되지 않는다.
 */
export async function applyUpdate(): Promise<{ ok: boolean; message?: string }> {
  const invoke = tauriInvoke();
  if (!invoke) return { ok: false, message: '데스크톱 앱에서만 사용할 수 있습니다.' };
  try {
    await invoke('cmd_update_apply');
    return { ok: true };
  } catch (e) {
    return { ok: false, message: e instanceof Error ? e.message : String(e) };
  }
}

/**
 * [#156] 업데이트 준비 알림을 이 실행에서 처음 띄우는 창인지 네이티브에 확인한다. 창이 여러 개여도 한 번만
 * 띄운다. 확인할 수 없으면(이전 런타임, 시험 대역) 띄운다.
 */
export async function claimUpdateNudge(version: string): Promise<boolean> {
  const invoke = tauriInvoke();
  if (!invoke) return true;
  try {
    return (await invoke<boolean | undefined>('cmd_claim_update_nudge', { version })) !== false;
  } catch {
    return true;
  }
}

/** 업데이트 준비 완료 알림 구독. 브라우저에선 no-op. */
export async function onUpdateReady(cb: (info: DesktopUpdateReady) => void): Promise<void> {
  const listen = tauriListen();
  if (!listen) return;
  await listen<DesktopUpdateReady>(EVT_UPDATE_READY, (e) => cb(e.payload));
}

/** 실시간 진행 상태 구독. 등록 완료 후 현재 상태를 조회하면 초기 이벤트 유실을 막는다. */
export async function onUpdateStatus(cb: (status: DesktopUpdateStatus) => void): Promise<void> {
  const listen = tauriListen();
  if (!listen) return;
  await listen<DesktopUpdateStatus>(EVT_UPDATE_STATUS, (e) => cb(e.payload));
}

export function getDesktopOpenHandler(): DesktopOpenHandler | null {
  return openHandler;
}

export function getDesktopSaveHandler(): DesktopSaveHandler | null {
  return saveHandler;
}
