import { isTextEditingTarget } from '../command/document-shortcut-guard.ts';

/**
 * HanPage Desktop(macOS) 네이티브 편집 메뉴 보호.
 *
 * 네이티브 메뉴의 실행 취소·오려두기·복사·붙이기·모두 선택은 포커스와 무관하게 `edit:*` 문서 명령으로
 * 온다. 문서 밖 입력창(AI 도우미 패널, 대화상자)에 커서가 있으면 문서를 바꾸지 않고 그 입력창에 맡긴다.
 * 편집기 자체의 숨김 입력 영역은 이 컨테이너 밖이라 문서 명령으로 그대로 보낸다.
 */
const TEXT_INPUT_HOSTS = '#side-panel-host, .modal-overlay';
const NATIVE_TEXT_EDIT: Record<string, 'undo' | 'redo' | 'cut' | 'copy' | 'selectAll' | null> = {
  'edit:undo': 'undo',
  'edit:redo': 'redo',
  'edit:cut': 'cut',
  'edit:copy': 'copy',
  // 스크립트 붙이기는 웹뷰가 막을 수 있다. 문서에 붙여 넣지 않는 것이 목적이다.
  'edit:paste': null,
  'edit:select-all': 'selectAll',
};

/** 문서 밖 입력창이 처리했으면 `true` — 호출자는 문서 명령을 보내지 않는다. */
export function routeNativeEditToTextInput(id: string, doc: Document = document): boolean {
  if (!(id in NATIVE_TEXT_EDIT)) return false;
  const active = doc.activeElement as HTMLElement | null;
  if (!active || !isTextEditingTarget(active) || !active.closest(TEXT_INPUT_HOSTS)) return false;
  const command = NATIVE_TEXT_EDIT[id];
  if (command === 'selectAll' && typeof (active as HTMLInputElement).select === 'function') {
    (active as HTMLInputElement).select();
  } else if (command && command !== 'selectAll') {
    doc.execCommand(command);
  }
  return true;
}
