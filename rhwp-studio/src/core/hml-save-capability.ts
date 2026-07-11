export interface HmlSavabilityMetadata {
  hmlSavable?: unknown;
  saveBlockers?: unknown;
}

export interface HmlSaveBlocker {
  code: string;
  xmlPath: string;
  message: string;
}

export interface HmlSaveCapability {
  hmlEnabled: boolean;
  diagnostic: string | null;
}

export interface NormalizedHmlSaveState {
  hmlSavable: boolean;
  saveBlockers: HmlSaveBlocker[];
}

export interface HmlSaveContext<T extends HmlSavabilityMetadata> {
  metadata: T | null;
  exporterAvailable: boolean;
}

function isHmlSaveBlocker(value: unknown): value is HmlSaveBlocker {
  if (!value || typeof value !== 'object') return false;
  const blocker = value as Partial<HmlSaveBlocker>;
  return typeof blocker.code === 'string'
    && typeof blocker.xmlPath === 'string'
    && typeof blocker.message === 'string';
}

export function normalizeHmlSaveState(metadata: unknown): NormalizedHmlSaveState | null {
  if (!metadata || typeof metadata !== 'object') return null;
  const candidate = metadata as { hmlSavable?: unknown; saveBlockers?: unknown };
  const blockersAreValid = Array.isArray(candidate.saveBlockers)
    && candidate.saveBlockers.every(isHmlSaveBlocker);
  const saveBlockers: HmlSaveBlocker[] = blockersAreValid
    ? candidate.saveBlockers as HmlSaveBlocker[]
    : [];
  return {
    hmlSavable: candidate.hmlSavable === true && blockersAreValid && saveBlockers.length === 0,
    saveBlockers,
  };
}

export function readHmlSaveContext<T extends HmlSavabilityMetadata>(
  readMetadata: () => T | null,
  readExporterAvailable: () => boolean,
): HmlSaveContext<T> {
  let metadata: T | null = null;
  let exporterAvailable = false;
  try {
    metadata = readMetadata();
  } catch {
    // Missing or stale WASM metadata must disable HML without blocking conversion saves.
  }
  try {
    exporterAvailable = readExporterAvailable() === true;
  } catch {
    // Missing or stale WASM export bindings fail closed.
  }
  return { metadata, exporterAvailable };
}

export function resolveHmlSaveCapability(
  metadata: HmlSavabilityMetadata | null,
  exporterAvailable: boolean,
): HmlSaveCapability {
  const saveState = normalizeHmlSaveState(metadata);
  if (!saveState) {
    return { hmlEnabled: false, diagnostic: 'HML 저장 정보를 확인할 수 없습니다.' };
  }
  if (!saveState.hmlSavable) {
    return { hmlEnabled: false, diagnostic: '보존할 수 없는 요소가 있어 HML 저장이 차단되었습니다.' };
  }
  if (!exporterAvailable) {
    return { hmlEnabled: false, diagnostic: '현재 WASM 빌드는 HML 저장을 지원하지 않습니다.' };
  }
  return { hmlEnabled: true, diagnostic: null };
}
