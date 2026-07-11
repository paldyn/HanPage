import { WasmBridge } from '@/core/wasm-bridge';
import type { DocumentInfo } from '@/core/types';
import { EventBus } from '@/core/event-bus';
import { assertRemoteDocumentBytes } from '@/core/document-signature';
import { CanvasView } from '@/view/canvas-view';
import { InputHandler } from '@/engine/input-handler';
import { Toolbar } from '@/ui/toolbar';
import { MenuBar } from '@/ui/menu-bar';
import { loadWebFonts } from '@/core/font-loader';
import { loadExtensionViewerSettings, type ExtensionViewerSettings } from '@/core/extension-settings';
import { CommandRegistry } from '@/command/registry';
import { CommandDispatcher } from '@/command/dispatcher';
import type { EditorContext, CommandServices, EditorEditMode } from '@/command/types';
import { confirmSaveBeforeReplacingDocument, fileCommands } from '@/command/commands/file';
import { editCommands } from '@/command/commands/edit';
import { syncClipMenu, syncTextMarkMenu, viewCommands } from '@/command/commands/view';
import { formatCommands } from '@/command/commands/format';
import { insertCommands } from '@/command/commands/insert';
import { tableCommands } from '@/command/commands/table';
import { pageCommands } from '@/command/commands/page';
import { toolCommands } from '@/command/commands/tool';
import { installPwaFileHandling, type FileHandlingWindowLike } from '@/command/pwa-file-handling';
import { ContextMenu } from '@/ui/context-menu';
import { CommandPalette } from '@/ui/command-palette';
import { showValidationModalIfNeeded } from '@/ui/validation-modal';
import { showLocalFontsModalIfNeeded } from '@/ui/local-fonts-modal';
import { showToast } from '@/ui/toast';
import { showDropConfirmDialog } from '@/ui/drop-confirm-dialog';
import { initRhwpDev } from '@/core/rhwp-dev';
import { DocumentDirtyState } from '@/core/document-dirty-state';
import { initThemeSync, setThemeMode, getThemeMode, getEffectiveTheme } from '@/core/theme';
import { analyzeDocumentFonts } from '@/core/document-font-status';
import { detectLocalFonts, getLocalFontState, loadStoredLocalFonts } from '@/core/local-fonts';
import { userSettings } from '@/core/user-settings';
import { AutosaveManager, type AutosaveScheduleSettings, type AutosaveStatus } from '@/recovery/autosave-manager';
import { clearAutosaveDrafts, deleteAutosaveDraft, listAutosaveDrafts, type AutosaveDraft } from '@/recovery/autosave-store';
import { recoveryFileName } from '@/recovery/recovery-format';
import { showAutosaveRecoveryDialog } from '@/recovery/recovery-ui';
import { CellSelectionRenderer } from '@/engine/cell-selection-renderer';
import { TableObjectRenderer } from '@/engine/table-object-renderer';
import { TableResizeRenderer } from '@/engine/table-resize-renderer';
import { Ruler } from '@/view/ruler';
import type { CanvasKitLayerRenderer } from '@/view/canvaskit-renderer';
import {
  resolveCanvasKitRenderModeRequest,
  resolveCanvasKitSurfaceRequest,
  resolveRenderBackendRequest,
  resolveRenderProfile,
  type RenderBackendFallbackReason,
} from '@/view/render-backend';

const wasm = new WasmBridge();
const eventBus = new EventBus();
const documentState = new DocumentDirtyState(eventBus);
documentState.installBeforeUnload(window);
const autosaveManager = new AutosaveManager({
  exportBytes: () => wasm.exportHwp(),
  schedule: autosaveScheduleFromUserSettings(),
  onStatus: handleAutosaveStatus,
});
autosaveManager.connect(eventBus);
initThemeSync((effective, mode) => {
  eventBus.emit('theme-changed', { mode, effective });
  eventBus.emit('command-state-changed');
});

// E2E 테스트용 전역 노출 (개발 모드 전용)
if (import.meta.env.DEV) {
  (window as any).__wasm = wasm;
  (window as any).__eventBus = eventBus;
  (window as any).__documentState = documentState;
  (window as any).__autosaveManager = autosaveManager;
  (window as any).__theme = { getThemeMode, getEffectiveTheme, setThemeMode };
  initRhwpDev(wasm);
}
let canvasView: CanvasView | null = null;
let inputHandler: InputHandler | null = null;
let toolbar: Toolbar | null = null;
let ruler: Ruler | null = null;
let canvaskitRenderer: CanvasKitLayerRenderer | null = null;
let editMode: EditorEditMode = 'normal';
let rendererRuntimeRequest: {
  backend: ReturnType<typeof resolveRenderBackendRequest>;
  canvaskitMode: ReturnType<typeof resolveCanvasKitRenderModeRequest>;
  canvaskitSurface: ReturnType<typeof resolveCanvasKitSurfaceRequest>;
  renderProfile: ReturnType<typeof resolveRenderProfile>;
} | null = null;
let renderBackendFallbackReason: RenderBackendFallbackReason | null = null;
let rendererInitializationError: string | null = null;
let rendererInitialized = false;
let extensionViewerSettings: ExtensionViewerSettings = {
  disableExternalWebFonts: false,
};


// ─── 커맨드 시스템 ─────────────────────────────
const registry = new CommandRegistry();

function getContext(): EditorContext {
  const hasDoc = wasm.pageCount > 0;
  const canEditFormField = inputHandler?.canEditCurrentFormField() ?? false;
  const isFormMode = editMode === 'form';
  return {
    hasDocument: hasDoc,
    hasSelection: inputHandler?.hasSelection() ?? false,
    hasCopiedFormat: inputHandler?.hasCopiedFormat() ?? false,
    inTable: inputHandler?.isInTable() ?? false,
    inCellSelectionMode: inputHandler?.isInCellSelectionMode() ?? false,
    hasMultiCellSelection: inputHandler?.hasMultiCellSelection() ?? false,
    hasTableTransposeClipboard: wasm.hasTableTransposeClipboard(),
    inTableObjectSelection: inputHandler?.isInTableObjectSelection() ?? false,
    inPictureObjectSelection: inputHandler?.isInPictureObjectSelection() ?? false,
    inField: inputHandler?.isInField() ?? false,
    isEditable: !isFormMode || canEditFormField,
    editMode,
    isFormMode,
    canEditFormField,
    canUndo: inputHandler?.canUndo() ?? false,
    canRedo: inputHandler?.canRedo() ?? false,
    zoom: canvasView?.getViewportManager().getZoom() ?? 1.0,
    showControlCodes: wasm.getShowControlCodes(),
    showParagraphMarks: wasm.getShowParagraphMarks(),
    isDirty: documentState.isDirty(),
    sourceFormat: hasDoc ? (wasm.getSourceFormat() as 'hwp' | 'hwpx') : undefined,
  };
}

