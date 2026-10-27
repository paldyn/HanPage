import type { SaveFormat } from './save-format.ts';

/**
 * HanPage 데스크톱의 PDF 변환 저장 형식 기억.
 *
 * 웹은 File System Access handle 로 "이 PDF 를 어느 형식으로 저장했는지" 기억해 다음 저장부터 형식을
 * 다시 묻지 않는다(`markConvertedPdfSaveHandle`). 데스크톱 네이티브 저장은 handle 이 없으므로 같은
 * 문서 인스턴스(`documentGeneration`, 문서를 열거나 새로 만들 때만 바뀜) 단위로 기억한다.
 */
let remembered: { generation: number; format: SaveFormat } | null = null;

function pdfOutputFormat(fileName: string, fallback: SaveFormat): SaveFormat {
  if (/\.hwpx$/i.test(fileName.trim())) return 'hwpx';
  if (/\.hwp$/i.test(fileName.trim())) return 'hwp';
  return fallback;
}

/** 저장된 실제 파일명의 확장자를 우선하고, 없으면 고른 형식을 기억한다. */
export function rememberDesktopConvertedPdfSave(generation: unknown, fileName: string, chosen: SaveFormat): void {
  remembered = typeof generation === 'number' ? { generation, format: pdfOutputFormat(fileName, chosen) } : null;
}

/** 같은 문서 인스턴스에서 앞서 변환 저장한 형식. 다른 문서이거나 기억이 없으면 `null`. */
export function desktopConvertedPdfSaveFormat(generation: unknown): SaveFormat | null {
  if (remembered === null || typeof generation !== 'number') return null;
  return remembered.generation === generation ? remembered.format : null;
}

export function forgetDesktopConvertedPdfSave(): void {
  remembered = null;
}
