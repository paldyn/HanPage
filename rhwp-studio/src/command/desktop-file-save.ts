import type { DesktopSaveHandler } from '../core/desktop-bridge.ts';
import type { SaveDocumentResult } from './file-system-access.ts';

/** Native writes participate in the same reported-export transaction as web saves. */
export async function saveDocumentToDesktop(options: {
  blob: Blob;
  suggestedName: string;
  forceSaveAs: boolean;
  save: DesktopSaveHandler;
}): Promise<SaveDocumentResult | 'cancelled'> {
  const result = await options.save({
    bytes: new Uint8Array(await options.blob.arrayBuffer()),
    suggestedName: options.suggestedName,
    saveAs: options.forceSaveAs,
  });
  if (result.status === 'cancelled') return 'cancelled';
  if (result.status === 'failed') throw new Error(result.message);
  return { method: 'save-picker', handle: null, fileName: result.fileName };
}