function setEditMode(mode: EditorEditMode): void {
  editMode = mode;
  inputHandler?.setEditMode(mode);
  document.documentElement.dataset.editMode = mode;
  document.querySelectorAll('[data-cmd="view:form-mode"]').forEach(el => {
    el.classList.toggle('active', mode === 'form');
  });
  sbMessage().textContent = mode === 'form' ? '양식 모드' : '기본 편집 모드';
  eventBus.emit('edit-mode-changed', mode);
  eventBus.emit('command-state-changed');
}

const commandServices: CommandServices = {
  eventBus,
  wasm,
  documentState,
  getContext,
  getInputHandler: () => inputHandler,
  getViewportManager: () => canvasView?.getViewportManager() ?? null,
  setEditMode,
};

const dispatcher = new CommandDispatcher(registry, commandServices, eventBus);

// 모든 내장 커맨드 등록
registry.registerAll(fileCommands);
registry.registerAll(editCommands);
registry.registerAll(viewCommands);
registry.registerAll(formatCommands);
registry.registerAll(insertCommands);
registry.registerAll(tableCommands);
registry.registerAll(pageCommands);
registry.registerAll(toolCommands);

// 상태 바 요소
const sbMessage = () => document.getElementById('sb-message')!;
const sbPage = () => document.getElementById('sb-page')!;
const sbSection = () => document.getElementById('sb-section')!;
const sbZoomVal = () => document.getElementById('sb-zoom-val')!;
let autosaveStatusRestoreTimer: ReturnType<typeof setTimeout> | null = null;
let autosavePreviousMessage: string | null = null;

function autosaveScheduleFromUserSettings(): AutosaveScheduleSettings {
  const settings = userSettings.getAutosaveSettings();
  return {
    recoveryEnabled: settings.recoveryEnabled,
    recoveryIntervalMs: settings.recoveryIntervalMinutes * 60_000,
    idleEnabled: settings.idleSaveEnabled,
    idleDelayMs: settings.idleDelaySeconds * 1_000,
  };
}

function handleAutosaveStatus(status: AutosaveStatus): void {
  const message = document.getElementById('sb-message');
  if (!message) return;
  if (autosaveStatusRestoreTimer) {
    clearTimeout(autosaveStatusRestoreTimer);
    autosaveStatusRestoreTimer = null;
  }

  if (status.state === 'saving') {
    if (autosavePreviousMessage === null) {
      autosavePreviousMessage = message.textContent ?? '';
    }
    message.textContent = '복구용 자동 저장 중...';
    return;
  }

  const restoreTarget = autosavePreviousMessage;
  autosavePreviousMessage = null;
  const nextMessage = status.state === 'saved'
    ? `복구용 자동 저장 완료 (${formatBytes(status.byteLength)})`
    : '복구용 자동 저장 실패';
  message.textContent = nextMessage;
  if (restoreTarget !== null) {
    autosaveStatusRestoreTimer = setTimeout(() => {
      if (message.textContent === nextMessage) {
        message.textContent = restoreTarget;
      }
      autosaveStatusRestoreTimer = null;
    }, status.state === 'saved' ? 1_600 : 4_000);
  }
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const kib = bytes / 1024;
  if (kib < 1024) return `${kib.toFixed(1)} KiB`;
  return `${(kib / 1024).toFixed(1)} MiB`;
}

function waitForNextPaint(): Promise<void> {
  return new Promise((resolve) => {
    let done = false;
    const finish = () => {
      if (done) return;
      done = true;
      resolve();
    };
    window.setTimeout(finish, 50);
    requestAnimationFrame(() => requestAnimationFrame(finish));
  });
}

async function updateLoadProgress(percent: number, label: string): Promise<void> {
  const safePercent = Math.max(0, Math.min(100, Math.round(percent)));
  sbMessage().textContent = `파일 로딩 ${safePercent}% - ${label}`;
  await waitForNextPaint();
}

/**
 * CanvasKit은 browser CSS font fallback을 사용하지 않는다. 초기 페이지를 먼저 표시한 뒤,
 * 저장된 권한 범위 안에서 필요한 local face를 준비하고 등록된 경우에만 다시 그린다.
 */
function prepareCanvasKitLocalFonts(fontNames: readonly string[] | undefined): void {
  const renderer = canvaskitRenderer;
  if (!renderer || !fontNames?.length) return;
  const requestedFonts = [...fontNames];
  void (async () => {
    await loadStoredLocalFonts();
    const registered = await renderer.prepareLocalFonts(requestedFonts);
    if (registered > 0 && renderer === canvaskitRenderer) {
      // 글꼴 face만 바뀌므로 문서와 편집 위치는 유지한 채 현재 페이지를 다시 그린다.
      eventBus.emit('document-view-changed');
    }
  })().catch((error) => {
    console.warn('[CanvasKit] 로컬 Typeface 준비 실패, 기본 fallback으로 계속 표시합니다:', error);
  });
}

