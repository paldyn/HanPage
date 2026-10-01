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
/** Tauri 이벤트 `listen` 시그니처(필요한 부분만 로컬 선언). */
type TauriListen = <T>(
  event: string,
  handler: (e: { payload: T }) => void,
) => Promise<() => void>;

interface TauriGlobal {
  core?: { invoke?: TauriInvoke };
  event?: { listen?: TauriListen };
}

/** 네이티브(Rust)↔웹 이벤트 이름. `src-tauri/src/lib.rs` 의 EVT_* 와 일치해야 한다. */
const EVT_MENU = 'hanpage://menu';
const EVT_DOCS_READY = 'hanpage://documents-ready';
const EVT_UPDATE_READY = 'hanpage://update-ready';

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

// ─── 열기 ──────────────────────────────────────────────────────────────────
/** 네이티브 열기로 받은 바이트를 기존 문서 오픈 흐름(eventBus)에 흘려보내는 콜백. */
export type EmitOpenDocument = (bytes: Uint8Array, fileName: string) => void;
/** `file:open` 에서 호출. 네이티브 dialog → 바이트 → `emit` 으로 문서 오픈. */
export type DesktopOpenHandler = (emit: EmitOpenDocument) => Promise<void>;

interface OpenedFile {
  name: string;
  path: string;
  data: number[];
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
}

/**
 * 브리지 초기화. `main.ts` 의 `initialize()` 에서 1줄로 호출한다.
 * 브라우저에서는 즉시 return 하여 완전한 no-op 이다.
 *
 * `deps` 가 주어지면(데스크톱) 네이티브 푸시 2종을 추가로 연결한다:
 * 1. **펜딩 문서 드레인** — 파일 연결/최근 문서/단일 인스턴스 argv 로 큐잉된 바이트를
 *    꺼내 `openDocument` 으로 오픈. 초기 1회(콜드 스타트) + `EVT_DOCS_READY` 수신 시(웜).
 * 2. **메뉴 명령** — `EVT_MENU` 페이로드(커맨드 id)를 `dispatchCommand` 로 디스패치.
 */
export async function initDesktopBridge(deps?: DesktopBridgeDeps): Promise<void> {
  if (!isDesktopRuntime()) return; // 웹: no-op

  const invoke = tauriInvoke();
  if (!invoke) {
    console.warn('[desktop-bridge] __TAURI__ invoke 미가용 — 네이티브 연동 비활성');
    return;
  }

  openHandler = async (emit) => {
    const res = await invoke<OpenedFile | null>('cmd_open_document');
    if (!res) return; // 사용자 취소
    emit(new Uint8Array(res.data), res.name);
  };

  saveHandler = async (req) => {
    try {
      const res = await invoke<SaveOutcome>('cmd_save_document', {
        suggestedName: req.suggestedName,
        data: Array.from(req.bytes),
      });
      if (res.status === 'cancelled') return { status: 'cancelled' };
      return { status: 'saved', fileName: res.name };
    } catch (e) {
      return { status: 'failed', message: e instanceof Error ? e.message : String(e) };
    }
  };

  if (!deps) return;

  // 큐 읽기부터 문서 열기 완료까지 같은 체인에 둔다. 등록 중 이벤트가 큐를 먼저
  // 비웠더라도 초기 drain은 그 열기를 기다리며, 미저장 확인·문서 교체도 경합하지 않는다.
  let pendingDrain: Promise<void> = Promise.resolve();
  const drainPending = (): Promise<void> => {
    pendingDrain = pendingDrain.then(async () => {
      try {
        const docs = await invoke<OpenedFile[]>('cmd_take_pending_documents');
        for (const d of docs) await deps.openDocument(new Uint8Array(d.data), d.name);
      } catch (e) {
        console.error('[desktop-bridge] 펜딩 문서 처리 실패:', e);
      }
    });
    return pendingDrain;
  };
  const listen = tauriListen();
  if (listen) {
    // 웜 스타트: 이미 실행 중인 창에 새 문서가 큐잉되면 알림 받아 드레인.
    // [#50] listen() 등록 전에 도착한 EVT_DOCS_READY는 유실될 수 있다.
    // 등록 뒤 초기 drain으로 큐를 다시 읽어 콜드 스타트 파일도 빠짐없이 연다.
    try {
      await listen(EVT_DOCS_READY, () => { void drainPending(); });
    } catch (error) {
      console.warn('[desktop-bridge] 문서 알림 구독 실패, 초기 큐는 계속 엽니다:', error);
    }
    // 네이티브 메뉴 → 스튜디오 커맨드.
    void listen<string>(EVT_MENU, (e) => {
      deps.dispatchCommand(e.payload);
    });
  }
  // 새 upstream의 시작 빈 문서가 파일 연결 문서를 덮지 않도록 초기 오픈 완료까지 기다린다.
  await drainPending();
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
  | { state: 'ready'; version: string }
  | { state: 'upToDate'; version: string }
  | { state: 'error'; message: string };

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
export async function checkUpdate(): Promise<void> {
  const invoke = tauriInvoke();
  if (!invoke) return;
  try {
    await invoke('cmd_update_check');
  } catch (e) {
    console.warn('[desktop-bridge] 업데이트 확인 실패:', e);
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

/** 업데이트 준비 완료 알림 구독. 브라우저에선 no-op. */
export function onUpdateReady(cb: (info: DesktopUpdateReady) => void): void {
  const listen = tauriListen();
  if (!listen) return;
  void listen<DesktopUpdateReady>(EVT_UPDATE_READY, (e) => cb(e.payload));
}

export function getDesktopOpenHandler(): DesktopOpenHandler | null {
  return openHandler;
}

export function getDesktopSaveHandler(): DesktopSaveHandler | null {
  return saveHandler;
}
