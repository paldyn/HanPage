/**
 * [Task #29] 웹 헤더의 "데스크톱 앱 다운로드" 버튼.
 *
 * OS를 감지해 GitHub Releases 최신 릴리스에서 해당 OS용 설치 파일을 내려받는다.
 * - macOS → `*.dmg`(aarch64), Windows → `*-setup.exe`(x64), 기타 → 릴리스 페이지.
 * - 버전이 파일명에 박혀 고정 URL이 불가하므로 GitHub API로 asset URL을 취득(CORS 허용).
 * - Tauri 데스크톱 앱 내부에서는 버튼을 표시하지 않는다(이미 앱).
 */

type TargetOS = 'mac' | 'win' | 'other';

const REPO = 'paldyn/HanPage';
const RELEASES_PAGE = `https://github.com/${REPO}/releases/latest`;
const API_LATEST = `https://api.github.com/repos/${REPO}/releases/latest`;

/** Tauri 웹뷰(데스크톱 앱) 안에서 실행 중인가. */
function isDesktopApp(): boolean {
  return (
    typeof window !== 'undefined' &&
    ('__TAURI_INTERNALS__' in window || '__TAURI__' in window)
  );
}

function detectOS(): TargetOS {
  const nav = navigator as Navigator & { userAgentData?: { platform?: string } };
  const platform = (nav.userAgentData?.platform || navigator.platform || '').toLowerCase();
  const ua = (navigator.userAgent || '').toLowerCase();
  if (platform.includes('mac') || ua.includes('mac')) return 'mac';
  if (platform.includes('win') || ua.includes('win')) return 'win';
  return 'other';
}

interface ReleaseAsset {
  name: string;
  browser_download_url: string;
}

/** OS에 맞는 최신 설치 파일 URL. 실패/없음이면 null(폴백은 호출부에서). */
async function findAssetUrl(os: TargetOS): Promise<string | null> {
  if (os === 'other') return null;
  try {
    const res = await fetch(API_LATEST, {
      headers: { Accept: 'application/vnd.github+json' },
    });
    if (!res.ok) return null;
    const data = (await res.json()) as { assets?: ReleaseAsset[] };
    const assets = data.assets ?? [];
    const match = assets.find((a) => {
      const n = a.name.toLowerCase();
      return os === 'mac'
        ? n.endsWith('.dmg')
        : n.endsWith('-setup.exe') || (n.endsWith('.exe') && n.includes('setup'));
    });
    return match?.browser_download_url ?? null;
  } catch {
    return null;
  }
}

/**
 * 헤더에 다운로드 버튼을 설치한다. 데스크톱 앱에서는 아무 것도 하지 않는다.
 * @param container 버튼을 붙일 헤더 요소(`#menu-bar`).
 */
export function installAppDownloadButton(container: HTMLElement): void {
  if (isDesktopApp()) return;

  const os = detectOS();
  const btn = document.createElement('button');
  btn.id = 'app-download-btn';
  btn.type = 'button';
  btn.textContent = '데스크톱 앱 다운로드';
  btn.title = '운영체제에 맞는 HanPage 데스크톱 앱을 내려받습니다';

  btn.addEventListener('click', async () => {
    if (btn.dataset.busy === '1') return;
    btn.dataset.busy = '1';
    const label = btn.textContent;
    btn.textContent = '확인 중…';
    try {
      const url = (await findAssetUrl(os)) ?? RELEASES_PAGE;
      window.open(url, '_blank', 'noopener,noreferrer');
    } finally {
      btn.textContent = label;
      btn.dataset.busy = '';
    }
  });

  container.appendChild(btn);
}