async function initialize(): Promise<void> {
  const msg = sbMessage();
  try {
    extensionViewerSettings = await loadExtensionViewerSettings();
    if (extensionViewerSettings.disableExternalWebFonts) {
      console.info('[main] 외부 웹폰트 사용 안 함 옵션이 켜져 있습니다.');
    }
    msg.textContent = extensionViewerSettings.disableExternalWebFonts
      ? '로컬 폰트 준비 중...'
      : '웹폰트 로딩 중...';
    await loadWebFonts([], undefined, extensionViewerSettings);  // CSS @font-face 등록 + CRITICAL 폰트만 로드
    msg.textContent = 'WASM 로딩 중...';
    await wasm.initialize();
    if (import.meta.env.DEV) {
      initRhwpDev(wasm);
    }
    const renderBackendRequest = resolveRenderBackendRequest(window.location.search);
    const canvaskitModeRequest = resolveCanvasKitRenderModeRequest(window.location.search);
    const canvaskitMode = canvaskitModeRequest.mode;
    const canvaskitSurfaceRequest = resolveCanvasKitSurfaceRequest(window.location.search);
    const renderProfile = resolveRenderProfile(window.location.search);
    rendererRuntimeRequest = {
      backend: renderBackendRequest,
      canvaskitMode: canvaskitModeRequest,
      canvaskitSurface: canvaskitSurfaceRequest,
      renderProfile,
    };
    if (renderBackendRequest.unsupportedReason) {
      console.warn(
        `[main] 지원하지 않는 renderer 값입니다: ${renderBackendRequest.requested}; Canvas2D를 사용합니다.`,
      );
    }
    if (canvaskitModeRequest.unsupportedReason) {
      console.warn(
        `[main] 지원하지 않는 CanvasKit mode입니다: ${canvaskitModeRequest.requested}; default를 사용합니다.`,
      );
    }
    let renderBackend = renderBackendRequest.backend;
    renderBackendFallbackReason = renderBackendRequest.unsupportedReason ?? null;
    if (renderBackend === 'canvaskit') {
      msg.textContent = 'CanvasKit 로딩 중...';
      try {
        const { CanvasKitLayerRenderer } = await import('@/view/canvaskit-renderer');
        canvaskitRenderer = await CanvasKitLayerRenderer.create(canvaskitMode, canvaskitSurfaceRequest);
      } catch (error) {
        console.error('[main] CanvasKit 초기화 실패, Canvas2D로 폴백합니다:', error);
        renderBackend = 'canvas2d';
        renderBackendFallbackReason = 'canvaskitInitializationFailed';
      }
    }
    msg.textContent = 'HWP 파일을 선택해주세요.';

    const container = document.getElementById('scroll-container')!;
    canvasView = new CanvasView(
      container,
      wasm,
      eventBus,
      renderBackend,
      renderProfile,
      canvaskitRenderer,
    );

    // 눈금자 초기화
    ruler = new Ruler(
      document.getElementById('h-ruler') as HTMLCanvasElement,
      document.getElementById('v-ruler') as HTMLCanvasElement,
      container,
      eventBus,
      wasm,
      canvasView.getVirtualScroll(),
      canvasView.getViewportManager(),
    );

    inputHandler = new InputHandler(
      container, wasm, eventBus,
      canvasView.getVirtualScroll(),
      canvasView.getViewportManager(),
    );
    inputHandler.setEditMode(editMode);

    toolbar = new Toolbar(document.getElementById('style-bar')!, wasm, eventBus, dispatcher);
    toolbar.setEnabled(false);

    // InputHandler에 커맨드 디스패처 및 컨텍스트 메뉴 주입
    inputHandler.setDispatcher(dispatcher);
    inputHandler.setContextMenu(new ContextMenu(dispatcher, registry));
    inputHandler.setCommandPalette(new CommandPalette(registry, dispatcher));
    inputHandler.setCellSelectionRenderer(
      new CellSelectionRenderer(container, canvasView.getVirtualScroll()),
    );
    inputHandler.setTableObjectRenderer(
      new TableObjectRenderer(container, canvasView.getVirtualScroll()),
    );
    inputHandler.setTableResizeRenderer(
      new TableResizeRenderer(container, canvasView.getVirtualScroll()),
    );
    inputHandler.setPictureObjectRenderer(
      new TableObjectRenderer(container, canvasView.getVirtualScroll(), true),
    );

    new MenuBar(document.getElementById('menu-bar')!, eventBus, dispatcher, registry);

    // 툴바 내 data-cmd 버튼 클릭 → 커맨드 디스패치
    document.querySelectorAll('.tb-btn[data-cmd]').forEach(btn => {
      btn.addEventListener('mousedown', (e) => {
        e.preventDefault();
        const cmd = (btn as HTMLElement).dataset.cmd;
        if (cmd) dispatcher.dispatch(cmd, { anchorEl: btn as HTMLElement });
      });
    });

    // 스플릿 버튼 드롭다운 메뉴
    document.querySelectorAll('.tb-split').forEach(split => {
      const arrow = split.querySelector('.tb-split-arrow');
      if (arrow) {
        arrow.addEventListener('mousedown', (e) => {
          e.preventDefault();
          e.stopPropagation();
          // 다른 열린 메뉴 닫기
          document.querySelectorAll('.tb-split.open').forEach(s => {
            if (s !== split) s.classList.remove('open');
          });
          split.classList.toggle('open');
        });
      }
      split.querySelectorAll('.tb-split-item[data-cmd]').forEach(item => {
        item.addEventListener('mousedown', (e) => {
          e.preventDefault();
          split.classList.remove('open');
          const cmd = (item as HTMLElement).dataset.cmd;
          if (cmd) dispatcher.dispatch(cmd, { anchorEl: item as HTMLElement });
        });
      });
    });
    // 외부 클릭 시 스플릿 메뉴 닫기
    document.addEventListener('mousedown', () => {
      document.querySelectorAll('.tb-split.open').forEach(s => s.classList.remove('open'));
    });

    // #780: 도구 모음/서식 도구 모음 영역 mousedown 시 focus 이동 방지
    // — 편집 영역의 텍스트 선택(cursor.anchor)이 보존되어야 서식 적용이 동작함
    for (const id of ['icon-toolbar', 'style-bar']) {
      const el = document.getElementById(id);
      if (el) el.addEventListener('mousedown', (e) => {
        if ((e.target as HTMLElement).tagName !== 'INPUT' && (e.target as HTMLElement).tagName !== 'SELECT') {
          e.preventDefault();
        }
      });
    }

    setupFileInput();
    setupZoomControls();
    setupEventListeners();
    setupGlobalShortcuts();
    void loadFromUrlParam();
    void offerAutosaveRecoveryIfIdle();
    installPwaFileHandling(window as FileHandlingWindowLike, {
      openDocumentBytes(payload) {
        eventBus.emit('open-document-bytes', payload);
      },
      notifyUnsupportedFile(fileName) {
        showLoadError(new Error(`지원하지 않는 파일 형식입니다: ${fileName}. HWP/HWPX 파일만 지원합니다.`));
      },
      notifyError(error) {
        showLoadError(error);
      },
      notifyMultipleFiles(count) {
        console.warn(`[pwa-file-handling] 여러 파일(${count}개)이 전달되어 첫 번째 파일만 엽니다.`);
      },
    });

    // E2E 테스트용 전역 노출 (개발 모드 전용)
    if (import.meta.env.DEV) {
      (window as any).__inputHandler = inputHandler;
      (window as any).__canvasView = canvasView;
      (window as any).__renderBackend = renderBackend;
      (window as any).__renderBackendRequest = renderBackendRequest;
      (window as any).__rendererRuntimeRequest = rendererRuntimeRequest;
      (window as any).__renderBackendFallbackReason = renderBackendFallbackReason;
      (window as any).__canvaskitRenderMode = canvaskitMode;
      (window as any).__canvaskitSurfaceRequest = canvaskitSurfaceRequest;
      (window as any).__renderProfile = renderProfile;
    }
    rendererInitialized = true;
  } catch (error) {
    rendererInitializationError = error instanceof Error ? error.message : String(error);
    msg.textContent = `WASM 초기화 실패: ${error}`;
    console.error('[main] WASM 초기화 실패:', error);
  }
}

