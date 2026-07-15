/**
 * 최근 열람 문서 저장소.
 *
 * 파일 메뉴 "최근 문서" 목록의 영속 저장을 담당한다. 재열기를 위해
 * `FileSystemFileHandle`을 IndexedDB에 structured-clone으로 저장한다(직렬화 아님).
 * 브라우저는 파일 절대경로를 노출하지 않으므로 파일명·형식·시각만 메타로 보관한다.
 *
 * 자동 백업(`rhwpStudioAutosave`)·비교 이력(`rhwpStudioDocHistory`)과 섞지 않기 위해
 * 별도 IndexedDB(`rhwpStudioRecent`)를 사용한다. IndexedDB를 쓸 수 없는 테스트/제한
 * 환경에서는 메모리 저장소로 폴백한다(핸들은 메모리 참조로만 유지).
 */

import type { FileSystemFileHandleLike } from '@/command/file-system-access';

const DB_NAME = 'rhwpStudioRecent';
const DB_VER = 1;
const STORE = 'recent';
const MAX_RECENT = 8;

export interface RecentDoc {
  /** 고유 ID (crypto.randomUUID) */
  id: string;
  /** 파일명 (경로 아님 — 브라우저 제약) */
  fileName: string;
  /** 원본 형식 ('hwp' | 'hwpx' | 'hml' 등) */
  sourceFormat: string;
  /** 마지막으로 연 시각 (epoch ms) */
  openedAt: number;
  /** 재열기용 파일 핸들 */
  handle: FileSystemFileHandleLike;
}

/** addRecentDoc 입력 (id/openedAt는 내부 생성) */
export type RecentDocInput = Pick<RecentDoc, 'fileName' | 'sourceFormat' | 'handle'>;

const memory = new Map<string, RecentDoc>();

function idbAvailable(): boolean {
  return typeof indexedDB !== 'undefined';
}

function createRecentId(): string {
  return globalThis.crypto?.randomUUID?.() ?? `recent_${Date.now()}_${Math.random().toString(36).slice(2, 9)}`;
}

function openDb(): Promise<IDBDatabase | null> {
  if (!idbAvailable()) return Promise.resolve(null);
  return new Promise((resolve) => {
    const req = indexedDB.open(DB_NAME, DB_VER);
    req.onerror = () => resolve(null);
    req.onsuccess = () => resolve(req.result);
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains(STORE)) {
        db.createObjectStore(STORE, { keyPath: 'id' });
      }
    };
  });
}

async function withDb<T>(fn: (db: IDBDatabase) => Promise<T>, fallback: () => Promise<T>): Promise<T> {
  const db = await openDb();
  if (!db) return fallback();
  try {
    return await fn(db);
  } finally {
    db.close();
  }
}

function getAllRows(db: IDBDatabase): Promise<RecentDoc[]> {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE, 'readonly');
    const req = tx.objectStore(STORE).getAll();
    req.onsuccess = () => resolve(((req.result as RecentDoc[]) ?? []));
    req.onerror = () => reject(req.error);
  });
}

function putRow(db: IDBDatabase, row: RecentDoc): Promise<void> {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE, 'readwrite');
    tx.objectStore(STORE).put(row);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

function deleteRow(db: IDBDatabase, id: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE, 'readwrite');
    tx.objectStore(STORE).delete(id);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

/** 동일 파일 판정: 핸들 isSameEntry 우선, 미지원 시 파일명 비교. */
async function isSameFile(a: FileSystemFileHandleLike, b: FileSystemFileHandleLike, fallbackName: string): Promise<boolean> {
  if (typeof a.isSameEntry === 'function') {
    try {
      return await a.isSameEntry(b);
    } catch {
      // isSameEntry 실패 시 이름 비교로 폴백
    }
  }
  return a.name === fallbackName;
}

/** 최신순(openedAt 내림차순)으로 정렬해 상한까지 자른다. */
function sortAndTrim(rows: RecentDoc[]): RecentDoc[] {
  return rows.sort((a, b) => b.openedAt - a.openedAt).slice(0, MAX_RECENT);
}

/**
 * 최근 문서를 추가한다. 동일 파일이 이미 있으면 제거 후 맨 앞에 다시 넣고,
 * 최대 {@link MAX_RECENT}개를 유지한다.
 */
export async function addRecentDoc(input: RecentDocInput): Promise<void> {
  const entry: RecentDoc = {
    id: createRecentId(),
    fileName: input.fileName,
    sourceFormat: input.sourceFormat,
    openedAt: Date.now(),
    handle: input.handle,
  };

  await withDb(
    async (db) => {
      const rows = await getAllRows(db);
      // 중복 제거
      for (const row of rows) {
        if (await isSameFile(entry.handle, row.handle, row.fileName)) {
          await deleteRow(db, row.id);
        }
      }
      await putRow(db, entry);
      // 상한 초과분 삭제
      const after = sortAndTrim(await getAllRows(db));
      const keep = new Set(after.map((r) => r.id));
      for (const row of await getAllRows(db)) {
        if (!keep.has(row.id)) await deleteRow(db, row.id);
      }
    },
    async () => {
      for (const [id, row] of memory) {
        if (await isSameFile(entry.handle, row.handle, row.fileName)) memory.delete(id);
      }
      memory.set(entry.id, entry);
      const keep = new Set(sortAndTrim([...memory.values()]).map((r) => r.id));
      for (const id of [...memory.keys()]) {
        if (!keep.has(id)) memory.delete(id);
      }
    },
  );
}

/** 최근 문서 목록(최신순). */
export async function listRecentDocs(): Promise<RecentDoc[]> {
  return withDb(
    async (db) => sortAndTrim(await getAllRows(db)),
    async () => sortAndTrim([...memory.values()]),
  );
}

/** 특정 최근 문서를 제거한다(파일 이동/삭제로 재열기 실패 시). */
export async function removeRecentDoc(id: string): Promise<void> {
  memory.delete(id);
  await withDb(
    async (db) => deleteRow(db, id),
    async () => {},
  );
}

/** 최근 문서 목록 전체 삭제. */
export async function clearRecentDocs(): Promise<void> {
  memory.clear();
  await withDb(
    async (db) =>
      new Promise<void>((resolve, reject) => {
        const tx = db.transaction(STORE, 'readwrite');
        tx.objectStore(STORE).clear();
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      }),
    async () => {},
  );
}