/**
 * 전역 단축키 핸들러 — InputHandler.active 여부와 무관하게 동작해야 하는 단축키.
 * 예: 문서 미로드 상태에서도 Alt+N(새 문서), Ctrl+O(열기) 등.
 */
function setupGlobalShortcuts(): void {
  document.addEventListener('keydown', (e) => {
    // input/textarea 등 편집 가능 요소 내부에서는 무시
    const target = e.target as HTMLElement;
    if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) return;
    // InputHandler가 활성 상태이면 자체 처리에 맡김
    if (inputHandler?.isActive()) return;

    const ctrlOrMeta = e.ctrlKey || e.metaKey;

    // Alt+N / Alt+ㅜ → 새 문서 (문서 미로드 상태에서도 동작)
    if (e.altKey && !ctrlOrMeta && !e.shiftKey) {
      if (e.key === 'n' || e.key === 'N' || e.key === 'ㅜ') {
        e.preventDefault();
        dispatcher.dispatch('file:new-doc');
        return;
      }
    }
    // Ctrl/Cmd+O → 열기 (문서 미로드 상태에서도 동작)
    if (ctrlOrMeta && !e.altKey && !e.shiftKey) {
      if (e.key === 'o' || e.key === 'O' || e.key === 'ㅐ') {
        e.preventDefault();
        dispatcher.dispatch('file:open');
        return;
      }
    }
  }, false);
}

function setupFileInput(): void {
  const fileInput = document.getElementById('file-input') as HTMLInputElement;

  fileInput.addEventListener('change', async (e) => {
    const input = e.target as HTMLInputElement;
    const skipUnsavedGuard = input.dataset.skipUnsavedGuard === 'true';
    delete input.dataset.skipUnsavedGuard;
    const file = input.files?.[0];
    if (!file) return;
    const name = file.name.toLowerCase();
    if (!name.endsWith('.hwp') && !name.endsWith('.hwpx')) {
      alert('HWP/HWPX 파일만 지원합니다.');
      fileInput.value = '';
      return;
    }
    await loadFile(file, { skipUnsavedGuard });
    fileInput.value = '';
  });

  // 문서 전체에서 브라우저 기본 드롭 동작 방지 (파일 열기/다운로드 방지)
  document.addEventListener('dragover', (e) => e.preventDefault());
  document.addEventListener('drop', (e) => e.preventDefault());

  // 드래그 앤 드롭 지원 (scroll-container 영역)
  const container = document.getElementById('scroll-container')!;
  container.addEventListener('dragover', (e) => {
    e.preventDefault();
    container.classList.add('drag-over');
  });
  container.addEventListener('dragleave', () => {
    container.classList.remove('drag-over');
  });
  container.addEventListener('drop', async (e) => {
    e.preventDefault();
    container.classList.remove('drag-over');
    const file = e.dataTransfer?.files[0];
    if (!file) return;
    const dropName = file.name.toLowerCase();
    const imageExts = ['.png', '.jpg', '.jpeg', '.gif', '.bmp', '.webp'];
    const isImage = imageExts.some(ext => dropName.endsWith(ext));
    const isDoc = dropName.endsWith('.hwp') || dropName.endsWith('.hwpx');
    if (!isImage && !isDoc) {
      alert('HWP/HWPX 파일 또는 이미지 파일만 지원합니다.');
      return;
    }

    // [#1439] 보안: 드롭으로 로컬 파일을 읽는 동작은 기본에서 제외하고, 사용자가
    // 명시적으로 [열기]를 눌러 동의한 경우에만 진행한다 (확장/웹 공통).
    const confirmed = await showDropConfirmDialog(file.name);
    if (!confirmed) return;

    if (isImage) {
      if (!inputHandler || wasm.pageCount === 0) return;
      const data = new Uint8Array(await file.arrayBuffer());
      const ext = file.name.split('.').pop()?.toLowerCase() || 'png';
      const img = new Image();
      const url = URL.createObjectURL(file);
      try {
        img.src = url;
        await img.decode();
        const result = inputHandler.insertDroppedImageAtClientPoint(
          data,
          ext,
          img.naturalWidth,
          img.naturalHeight,
          file.name,
          e.clientX,
          e.clientY,
        );
        if (!result.ok) {
          showToast({
            message: `그림 삽입에 실패했습니다.\n${result.error ?? '삽입 위치 또는 이미지 정보를 확인할 수 없습니다.'}`,
            durationMs: 6000,
          });
        }
      } catch {
        console.warn('[drop] 이미지 디코딩 실패:', file.name);
        showToast({
          message: '그림을 삽입할 수 없습니다.\n브라우저가 이 이미지 파일을 읽지 못했습니다.',
          durationMs: 6000,
        });
      } finally {
        URL.revokeObjectURL(url);
      }
      return;
    }

    // HWP/HWPX — loadFile 내부 unsaved 가드는 드롭 확인 이후에 동작한다.
    await loadFile(file);
  });
}

function setupZoomControls(): void {
  if (!canvasView) return;
  const vm = canvasView.getViewportManager();

  document.getElementById('sb-zoom-in')!.addEventListener('click', () => {
    vm.setZoom(vm.getZoom() + 0.1);
  });
  document.getElementById('sb-zoom-out')!.addEventListener('click', () => {
    vm.setZoom(vm.getZoom() - 0.1);
  });

  // 폭 맞춤: 용지 폭에 맞게 줌 조절
  document.getElementById('sb-zoom-fit-width')!.addEventListener('click', () => {
    if (wasm.pageCount === 0) return;
    const container = document.getElementById('scroll-container')!;
    const containerWidth = container.clientWidth - 40; // 좌우 여백 제외
    const pageInfo = wasm.getPageInfo(0);
    // pageInfo.width는 이미 px 단위 (96dpi 기준)
    const zoom = containerWidth / pageInfo.width;
    console.log(`[zoom-fit-width] container=${containerWidth} page=${pageInfo.width} zoom=${zoom.toFixed(3)}`);
    vm.setZoom(Math.max(0.1, Math.min(zoom, 4.0)));
  });

  // 쪽 맞춤: 한 페이지 전체가 보이도록 줌 조절
  document.getElementById('sb-zoom-fit')!.addEventListener('click', () => {
    if (wasm.pageCount === 0) return;
    const container = document.getElementById('scroll-container')!;
    const containerWidth = container.clientWidth - 40;
    const containerHeight = container.clientHeight - 40;
    const pageInfo = wasm.getPageInfo(0);
    // pageInfo.width/height는 이미 px 단위 (96dpi 기준)
    const zoomW = containerWidth / pageInfo.width;
    const zoomH = containerHeight / pageInfo.height;
    console.log(`[zoom-fit-page] containerW=${containerWidth} containerH=${containerHeight} pageW=${pageInfo.width} pageH=${pageInfo.height} zoomW=${zoomW.toFixed(3)} zoomH=${zoomH.toFixed(3)}`);
    vm.setZoom(Math.max(0.1, Math.min(zoomW, zoomH, 4.0)));
  });

  // 모바일: 줌 값 클릭 → 100% 토글
  document.getElementById('sb-zoom-val')!.addEventListener('click', () => {
    const currentZoom = vm.getZoom();
    if (Math.abs(currentZoom - 1.0) < 0.05) {
      // 현재 100% → 쪽 맞춤으로 전환
      document.getElementById('sb-zoom-fit')!.click();
    } else {
      // 현재 쪽 맞춤/기타 → 100%로 전환
      vm.setZoom(1.0);
    }
  });

  document.addEventListener('keydown', (e) => {
    if (!e.ctrlKey && !e.metaKey) return;
    if (e.key === '=' || e.key === '+') {
      e.preventDefault();
      vm.setZoom(vm.getZoom() + 0.1);
    } else if (e.key === '-') {
      e.preventDefault();
      vm.setZoom(vm.getZoom() - 0.1);
    } else if (e.key === '0') {
      e.preventDefault();
      vm.setZoom(1.0);
    }
  });
}

let totalSections = 1;

function setupEventListeners(): void {
  eventBus.on('current-page-changed', (page, _total) => {
    const pageIdx = page as number;
    sbPage().textContent = `${pageIdx + 1} / ${_total} 쪽`;

    // 구역 정보: 현재 페이지의 sectionIndex로 갱신
    if (wasm.pageCount > 0) {
      try {
        const pageInfo = wasm.getPageInfo(pageIdx);
        sbSection().textContent = `구역: ${pageInfo.sectionIndex + 1} / ${totalSections}`;
      } catch { /* 무시 */ }
    }
  });

  eventBus.on('zoom-level-display', (zoom) => {
    sbZoomVal().textContent = `${Math.round((zoom as number) * 100)}%`;
  });

  // 삽입/수정 모드 토글
  eventBus.on('insert-mode-changed', (insertMode) => {
    document.getElementById('sb-mode')!.textContent = (insertMode as boolean) ? '삽입' : '수정';
  });

  eventBus.on('document-mutated', (reason) => {
    documentState.markDirty(typeof reason === 'string' ? reason : 'document-mutated');
  });

  eventBus.on('document-changed', (reason) => {
    documentState.markDirty(typeof reason === 'string' ? reason : 'document-changed');
  });

  eventBus.on('document-dirty-changed', () => {
    eventBus.emit('command-state-changed');
  });

  eventBus.on('local-fonts-changed', () => {
    if (wasm.pageCount > 0) {
      canvasView?.loadDocument();
    }
  });

  eventBus.on('autosave-settings-changed', () => {
    autosaveManager.updateSchedule(autosaveScheduleFromUserSettings());
  });

  // 필드 정보 표시
  const sbField = document.getElementById('sb-field');
  eventBus.on('field-info-changed', (info) => {
    if (!sbField) return;
    const fi = info as { fieldId: number; fieldType: string; guideName?: string } | null;
    if (fi) {
      const label = fi.guideName || `#${fi.fieldId}`;
      sbField.textContent = `[누름틀] ${label}`;
      sbField.style.display = '';
    } else {
      sbField.textContent = '';
      sbField.style.display = 'none';
    }
  });

  // 개체 선택 시 회전/대칭 버튼 그룹 표시/숨김
  const rotateGroup = document.querySelector('.tb-rotate-group') as HTMLElement | null;
  let noteToolbarActive = false;
  if (rotateGroup) {
    eventBus.on('picture-object-selection-changed', (selected) => {
      rotateGroup.style.display = (selected as boolean) && !noteToolbarActive ? '' : 'none';
    });
  }

  // 머리말/꼬리말 편집 모드 시 도구상자 전환 + 본문 dimming
  const hfGroup = document.querySelector('.tb-headerfooter-group') as HTMLElement | null;
  const hfLabel = hfGroup?.querySelector('.tb-hf-label') as HTMLElement | null;
  const noteGroup = document.querySelector('.tb-note-group') as HTMLElement | null;
  const defaultTbGroups = document.querySelectorAll('#icon-toolbar > .tb-group:not(.tb-headerfooter-group):not(.tb-note-group):not(.tb-rotate-group), #icon-toolbar > .tb-sep');
  const scrollContainer = document.getElementById('scroll-container');
  const styleBar = document.getElementById('style-bar');

  eventBus.on('headerFooterModeChanged', (mode) => {
    const isActive = (mode as string) !== 'none';
    // 도구상자 전환
    if (hfGroup) {
      hfGroup.style.display = isActive ? '' : 'none';
    }
    if (hfLabel) {
      hfLabel.textContent = (mode as string) === 'header' ? '머리말' : (mode as string) === 'footer' ? '꼬리말' : '';
    }
    defaultTbGroups.forEach((el) => {
      (el as HTMLElement).style.display = isActive ? 'none' : '';
    });
    // 서식 도구 모음은 머리말/꼬리말 편집 시에도 유지 (문단/글자 모양 설정 필요)
    // 본문 dimming
    if (scrollContainer) {
      if (isActive) {
        scrollContainer.classList.add('hf-editing');
      } else {
        scrollContainer.classList.remove('hf-editing');
      }
    }
  });

  eventBus.on('footnoteModeChanged', (active) => {
    const isActive = active as boolean;
    noteToolbarActive = isActive;
    if (noteGroup) {
      noteGroup.style.display = isActive ? '' : 'none';
    }
    if (rotateGroup && isActive) {
      rotateGroup.style.display = 'none';
    }
    defaultTbGroups.forEach((el) => {
      (el as HTMLElement).style.display = isActive ? 'none' : '';
    });
  });
}

/** 문서 초기화 공통 시퀀스 (loadFile, createNewDocument 양쪽에서 사용) */
function applySavedTextMarkSettings(): void {
  const view = userSettings.getViewSettings();
  wasm.setShowControlCodes(view.showControlCodes);
  wasm.setShowParagraphMarks(view.showParagraphMarks);
  syncTextMarkMenu(view.showControlCodes, view.showParagraphMarks);
  // #2204: 짤림보기(잘림 보기) 저장 설정 복원. clipView=켜짐 => clip 미적용(clipEnabled=false).
  const clipEnabled = !view.clipView;
  wasm.setClipEnabled(clipEnabled);
  syncClipMenu(clipEnabled);
}

async function initializeDocument(docInfo: DocumentInfo, displayName: string): Promise<void> {
  const msg = sbMessage();
  let normalizedDuringLoad = false;
  try {
    console.log('[initDoc] 1. 폰트 로딩 시작');
    await updateLoadProgress(55, '폰트 준비 중...');
    if (docInfo.fontsUsed?.length) {
      await loadWebFonts(docInfo.fontsUsed, (loaded, total) => {
        const fontPercent = total > 0 ? 55 + Math.round((loaded / total) * 20) : 65;
        msg.textContent = `파일 로딩 ${fontPercent}% - 폰트 로딩 중... (${loaded}/${total})`;
      }, extensionViewerSettings);
    }
    console.log('[initDoc] 2. 폰트 로딩 완료');
    await updateLoadProgress(75, '문서 상태 적용 중...');
    totalSections = docInfo.sectionCount ?? 1;
    sbSection().textContent = `구역: 1 / ${totalSections}`;
    applySavedTextMarkSettings();
    console.log('[initDoc] 3. inputHandler deactivate');
    inputHandler?.deactivate();
    console.log('[initDoc] 4. canvasView loadDocument');
    await updateLoadProgress(82, '페이지 렌더 준비 중...');
    canvasView?.loadDocument();
    prepareCanvasKitLocalFonts(docInfo.fontsUsed);
    console.log('[initDoc] 5. toolbar setEnabled');
    await updateLoadProgress(90, '도구 모음 준비 중...');
    toolbar?.setEnabled(true);
    console.log('[initDoc] 6. toolbar initFontDropdown + initStyleDropdown');
    toolbar?.initFontDropdown(docInfo.fontsUsed);
    toolbar?.initStyleDropdown();
    console.log('[initDoc] 7. 사전 검증 및 로컬 글꼴 확인');
    await updateLoadProgress(94, '문서 검증 및 글꼴 확인 중...');

    // #177: HWPX 비표준 lineseg 감지 → 경고 있으면 모달로 사용자 선택 요청
    try {
      if (wasm.getSourceFormat() === 'hwpx') {
        const report = wasm.getValidationWarnings();
        console.log(`[validation] ${report.count} warnings`, report.summary);
        if (report.count > 0) {
          const choice = await showValidationModalIfNeeded(report);
          console.log(`[validation] user choice: ${choice}`);
          if (choice === 'auto-fix') {
            const n = wasm.reflowLinesegs();
            console.log(`[validation] reflowed ${n} paragraphs`);
            if (n > 0) {
              // 렌더 재계산
              canvasView?.loadDocument();
              msg.textContent = `${displayName} (비표준 lineseg ${n}건 자동 보정됨)`;
              normalizedDuringLoad = true;
            }
          }
        }
      }
    } catch (e) {
      console.warn('[validation] 감지/보정 실패 (치명적이지 않음):', e);
    }

    await promptLocalFontsIfNeeded(docInfo, displayName);

    // 로컬 글꼴 감지 결과가 뷰를 갱신한 뒤에 캐럿을 연결해야 입력 포커스가 재설정과 경합하지 않는다.
    console.log('[initDoc] 8. inputHandler activateWithCaretPosition');
    await updateLoadProgress(96, '편집 상태 초기화 중...');
    inputHandler?.activateWithCaretPosition();
    // 최종 단계 뒤에는 비동기 작업이 없으므로 100% progress paint를 기다리지 않는다.
    msg.textContent = displayName;
    console.log('[initDoc] 9. 완료');

    if (normalizedDuringLoad) {
      documentState.markDirty('validation-auto-fix');
    } else {
      documentState.markClean('document-initialized');
    }
  } catch (error) {
    console.error('[initDoc] 오류:', error);
    if (window.innerWidth < 768) alert(`초기화 오류: ${error}`);
  }
}

async function promptLocalFontsIfNeeded(docInfo: DocumentInfo, displayName: string): Promise<void> {
  if (!docInfo.fontsUsed?.length) return;

  const msg = sbMessage();
  try {
    await loadStoredLocalFonts();
    const report = analyzeDocumentFonts(docInfo.fontsUsed);
    if (!report.shouldPromptLocalAccess) return;

    const choice = await showLocalFontsModalIfNeeded(report, {
      disableExternalWebFonts: extensionViewerSettings.disableExternalWebFonts,
    });
    if (choice !== 'detect') return;

    msg.textContent = '로컬 글꼴 감지 중...';
    const fonts = await detectLocalFonts({
      force: true,
      includeRegistered: true,
      candidateFamilies: docInfo.fontsUsed,
    });
    const nextReport = analyzeDocumentFonts(docInfo.fontsUsed);
    eventBus.emit('local-fonts-changed', { fonts, report: nextReport });
    prepareCanvasKitLocalFonts(docInfo.fontsUsed);
    const state = getLocalFontState();
    const resultLabel = state.source === 'font-presence-probe' ? '확인됨' : '감지됨';
    msg.textContent = `${displayName} (로컬 글꼴 ${fonts.length}개 ${resultLabel})`;
    showToast({
      message: `로컬 글꼴 ${fonts.length}개를 ${resultLabel.replace('됨', '')}하고 저장했습니다.\n다음 문서 로드부터 감지 결과를 재사용합니다.`,
      durationMs: 5000,
    });
  } catch (error) {
    console.warn('[local-fonts] 감지 안내/실행 실패 (치명적이지 않음):', error);
    msg.textContent = displayName;
    showToast({
      message: '로컬 글꼴 감지에 실패했습니다.\n웹 대체 글꼴로 계속 표시합니다.',
      durationMs: 8000,
    });
  }
}

async function loadFile(file: File, options: { skipUnsavedGuard?: boolean } = {}): Promise<boolean> {
  try {
    if (!options.skipUnsavedGuard) {
      const canReplace = await confirmSaveBeforeReplacingDocument(commandServices);
      if (!canReplace) return false;
    }
    const startTime = performance.now();
    await updateLoadProgress(0, '파일 읽는 중...');
    const data = new Uint8Array(await file.arrayBuffer());
    await updateLoadProgress(15, '파일 읽기 완료');
    await loadBytes(data, file.name, null, startTime, { dataReadProgressShown: true });
    return true;
  } catch (error) {
    showLoadError(error);
    return false;
  }
}

async function loadBytes(
  data: Uint8Array,
  fileName: string,
  fileHandle: typeof wasm.currentFileHandle,
  startTime = performance.now(),
  options: { dataReadProgressShown?: boolean } = {},
): Promise<void> {
  if (!options.dataReadProgressShown) {
    await updateLoadProgress(0, '문서 데이터 준비 중...');
  }
  await updateLoadProgress(25, '문서 파싱 및 쪽 계산 중...');
  const docInfo = wasm.loadDocument(data, fileName);
  await updateLoadProgress(45, '자동 저장 준비 중...');
  wasm.currentFileHandle = fileHandle;
  await autosaveManager.beginDocument(
    { fileName: wasm.fileName, sourceFormat: wasm.getSourceFormat() },
    { discardPreviousDraft: true },
  );
  await updateLoadProgress(50, '문서 초기화 중...');
  const elapsed = performance.now() - startTime;
  await initializeDocument(docInfo, `${fileName} — ${docInfo.pageCount}페이지 (${elapsed.toFixed(1)}ms)`);
}

function shouldSkipInitialAutosaveRecovery(): boolean {
  const params = new URLSearchParams(window.location.search);
  return params.has('url');
}

async function offerAutosaveRecoveryIfIdle(): Promise<void> {
  if (shouldSkipInitialAutosaveRecovery()) return;

  try {
    const drafts = (await listAutosaveDrafts()).filter((draft) => draft.data.byteLength > 0);
    if (drafts.length === 0) return;
    if (wasm.pageCount > 0 || documentState.isDirty()) return;

    const choice = await showAutosaveRecoveryDialog(drafts);
    if (choice.action === 'later') return;
    if (choice.action === 'delete-all') {
      await clearAutosaveDrafts();
      showToast({ message: '복구 후보를 삭제했습니다.', durationMs: 2200 });
      return;
    }

    const draft = drafts.find((item) => item.id === choice.draftId);
    if (!draft) return;
    try {
      await restoreAutosaveDraft(draft);
    } catch (error) {
      showLoadError(error);
    }
  } catch (error) {
    console.warn('[autosave] 복구 후보 확인 실패:', error);
  }
}

async function restoreAutosaveDraft(draft: AutosaveDraft): Promise<void> {
  const fileName = recoveryFileName(draft.fileName, draft.sourceFormat);
  await loadBytes(new Uint8Array(draft.data), fileName, null);
  await deleteAutosaveDraft(draft.id);
  documentState.markDirty('autosave-recovered');
  showToast({
    message: `"${fileName}" 복구본을 열었습니다.\n원본 파일은 자동으로 덮어쓰지 않습니다.`,
    durationMs: 5000,
  });
}


async function createNewDocument(): Promise<void> {
  const msg = sbMessage();
  try {
    msg.textContent = '새 문서 생성 중...';
    const docInfo = wasm.createNewDocument();
    await autosaveManager.beginDocument(
      { fileName: wasm.fileName, sourceFormat: wasm.getSourceFormat() },
      { discardPreviousDraft: true },
    );
    await initializeDocument(docInfo, `새 문서.hwp — ${docInfo.pageCount}페이지`);
  } catch (error) {
    msg.textContent = `새 문서 생성 실패: ${error}`;
    console.error('[main] 새 문서 생성 실패:', error);
  }
}

async function canReplaceCurrentDocument(skipUnsavedGuard?: boolean): Promise<boolean> {
  return skipUnsavedGuard === true || await confirmSaveBeforeReplacingDocument(commandServices);
}

// 커맨드에서 새 문서 생성 호출
eventBus.on('create-new-document', (payload) => {
  void (async () => {
    const options = payload as { skipUnsavedGuard?: boolean } | undefined;
    if (!await canReplaceCurrentDocument(options?.skipUnsavedGuard)) return;
    await createNewDocument();
  })();
});
eventBus.on('open-document-bytes', async (payload) => {
  const data = payload as {
    bytes: Uint8Array;
    fileName: string;
    fileHandle: typeof wasm.currentFileHandle;
    skipUnsavedGuard?: boolean;
    /** 문서 비교 등: 로드 완료를 기다리는 쪽과 짝을 맞출 때만 전달 */
    requestId?: string;
  };
  const notifyDone = (ok: boolean, error?: string) => {
    if (!data.requestId) return;
    eventBus.emit('open-document-bytes:done', { requestId: data.requestId, ok, error });
  };
  try {
    if (!await canReplaceCurrentDocument(data.skipUnsavedGuard)) {
      notifyDone(false, '문서 열기가 취소되었습니다.');
      return;
    }
    await loadBytes(data.bytes, data.fileName, data.fileHandle);
    notifyDone(true);
  } catch (error) {
    // #265: WASM 파서 에러 (예: HWP 3.0 미지원) 를 사용자에게 전파
    showLoadError(error);
    const msg = error instanceof Error ? error.message : String(error);
    notifyDone(false, msg);
  }
});

// 수식 더블클릭 → 수식 편집 대화상자
eventBus.on('equation-edit-request', () => {
  dispatcher.dispatch('insert:equation-edit');
});

/**
 * URL 파라미터(?url=)로 전달된 HWP 파일을 자동 로드한다.
 * Chrome 확장 프로그램에서 뷰어 탭을 열 때 사용.
 */
async function loadFromUrlParam(): Promise<void> {
  const params = new URLSearchParams(window.location.search);
  const fileUrl = params.get('url');
  if (!fileUrl) return;

  const fileName = params.get('filename') || fileUrl.split('/').pop()?.split('?')[0] || 'document.hwp';
  const msg = sbMessage();

  try {
    msg.textContent = '파일 로딩 중...';
    console.log(`[loadFromUrlParam] ${fileUrl}`);

    let response: Response;

    // Chrome 확장 환경: Service Worker를 통한 CORS 우회 fetch
    if (typeof chrome !== 'undefined' && chrome.runtime?.sendMessage) {
      try {
        response = await fetch(fileUrl);
      } catch {
        // 직접 fetch 실패 시 Service Worker 프록시
        const result = await chrome.runtime.sendMessage({ type: 'fetch-file', url: fileUrl });
        if (result.error) throw new Error(result.error);
        const data = new Uint8Array(result.data);
        assertRemoteDocumentBytes(data);
        await loadBytes(data, fileName, null);
        return;
      }
    } else {
      response = await fetch(fileUrl);
    }

    if (!response.ok) throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    const contentType = response.headers.get('content-type');
    const buffer = await response.arrayBuffer();
    const data = new Uint8Array(buffer);
    assertRemoteDocumentBytes(data, contentType);
    await loadBytes(data, fileName, null);
  } catch (error) {
    // 로컬 file:// 로드 실패 + "파일 URL 액세스 허용" 미허용 → 전용 안내 (#1131)
    if (fileUrl.startsWith('file:') && typeof chrome !== 'undefined') {
      const allowed = await isFileSchemeAccessAllowed();
      if (allowed === false) {
        showFileUrlAccessGuidance();
        return;
      }
    }
    showLoadError(error);
  }
}

/**
 * 확장 프로그램의 "파일 URL에 대한 액세스 허용" 권한 상태를 조회한다 (#1131).
 *
 * 확장 페이지에서만 의미가 있다. API 부재(비-확장 환경 등) 시 판정 불가로
 * `null` 을 반환하여 호출부가 기존 동작(일반 에러)으로 폴백하도록 한다.
 *
 * @returns 허용=true, 미허용=false, 판정 불가=null
 */
async function isFileSchemeAccessAllowed(): Promise<boolean | null> {
  const ext = (typeof chrome !== 'undefined' ? chrome.extension : undefined) as
    | { isAllowedFileSchemeAccess?: () => Promise<boolean> }
    | undefined;
  if (!ext?.isAllowedFileSchemeAccess) return null;
  try {
    return await ext.isAllowedFileSchemeAccess();
  } catch {
    return null;
  }
}

/**
 * 로컬 file:// 문서를 열 때 "파일 URL 액세스 허용" 권한이 꺼져 있어 로드가
 * 실패한 경우, 일반 "Failed to fetch" 대신 원인과 해결 방법을 안내한다 (#1131).
 *
 * 설정 화면(chrome://extensions/?id=...)은 일반 링크로는 열리지 않으므로
 * 확장 컨텍스트의 chrome.tabs.create 로 연다.
 */
function showFileUrlAccessGuidance(): void {
  const errMsg = '로컬 파일을 열려면 확장 프로그램의 "파일 URL에 대한 액세스 허용"을 켜야 합니다.\n설정에서 권한을 허용한 뒤 파일을 다시 열어 주세요.';
  const sb = sbMessage();
  if (sb) sb.textContent = '파일 로드 실패: 파일 URL 액세스 권한이 필요합니다.';
  console.error('[main] file:// 로드 실패 — 파일 URL 액세스 미허용 (#1131)');
  showToast({
    message: errMsg,
    durationMs: 0, // 사용자가 읽고 직접 닫기
    confirmLabel: '확인',
    action: {
      label: '설정 열기',
      onClick: () => {
        if (typeof chrome !== 'undefined' && chrome.tabs?.create && chrome.runtime?.id) {
          chrome.tabs.create({ url: `chrome://extensions/?id=${chrome.runtime.id}` });
        }
      },
    },
  });
}

/**
 * 파일 로드 실패 시 사용자에게 에러를 명확히 알린다 (#265).
 *
 * 상태 표시줄은 22px 한 줄로 긴 에러 메시지가 ellipsis 로 잘리므로,
 * 우상단 토스트 (긴 메시지 줄바꿈 지원 · 사용자 닫기 · action 링크) 를
 * 병행 사용한다.
 */
function showLoadError(error: unknown): void {
  const raw = String(error).replace(/^Error:\s*/, '');
  const errMsg = `파일 로드 실패: ${raw}`;
  const sb = sbMessage();
  if (sb) sb.textContent = errMsg;
  console.error('[main] 파일 로드 실패:', error);
  showToast({
    message: errMsg,
    durationMs: 0, // 에러는 자동 페이드 없음 — 사용자가 읽고 닫기
    confirmLabel: '확인',
  });
}

const initPromise = initialize();

// ── iframe 연동 API (postMessage) ──
// 부모 페이지에서 postMessage로 에디터를 제어할 수 있다.
// 요청: { type: 'rhwp-request', id, method, params }
// 응답: { type: 'rhwp-response', id, result?, error? }
window.addEventListener('message', async (e) => {
  const msg = e.data;
  if (!msg || typeof msg !== 'object') return;

  // 기존 hwpctl-load 호환
  if (msg.type === 'hwpctl-load' && msg.data) {
    try {
      await initPromise;
      if (!await canReplaceCurrentDocument(Boolean(msg.skipUnsavedGuard))) {
        e.source?.postMessage({ type: 'rhwp-response', id: msg.id, error: '문서 열기가 취소되었습니다.' }, { targetOrigin: '*' });
        return;
      }
      const bytes = new Uint8Array(msg.data);
      await loadBytes(bytes, msg.fileName || 'document.hwp', null);
      e.source?.postMessage({ type: 'rhwp-response', id: msg.id, result: { pageCount: wasm.pageCount } }, { targetOrigin: '*' });
    } catch (err: any) {
      e.source?.postMessage({ type: 'rhwp-response', id: msg.id, error: err.message || String(err) }, { targetOrigin: '*' });
    }
    return;
  }

  // rhwp-request: 범용 API
  if (msg.type !== 'rhwp-request' || !msg.method) return;
  const { id, method, params } = msg;
  const reply = (result?: any, error?: string) => {
    e.source?.postMessage({ type: 'rhwp-response', id, result, error }, { targetOrigin: '*' });
  };

  try {
    switch (method) {
      case 'ready':
        // wasm 초기화 완료 후에만 true 응답 — race condition 방지 (#522)
        await initPromise;
        reply(true);
        break;
      case 'loadFile': {
        await initPromise;
        if (!await canReplaceCurrentDocument(Boolean(params?.skipUnsavedGuard))) {
          reply(undefined, '문서 열기가 취소되었습니다.');
          break;
        }
        const bytes = new Uint8Array(params.data);
        await loadBytes(bytes, params.fileName || 'document.hwp', null);
        reply({ pageCount: wasm.pageCount });
        break;
      }
      case 'pageCount':
        await initPromise;
        reply(wasm.pageCount);
        break;
      case 'getRendererDiagnostics': {
        await initPromise;
        const pageIndex = Number(params?.page ?? 0);
        if (!Number.isInteger(pageIndex) || pageIndex < 0) {
          reply(undefined, 'page must be a non-negative integer');
          break;
        }
        reply({
          request: rendererRuntimeRequest,
          initialized: rendererInitialized,
          initializationError: rendererInitializationError,
          effectiveBackend: rendererInitialized ? canvasView?.getRenderBackend() ?? null : null,
          backendFallbackReason: renderBackendFallbackReason,
          page: {
            index: pageIndex,
            canvaskit: canvasView?.getCanvasKitRenderDiagnostics(pageIndex) ?? null,
          },
        });
        break;
      }
      case 'getPageSvg':
        await initPromise;
        reply(wasm.renderPageSvg(params.page ?? 0));
        break;
      case 'exportHwp':
        await initPromise;
        reply(Array.from(wasm.exportHwp()));
        break;
      case 'exportHwpx':
        await initPromise;
        reply(Array.from(wasm.exportHwpx()));
        break;
      case 'exportHwpVerify':
        await initPromise;
        reply(JSON.parse(wasm.exportHwpVerify()));
        break;
      default:
        reply(undefined, `Unknown method: ${method}`);
    }
  } catch (err: any) {
    reply(undefined, err.message || String(err));
  }
});
