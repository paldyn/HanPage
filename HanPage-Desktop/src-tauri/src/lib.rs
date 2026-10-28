// HanPage 데스크톱 앱 엔트리.
//
// 1단계(Task #1)는 기존 rhwp-studio 빌드 산출물 + WASM을 OS 웹뷰에 그대로 로드한다.
// 엔진(WASM)은 수정하지 않는다.
//
// Stage 2: 네이티브 열기/저장 dialog 를 Rust 측 app command 로 제공한다. dialog 와
// 파일 IO 를 전부 Rust 에서 처리하므로 프런트(JS)에는 dialog/fs 플러그인 권한을 주지
// 않는다(앱 자체 command 는 ACL 권한 대상이 아니다).
//
// Stage 3: 파일 연결(.hwp/.hwpx 더블클릭)·네이티브 메뉴바·최근 문서·창 상태 복원.
//   - 파일 연결: macOS 는 `RunEvent::Opened{urls}`, Win/Linux 는 실행 argv 로 경로를 받는다
//     (첫 실행은 `setup()` 에서 자기 argv, 이미 실행 중이면 single-instance 가 넘긴 argv).
//     바이트를 읽어 "펜딩 큐"에 적재 후 웹뷰에 신호(emit)를 보낸다. 읽지 못한 문서도
//     실패 항목으로 큐에 넣어 웹뷰가 사용자에게 알린다.
//   - 메뉴바: Rust 에서 메뉴를 만들고, 사용자 정의 항목 클릭 시 커맨드 id 를 웹뷰로
//     emit 한다. 프런트 브리지가 이를 받아 기존 rhwp-studio 커맨드를 dispatch 한다
//     (열기/저장/저장하기는 Stage 2 흐름 재사용). 단축키는 스튜디오의 문맥 인지
//     키보드 핸들러가 그대로 소유하므로 메뉴 항목에는 가속기를 달지 않는다.
//   - 최근 문서: tauri-plugin-store 에 경로 목록을 영속화하고, 시작 시 메뉴에 노출한다.
//   - 창 상태: tauri-plugin-window-state 로 크기/위치를 저장·복원한다.
// [#156] 문서마다 창 하나: 문서는 지금 창이 손대지 않은 빈 창일 때만 그 창에서, 아니면 새 창에서
//   연다. 창 목록·창별 대기 문서·닫기 판단은 `windows.rs` 가 소유하고, 메뉴 명령은 가장 최근
//   창에만 보낸다. 창 닫기와 Cmd+Q 는 저장하지 않은 문서를 웹뷰에 묻는다.
//   store/window-state/single-instance 모두 Rust 측에서만 사용하므로 JS ACL 권한
//   추가가 불필요하다(core:default 유지). 프런트 브리지는 web-inert
//   (`rhwp-studio/src/core/desktop-bridge.ts`)로, 브라우저에서는 완전한 no-op 이다.

use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, MutexGuard};
use std::time::Instant;

#[cfg(desktop)]
use std::sync::Arc;

#[cfg(desktop)]
mod update_state;
mod windows;
#[cfg(desktop)]
use update_state::{should_emit_download_progress, UpdateSlot, UpdateState};

use serde::Serialize;
// 네이티브 메뉴는 macOS 시스템 메뉴바 전용(이슈 #7). 비-macOS 는 메뉴 미부착.
#[cfg(target_os = "macos")]
use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_dialog::{MessageDialogButtons, MessageDialogKind};
use tauri_plugin_store::StoreExt;
use windows::{CloseDecision, QuitStep, Registry, Route, Target};

/// 최근 문서 store 파일명·키·최대 보관 수.
const RECENT_STORE: &str = "recent.json";
const RECENT_KEY: &str = "documents";
const RECENT_MAX: usize = 10;

/// 업데이트 뒤 재실행 표식 store 파일명·키·유효 시간.
#[cfg(desktop)]
const RELAUNCH_STORE: &str = "relaunch.json";
#[cfg(desktop)]
const RELAUNCH_KEY: &str = "updateRelaunch";
/// 업데이트 직후의 자동 재실행만 걸러내고, 나중의 실제 파일 열기는 막지 않도록 짧게 둔다.
#[cfg(desktop)]
const RELAUNCH_MARKER_TTL_SECS: u64 = 600;

/// 웹뷰로 보내는 이벤트 이름.
const EVT_MENU: &str = "hanpage://menu"; // 메뉴 액션 → 스튜디오 커맨드 id
const EVT_DOCS_READY: &str = "hanpage://documents-ready"; // 펜딩 문서 도착 신호
const EVT_UPDATE_READY: &str = "hanpage://update-ready"; // 새 버전 내려받기 완료(적용 대기)
const EVT_CLOSE_REQUESTED: &str = "hanpage://close-requested"; // 저장 안 한 창 닫기 → 웹뷰가 묻는다

/// macOS 앱 메뉴의 종료(Cmd+Q). 기본 종료 항목은 창에 묻지 않고 바로 끝내므로 직접 처리한다.
#[cfg(target_os = "macos")]
const MENU_QUIT: &str = "app:quit";
/// 새 창을 기준 창에서 비스듬히 띄우는 간격(논리 px).
const CASCADE_OFFSET: f64 = 28.0;
#[cfg(desktop)]
const EVT_UPDATE_STATUS: &str = "hanpage://update-status"; // 확인·다운로드·검증·적용 진행

/// 열기 dialog/파일 연결로 읽은 문서. `data` 는 파일 바이트(JSON 배열 직렬화).
#[derive(Serialize)]
struct OpenedFile {
    name: String,
    path: String,
    data: Vec<u8>,
    /// 펜딩 큐 전용: 파일을 읽지 못한 사유. `Some` 이면 `data` 는 비어 있고, 웹뷰는
    /// 문서를 여는 대신 이 사유를 사용자에게 알린다.
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// 저장 dialog 결과. 프런트에서 `status` 로 분기한다(saved/cancelled).
#[derive(Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
enum SaveOutcome {
    Saved { path: String, name: String },
    Cancelled,
}

/// [#156] 창 목록과 창별 대기 문서(웹뷰가 가져갈 때까지 보관).
///
/// [#50] Tauri managed state 가 아니라 **프로세스 전역**으로 둔다. macOS 는 파일 연결
/// 더블클릭을 `RunEvent::Opened` 로 전달하는데, 콜드 스타트에서는 이 이벤트가
/// `setup()` 의 `app.manage(..)` 보다 먼저 도착할 수 있다(실측: 웹뷰 생성 816ms 전).
/// 첫 창(`main`)은 처음부터 등록돼 있어 그때 온 문서도 그 창 몫으로 보관한다.
static WINDOWS: LazyLock<Mutex<Registry<OpenedFile>>> =
    LazyLock::new(|| Mutex::new(Registry::new()));

fn registry() -> MutexGuard<'static, Registry<OpenedFile>> {
    WINDOWS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// [#59] 백그라운드로 미리 내려받아 둔 업데이트(설치 대기).
///
/// Claude 데스크톱 앱 방식: 새 버전을 발견하면 **묻지 않고 조용히 받아둔 뒤**, 완료 시점에
/// 준비 완료 안내로 한 번만 알린다. 사용자가 '업데이트'를 누르면 이미 받아둔 바이트를
/// 적용한다. '나중에'를 눌러도 바이트를 버리지 않아 다음 클릭에도 재다운로드하지 않는다.
/// 상태와 파일은 한 락으로 묶어 시작 시 확인·수동 확인·적용의 중복 실행을 막는다.
#[cfg(desktop)]
static UPDATE_SLOT: LazyLock<Mutex<UpdateSlot<Arc<ReadyUpdate>>>> =
    LazyLock::new(|| Mutex::new(UpdateSlot::new()));

#[cfg(desktop)]
struct ReadyUpdate {
    update: tauri_plugin_updater::Update,
    bytes: Vec<u8>,
}

/// 최근 문서 메뉴 항목. 네이티브 메뉴(macOS) 표시 전용.
#[cfg(target_os = "macos")]
struct RecentEntry {
    path: String,
    name: String,
}

// ─── 파일 IO 헬퍼 ────────────────────────────────────────────────────────────
/// 경로의 파일을 읽어 `OpenedFile` 로 만든다.
fn read_document(path: &Path) -> Result<OpenedFile, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document")
        .to_string();
    Ok(OpenedFile {
        name,
        path: path.to_string_lossy().into_owned(),
        data,
        error: None,
    })
}

/// 경로의 파일 이름(표시용). 없으면 경로 전체를 쓴다.
fn display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| path.display().to_string())
}

/// [#121] 시작 argv 를 큐에 넣기 전에 도착한 single-instance 경로를 보관하는 관문.
///
/// Windows 는 첫 실행이 WebView2 를 만드는 동안(`setup()` 전)에도 메시지를 처리하므로,
/// 그 사이 두 번째 실행이 넘긴 파일이 첫 실행의 시작 파일보다 먼저 큐에 들어갈 수 있다.
/// 그러면 나중에 연 파일 위에 시작 파일이 열린다. 시작 파일을 넣을 때까지 모아 두었다가
/// 그 뒤에 넣어 실행 순서를 지킨다. 확인과 보관을 한 락 안에서 해 `setup()` 과 경합하지 않는다.
#[cfg(desktop)]
struct StartupGate {
    pending: Option<Vec<PathBuf>>,
}

#[cfg(desktop)]
impl StartupGate {
    const fn new() -> Self {
        Self {
            pending: Some(Vec::new()),
        }
    }

    /// 시작 전이면 경로를 보관하고 빈 목록을, 시작 뒤면 그대로 돌려준다.
    fn admit(&mut self, paths: Vec<PathBuf>) -> Vec<PathBuf> {
        match &mut self.pending {
            Some(early) => {
                early.extend(paths);
                Vec::new()
            }
            None => paths,
        }
    }

    /// 시작 문서를 큐에 넣은 뒤 호출한다. 보관분을 돌려주고 이후 경로는 바로 통과시킨다.
    fn open(&mut self) -> Vec<PathBuf> {
        self.pending.take().unwrap_or_default()
    }
}

#[cfg(desktop)]
static STARTUP_GATE: Mutex<StartupGate> = Mutex::new(StartupGate::new());

/// [#121] 업데이트 적용 직전에 남기는 재실행 표식.
///
/// 업데이터는 새 버전을 띄울 때 지금 프로세스의 실행 인자를 그대로 넘긴다(Windows NSIS
/// `/ARGS`, macOS `restart()`). 표식이 없으면 처음 파일 연결로 연 문서를 업데이트할 때마다
/// 다시 열고, NSIS 가 따옴표를 벗겨 공백 있는 경로가 쪼개지면 엉뚱한 실패 안내가 뜬다.
#[cfg(desktop)]
#[derive(Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
struct RelaunchMarker {
    /// 실행 인자(argv[1..])를 공백으로 이은 값. NSIS 가 따옴표를 벗겨도 같게 비교된다.
    args: String,
    /// 설치할 버전. 설치가 실패해 이전 버전이 다시 뜨면 일치하지 않는다.
    version: String,
    /// 표식을 남긴 시각(UNIX 초).
    at: u64,
}

/// argv[1..] 를 공백으로 이어 재실행 비교 키를 만든다.
#[cfg(desktop)]
fn launch_args_key<I, S>(args: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    args.into_iter()
        .skip(1)
        .map(|arg| arg.as_ref().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

/// 이번 실행이 업데이트 직후의 자동 재실행이라 실행 인자의 문서를 다시 열지 말아야 하는지.
#[cfg(desktop)]
fn is_update_relaunch(
    marker: Option<&RelaunchMarker>,
    args_key: &str,
    current_version: &str,
    now: u64,
) -> bool {
    marker.is_some_and(|m| {
        !m.args.is_empty()
            && m.args == args_key
            && m.version == current_version
            && now.saturating_sub(m.at) <= RELAUNCH_MARKER_TTL_SECS
    })
}

#[cfg(desktop)]
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// 업데이트 적용 직전: 실행 인자에 문서가 있으면 재실행 표식을 남긴다. Windows 는 설치
/// 프로그램을 띄운 뒤 곧바로 프로세스를 끝내므로 즉시 저장한다.
#[cfg(desktop)]
fn mark_update_relaunch(app: &tauri::AppHandle, version: &str) {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let cwd = std::env::current_dir().unwrap_or_default();
    if document_paths_from_args(args.iter().cloned(), &cwd).is_empty() {
        return;
    }
    let marker = RelaunchMarker {
        args: launch_args_key(&args),
        version: version.to_string(),
        at: unix_now(),
    };
    if let (Ok(store), Ok(value)) = (app.store(RELAUNCH_STORE), serde_json::to_value(&marker)) {
        store.set(RELAUNCH_KEY, value);
        let _ = store.save();
    }
}

/// 재실행 표식을 지운다(설치 실패 시, 그리고 시작 시 한 번 읽은 뒤).
#[cfg(desktop)]
fn clear_update_relaunch(app: &tauri::AppHandle) {
    if let Ok(store) = app.store(RELAUNCH_STORE) {
        if store.delete(RELAUNCH_KEY) {
            let _ = store.save();
        }
    }
}

/// 시작 시 재실행 표식을 한 번 읽고 지운다.
#[cfg(desktop)]
fn take_update_relaunch(app: &tauri::AppHandle) -> Option<RelaunchMarker> {
    let marker = app
        .store(RELAUNCH_STORE)
        .ok()
        .and_then(|store| store.get(RELAUNCH_KEY))
        .and_then(|value| serde_json::from_value(value).ok());
    clear_update_relaunch(app);
    marker
}

/// 실행 인자로 받아 여는 문서 확장자인지(대소문자 무시). 파일 연결은 `.hwp`/`.hwpx` 만 등록하지만,
/// `연결 프로그램`으로 넘긴 `.hml`·`.pdf` 도 macOS(`RunEvent::Opened`)처럼 열고 웹뷰가 형식을 판별한다.
#[cfg(desktop)]
fn is_document_path(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        ["hwp", "hwpx", "hml", "pdf"]
            .iter()
            .any(|x| e.eq_ignore_ascii_case(x))
    })
}

/// 실행 인자에서 열 문서 경로를 고른다. 첫 항목은 실행 파일이라 건너뛰고, 상대 경로는
/// 그 인자를 받은 프로세스의 작업 디렉터리(`cwd`) 기준으로 바꾼다 — single-instance 로
/// 넘어온 argv 는 이미 떠 있는 프로세스와 작업 디렉터리가 다를 수 있다.
#[cfg(desktop)]
fn document_paths_from_args<I, S>(args: I, cwd: &Path) -> Vec<PathBuf>
where
    I: IntoIterator<Item = S>,
    S: Into<PathBuf>,
{
    args.into_iter()
        .skip(1)
        .map(Into::into)
        .filter(|path: &PathBuf| is_document_path(path))
        .map(|path| {
            if path.is_relative() {
                cwd.join(path)
            } else {
                path
            }
        })
        .collect()
}

// ─── 최근 문서(store) ────────────────────────────────────────────────────────
/// 최근 문서 목록에 경로를 등록한다(중복 제거 후 맨 앞에 추가, 최대 RECENT_MAX).
fn record_recent(app: &tauri::AppHandle, path: &str, name: &str) {
    let Ok(store) = app.store(RECENT_STORE) else {
        return;
    };
    let mut list: Vec<serde_json::Value> = store
        .get(RECENT_KEY)
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    list.retain(|e| e.get("path").and_then(|p| p.as_str()) != Some(path));
    list.insert(0, serde_json::json!({ "path": path, "name": name }));
    list.truncate(RECENT_MAX);
    store.set(RECENT_KEY, serde_json::Value::Array(list));
    let _ = store.save();
}

/// store 에서 최근 문서 목록을 읽는다(메뉴 구성용). 네이티브 메뉴(macOS) 전용.
#[cfg(target_os = "macos")]
fn load_recent(app: &tauri::AppHandle) -> Vec<RecentEntry> {
    let Ok(store) = app.store(RECENT_STORE) else {
        return Vec::new();
    };
    store
        .get(RECENT_KEY)
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|e| {
            let path = e.get("path")?.as_str()?.to_string();
            let name = e.get("name")?.as_str()?.to_string();
            Some(RecentEntry { path, name })
        })
        .collect()
}

// ─── 문서 전달(파일 연결/최근 문서 → 창) ───────────────────────────────────────
/// 같은 파일 판단에 쓰는 경로. 심볼릭 링크와 상대 경로 차이를 없앤다.
fn document_key(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// 창을 앞으로 가져온다. 최소화·숨김 창에는 포커스만으로는 듣지 않는다(macOS·Windows 공통).
fn focus_window(app: &tauri::AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 문서를 받을 창을 정해 보낸다. 새 창은 문서를 먼저 넣은 뒤 만든다 — 웹뷰가 시작하며 직접 가져간다.
fn deliver(app: &tauri::AppHandle, file: OpenedFile, target: Target<'_>) -> Route {
    let key = (!file.path.is_empty()).then(|| document_key(Path::new(&file.path)));
    let name = file.name.clone();
    let route = {
        let mut reg = registry();
        let route = reg.route(key.as_deref(), target);
        if let Route::Into(label) | Route::New(label) = &route {
            let _ = reg.enqueue(label, file);
        }
        route
    };
    // 어느 창으로 보냈는지 남긴다(현장 진단: 창이 엉뚱하게 늘거나 문서가 사라지는 증상).
    eprintln!("[HanPage] 문서 전달 {name} → {route:?}");
    match &route {
        Route::Into(label) => {
            let _ = app.emit_to(label.as_str(), EVT_DOCS_READY, ());
            focus_window(app, label);
        }
        Route::New(label) => spawn_document_window(app, label.clone()),
        Route::Focus(label) => focus_window(app, label),
    }
    route
}

/// 읽지 못했거나 열 수 없는 문서를 가장 최근 창에 알린다. 알림 하나 때문에 새 창을 만들지 않는다.
fn notify_failure(app: &tauri::AppHandle, name: String, path: String, message: String) {
    let label = {
        let mut reg = registry();
        let label = reg.front().map(str::to_string);
        if let Some(label) = &label {
            let _ = reg.enqueue(
                label,
                OpenedFile {
                    name,
                    path,
                    data: Vec::new(),
                    error: Some(message),
                },
            );
        }
        label
    };
    if let Some(label) = label {
        let _ = app.emit_to(label.as_str(), EVT_DOCS_READY, ());
    }
}

/// 경로를 읽어 창으로 보낸다(파일 연결/최근 문서 클릭/single-instance 공통).
/// [#121] 읽지 못하면 사유를 알린다 — 조용히 버리면 사용자는 빈 창만 보고 다시 열어야 한다.
fn open_path(app: &tauri::AppHandle, path: PathBuf) {
    match read_document(&path) {
        Ok(file) => {
            record_recent(app, &file.path, &file.name);
            deliver(app, file, Target::Front);
        }
        Err(e) => {
            eprintln!("[HanPage] 파일 열기 실패 {}: {}", path.display(), e);
            notify_failure(
                app,
                display_name(&path),
                path.to_string_lossy().into_owned(),
                e,
            );
        }
    }
}

/// macOS/모바일: 파일 연결 더블클릭 시 전달되는 URL 들을 처리한다.
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
fn handle_opened(app: &tauri::AppHandle, urls: &[tauri::Url]) {
    for url in urls {
        if let Ok(path) = url.to_file_path() {
            // Dock 드롭·연결 프로그램으로 온 열 수 없는 파일 때문에 빈 새 창을 만들지 않는다.
            #[cfg(desktop)]
            if !is_document_path(&path) {
                notify_failure(
                    app,
                    display_name(&path),
                    path.to_string_lossy().into_owned(),
                    "지원하지 않는 파일 형식입니다.".into(),
                );
                continue;
            }
            open_path(app, path);
        }
    }
}

// ─── 창 만들기·닫기·종료 ──────────────────────────────────────────────────────
/// 등록한 레이블로 문서 창을 만든다. Windows 는 동기 command·이벤트 처리기 안에서 창을 만들면
/// 교착되므로(Tauri 문서) 별도 스레드에서 만든다. 실패하면 그 창 몫의 문서를 최근 창에 알린다.
fn spawn_document_window(app: &tauri::AppHandle, label: String) {
    let app = app.clone();
    std::thread::spawn(move || {
        wait_for_anchor_window(&app, &label);
        let built = build_document_window(&app, &label);
        // 다른 스레드에서 만들면 Tauri 는 만들기 요청만 보내고 돌아온다. 창에 한 번 물어 실제로
        // 만들어졌는지 확인한다(요청은 순서대로 처리되므로 만들기 뒤에 답한다).
        let ok = built
            .as_ref()
            .is_ok_and(|window| window.scale_factor().is_ok());
        let reason = match &built {
            Ok(_) if ok => None,
            Ok(_) => Some("창이 만들어지지 않았습니다".to_string()),
            Err(e) => Some(e.to_string()),
        };
        match &reason {
            None => eprintln!("[HanPage] 새 창 {label}"),
            Some(reason) => eprintln!("[HanPage] 새 창을 만들지 못했습니다 {label}: {reason}"),
        }
        let orphans = registry().created(&label, ok);
        let Some(reason) = reason else {
            return;
        };
        if let Some(window) = app.get_webview_window(&label) {
            let _ = window.destroy();
        }
        for doc in orphans {
            notify_failure(
                &app,
                doc.name,
                doc.path,
                format!("새 창을 열지 못했습니다: {reason}"),
            );
        }
        // 마지막 창이 닫히는 사이 만들기에 실패했으면 보이지 않는 프로세스로 남지 않는다.
        if app.webview_windows().is_empty() && registry().creating() == 0 {
            app.exit(0);
        }
    });
}

/// 콜드 스타트에서는 문서 창 요청이 첫 창보다 먼저 올 수 있다(#50). 기준 창이 생기고 창 위치 복원이
/// 끝난 뒤 위치를 정해야 두 창이 같은 자리에 겹치지 않는다. 기준 창이 끝내 없으면 그냥 만든다.
fn wait_for_anchor_window(app: &tauri::AppHandle, new_label: &str) {
    let Some(anchor) = registry().front_except(new_label).map(str::to_string) else {
        return;
    };
    for _ in 0..60 {
        if app.get_webview_window(&anchor).is_some() {
            std::thread::sleep(std::time::Duration::from_millis(150));
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// 첫 창 설정을 복제해 레이블과 위치만 바꾼다(크기·최소 크기·드롭 설정이 같다).
fn build_document_window(
    app: &tauri::AppHandle,
    label: &str,
) -> tauri::Result<tauri::WebviewWindow> {
    let configs = &app.config().app.windows;
    let mut config = configs
        .iter()
        .find(|w| w.label == windows::MAIN_LABEL)
        .or_else(|| configs.first())
        .cloned()
        .unwrap_or_default();
    config.label = label.to_string();
    config.center = false;
    if let Some((x, y)) = cascade_position(app, label, config.width, config.height) {
        config.x = Some(x);
        config.y = Some(y);
    }
    tauri::WebviewWindowBuilder::from_config(app, &config)?.build()
}

/// 가장 최근 창에서 오른쪽 아래로 비스듬히 띄운다. 이미 창이 있는 자리는 한 칸씩 더 비키고,
/// 화면 작업 영역을 넘으면 왼쪽 위로 돌아간다.
fn cascade_position(
    app: &tauri::AppHandle,
    new_label: &str,
    width: f64,
    height: f64,
) -> Option<(f64, f64)> {
    // 최소화·숨김 창은 기준으로 쓰지 않는다(Windows 는 최소화 창 위치가 -32000 이다).
    let labels = registry().labels();
    let window = labels
        .iter()
        .filter(|label| label.as_str() != new_label)
        .filter_map(|label| app.get_webview_window(label))
        .find(|w| !w.is_minimized().unwrap_or(false) && w.is_visible().unwrap_or(true))?;
    let scale = window.scale_factor().ok()?;
    let position = window.outer_position().ok()?.to_logical::<f64>(scale);
    let occupied: Vec<(f64, f64)> = app
        .webview_windows()
        .values()
        .filter(|w| w.label() != new_label)
        .filter_map(|w| {
            let s = w.scale_factor().ok()?;
            let p = w.outer_position().ok()?.to_logical::<f64>(s);
            Some((p.x, p.y))
        })
        .collect();
    let area = window.current_monitor().ok().flatten().map(|monitor| {
        let a = monitor.work_area();
        let s = monitor.scale_factor();
        let left = f64::from(a.position.x) / s;
        let top = f64::from(a.position.y) / s;
        (
            left,
            top,
            left + f64::from(a.size.width) / s,
            top + f64::from(a.size.height) / s,
        )
    });
    Some(next_cascade_position(
        (position.x, position.y),
        &occupied,
        (width, height),
        area,
    ))
}

/// 기준 위치에서 비스듬히 비켜 빈 자리를 찾는다(창 위치 계산만 따로 시험한다).
fn next_cascade_position(
    anchor: (f64, f64),
    occupied: &[(f64, f64)],
    size: (f64, f64),
    area: Option<(f64, f64, f64, f64)>,
) -> (f64, f64) {
    let taken = |x: f64, y: f64| {
        occupied
            .iter()
            .any(|(ox, oy)| (ox - x).abs() < 4.0 && (oy - y).abs() < 4.0)
    };
    let (mut x, mut y) = (anchor.0 + CASCADE_OFFSET, anchor.1 + CASCADE_OFFSET);
    for _ in 0..32 {
        if let Some((left, top, right, bottom)) = area {
            if x < left || y < top || x + size.0 > right || y + size.1 > bottom {
                x = left + CASCADE_OFFSET;
                y = top + CASCADE_OFFSET;
            }
        }
        if !taken(x, y) {
            break;
        }
        x += CASCADE_OFFSET;
        y += CASCADE_OFFSET;
    }
    (x, y)
}

/// 저장하지 않은 문서가 있는 창에 닫아도 되는지 묻게 한다(최소화된 창도 앞으로 가져온다).
fn ask_close(app: &tauri::AppHandle, label: &str) {
    focus_window(app, label);
    let _ = app.emit_to(label, EVT_CLOSE_REQUESTED, ());
}

/// 웹뷰가 닫기 확인에 답하지 않는다. 묻지 않고 닫지도, 영영 못 닫게 두지도 않도록 네이티브 확인창으로 묻는다.
fn confirm_close_natively(app: &tauri::AppHandle, label: String) {
    let app = app.clone();
    std::thread::spawn(move || {
        let Some(window) = app.get_webview_window(&label) else {
            return;
        };
        let confirmed = app
            .dialog()
            .message("이 창이 응답하지 않습니다.\n저장하지 않은 변경을 버리고 창을 닫을까요?")
            .title("HanPage")
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(
                "닫기".to_string(),
                "취소".to_string(),
            ))
            .parent(&window)
            .blocking_show();
        registry().close_answered(&label, confirmed);
        if confirmed {
            let _ = window.destroy();
        }
    });
}

/// 종료 단계를 실행한다: 저장할 것이 없는 창은 닫고, 저장할 것이 있는 창 하나에 묻는다.
fn apply_quit_step(app: &tauri::AppHandle, step: QuitStep) {
    for label in &step.close_now {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.destroy();
        }
    }
    if let Some(label) = step.ask {
        ask_close(app, &label);
    }
}

/// Cmd+Q: 저장할 것이 없는 창은 바로 닫고, 저장할 것이 있는 창은 하나씩 묻는다. 모든 창이
/// 닫히면 앱이 끝난다. 사용자가 한 번이라도 취소하면 종료를 멈춘다.
#[cfg(target_os = "macos")]
fn request_quit(app: &tauri::AppHandle) {
    if app.webview_windows().is_empty() {
        app.exit(0);
        return;
    }
    // 아직 만들어지는 창이 있으면 종료를 끝낼 수 없다. 창이 뜬 뒤 다시 종료하면 된다.
    let labels = registry().labels();
    if labels
        .iter()
        .any(|label| app.get_webview_window(label).is_none())
    {
        return;
    }
    let step = registry().begin_quit();
    apply_quit_step(app, step);
}

// ─── 네이티브 메뉴 ────────────────────────────────────────────────────────────
/// 앱 전역 메뉴를 만든다. 사용자 정의 항목 id 는 rhwp-studio 커맨드 id 와 동일하게 두어
/// 클릭 시 그대로 dispatch 한다. 단축키는 스튜디오 키보드 핸들러가 소유하므로 가속기는
/// 달지 않는다(문맥 인지 편집·입력 필드 복사/붙여넣기 보존).
/// 네이티브 메뉴는 macOS 시스템 메뉴바 전용(이슈 #7).
#[cfg(target_os = "macos")]
fn build_app_menu(
    app: &tauri::AppHandle,
    recent: &[RecentEntry],
) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    // 앱 메뉴(macOS): About / Quit (cross-platform predefined 만 사용).
    // [Task #26] 업데이트 수동 확인 (macOS 네이티브 메뉴).
    let check_update = MenuItemBuilder::with_id("app:check-update", "업데이트 확인").build(app)?;
    // [#156] 기본 종료 항목은 창에 묻지 않고 끝낸다. 저장하지 않은 창을 묻도록 직접 처리한다.
    let quit = MenuItemBuilder::with_id(MENU_QUIT, "HanPage 종료")
        .accelerator("CmdOrCtrl+Q")
        .build(app)?;
    let app_menu = SubmenuBuilder::new(app, "HanPage")
        .about(None)
        .item(&check_update)
        .separator()
        .item(&quit)
        .build()?;

    // 파일 메뉴
    let new_doc = MenuItemBuilder::with_id("file:new-doc", "새 문서").build(app)?;
    let open = MenuItemBuilder::with_id("file:open", "열기…").build(app)?;
    let save = MenuItemBuilder::with_id("file:save", "저장").build(app)?;
    let save_as = MenuItemBuilder::with_id("file:save-as", "다른 이름으로 저장…").build(app)?;

    let mut recent_b = SubmenuBuilder::new(app, "최근 문서");
    if recent.is_empty() {
        let none = MenuItemBuilder::with_id("recent:__none__", "(없음)")
            .enabled(false)
            .build(app)?;
        recent_b = recent_b.item(&none);
    } else {
        for e in recent {
            let item =
                MenuItemBuilder::with_id(format!("recent:{}", e.path), &e.name).build(app)?;
            recent_b = recent_b.item(&item);
        }
    }
    let recent_menu = recent_b.build()?;

    let file_menu = SubmenuBuilder::new(app, "파일")
        .item(&new_doc)
        .item(&open)
        .separator()
        .item(&save)
        .item(&save_as)
        .separator()
        .item(&recent_menu)
        .build()?;

    // 편집 메뉴: 스튜디오 커맨드로 위임(캔버스 에디터 전용 편집 로직). 가속기 없음.
    let undo = MenuItemBuilder::with_id("edit:undo", "실행 취소").build(app)?;
    let redo = MenuItemBuilder::with_id("edit:redo", "다시 실행").build(app)?;
    let cut = MenuItemBuilder::with_id("edit:cut", "오려두기").build(app)?;
    let copy = MenuItemBuilder::with_id("edit:copy", "복사").build(app)?;
    let paste = MenuItemBuilder::with_id("edit:paste", "붙이기").build(app)?;
    let select_all = MenuItemBuilder::with_id("edit:select-all", "모두 선택").build(app)?;
    let edit_menu = SubmenuBuilder::new(app, "편집")
        .item(&undo)
        .item(&redo)
        .separator()
        .item(&cut)
        .item(&copy)
        .item(&paste)
        .separator()
        .item(&select_all)
        .build()?;

    // 보기 메뉴: 확대/축소/실제 크기(스튜디오 커맨드) + 전체 화면(predefined).
    let zoom_in = MenuItemBuilder::with_id("view:zoom-in", "확대").build(app)?;
    let zoom_out = MenuItemBuilder::with_id("view:zoom-out", "축소").build(app)?;
    let zoom_reset = MenuItemBuilder::with_id("view:zoom-100", "실제 크기 (100%)").build(app)?;
    let view_menu = SubmenuBuilder::new(app, "보기")
        .item(&zoom_in)
        .item(&zoom_out)
        .item(&zoom_reset)
        .separator()
        .fullscreen()
        .build()?;

    MenuBuilder::new(app)
        .item(&app_menu)
        .item(&file_menu)
        .item(&edit_menu)
        .item(&view_menu)
        .build()
}

// ─── 앱 command ──────────────────────────────────────────────────────────────
/// 열기 대화상자 필터. HML 은 엔진이 원래 여는 형식이고, PDF 는 가져오기를 지원할 때만 넣는다.
fn open_dialog_filter(allow_pdf: bool) -> (&'static str, &'static [&'static str]) {
    if allow_pdf {
        ("문서 (HWP/HWPX/HML/PDF)", &["hwp", "hwpx", "hml", "pdf"])
    } else {
        ("한글 문서 (HWP/HWPX/HML)", &["hwp", "hwpx", "hml"])
    }
}

/// 저장 대화상자 필터. 제안 파일명의 확장자(웹뷰가 고른 저장 형식)를 따른다. PDF 는 넣지 않는다.
fn save_dialog_filter(suggested_name: &str) -> (&'static str, &'static [&'static str]) {
    let ext = Path::new(suggested_name)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match ext.as_deref() {
        Some("hwpx") => ("한글 문서 (HWPX)", &["hwpx"]),
        Some("hml") => ("한글 문서 (HML)", &["hml"]),
        _ => ("한글 문서 (HWP)", &["hwp"]),
    }
}

fn is_pdf_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// 네이티브 열기 dialog → 선택 파일 바이트 반환. 취소 시 `Ok(None)`.
///
/// async command 는 메인 스레드가 아닌 async 런타임 워커에서 실행되므로 여기서
/// `blocking_pick_file()`(내부적으로 dialog 를 메인 스레드에 디스패치)을 호출해도
/// 데드락이 없다. 메인 스레드에서 직접 호출하면 안 된다.
///
/// `allow_pdf` 는 웹뷰의 WASM 이 PDF 가져오기를 지원할 때만 참이다. 키가 없는 이전 호출도 받는다.
///
/// [#156] 고른 문서는 파일 연결과 같은 길로 보낸다: 부른 창이 손대지 않은 빈 창이면 그 창의 대기
/// 문서로, 아니면 새 창(또는 그 파일을 이미 연 창)으로. 웹뷰는 대기 문서를 가져가 연다.
#[tauri::command]
async fn cmd_open_document(
    app: tauri::AppHandle,
    webview_window: tauri::WebviewWindow,
    allow_pdf: Option<bool>,
) -> Result<(), String> {
    let (label, exts) = open_dialog_filter(allow_pdf.unwrap_or(false));
    let picked = app
        .dialog()
        .file()
        .add_filter(label, exts)
        .blocking_pick_file();

    let Some(file_path) = picked else {
        return Ok(()); // 사용자 취소
    };
    let path = file_path.into_path().map_err(|e| e.to_string())?;
    let file = read_document(&path)?;
    record_recent(&app, &file.path, &file.name);
    deliver(&app, file, Target::Window(webview_window.label()));
    Ok(())
}

/// 네이티브 저장 dialog → 선택 경로에 바이트 기록. 취소 시 `SaveOutcome::Cancelled`.
#[tauri::command]
async fn cmd_save_document(
    app: tauri::AppHandle,
    webview_window: tauri::WebviewWindow,
    suggested_name: String,
    data: Vec<u8>,
) -> Result<SaveOutcome, String> {
    // 저장 바이트 형식과 확장자를 맞춘다. macOS 저장 패널은 필터 밖 확장자를 허용하지 않아
    // HWP 필터만 두면 HWPX 바이트가 `.hwp` 로 저장된다.
    let (label, exts) = save_dialog_filter(&suggested_name);
    let picked = app
        .dialog()
        .file()
        .set_file_name(&suggested_name)
        .add_filter(label, exts)
        .blocking_save_file();

    let Some(file_path) = picked else {
        return Ok(SaveOutcome::Cancelled); // 사용자 취소
    };
    let path = file_path.into_path().map_err(|e| e.to_string())?;
    // Windows 에서는 확장자를 직접 입력해 필터를 넘을 수 있다. 원본 PDF 를 편집 문서로 덮어쓰지 않는다.
    if is_pdf_path(&path) {
        return Err("PDF 파일에는 편집 문서를 저장할 수 없습니다.".into());
    }
    std::fs::write(&path, &data).map_err(|e| e.to_string())?;
    registry().set_path(webview_window.label(), document_key(&path));
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(suggested_name.as_str())
        .to_string();
    Ok(SaveOutcome::Saved {
        path: path.to_string_lossy().into_owned(),
        name,
    })
}

/// 부른 창의 대기 문서를 비우고 반환한다(웹뷰가 init 시점·도착 신호 수신 시 호출).
/// `initial` 은 페이지가 (다시) 시작할 때의 첫 호출이다.
#[tauri::command]
fn cmd_take_pending_documents(
    webview_window: tauri::WebviewWindow,
    initial: Option<bool>,
) -> Vec<OpenedFile> {
    let docs = registry().take_pending(webview_window.label(), initial.unwrap_or(false));
    #[cfg(debug_assertions)]
    eprintln!(
        "[HanPage] {} 대기 문서 {}개 (initial={})",
        webview_window.label(),
        docs.len(),
        initial.unwrap_or(false)
    );
    docs
}

/// [#156] 웹뷰가 시작 흐름을 마친 뒤와 문서·변경 상태가 바뀔 때 알린다. `native_path` 는 지금
/// 문서가 네이티브에서 넘겨받거나 저장한 파일이면 그 경로다(같은 파일 재열기 판단).
#[tauri::command]
fn cmd_report_window_state(
    webview_window: tauri::WebviewWindow,
    pristine: bool,
    dirty: bool,
    native_path: Option<String>,
) {
    let key = native_path
        .filter(|p| !p.is_empty())
        .map(|p| document_key(Path::new(&p)));
    registry().report(webview_window.label(), pristine, dirty, key);
}

/// [#156] 손대지 않은 창이 아니라서 받을 수 없는 문서(끌어다 놓기, 늦게 도착한 문서)를 새 창으로 넘긴다.
#[tauri::command]
fn cmd_open_in_new_window(
    app: tauri::AppHandle,
    name: String,
    path: Option<String>,
    data: Vec<u8>,
) {
    let file = OpenedFile {
        name,
        path: path.unwrap_or_default(),
        data,
        error: None,
    };
    deliver(&app, file, Target::NewWindow);
}

/// [#156] 창 제목을 문서 이름으로 맞춘다(창 전환·작업 표시줄에서 문서를 구분한다).
#[tauri::command]
fn cmd_set_window_title(webview_window: tauri::WebviewWindow, title: String) {
    let _ = webview_window.set_title(&title);
    registry().set_title(webview_window.label(), title);
}

/// [#156] 웹뷰가 닫기 확인을 받아 사용자에게 묻기 시작했다.
#[tauri::command]
fn cmd_close_ack(webview_window: tauri::WebviewWindow) {
    registry().close_acknowledged(webview_window.label());
}

/// [#156] 닫기 확인의 답. 닫기로 했으면 창을 없애고, 아니면 진행 중인 종료도 멈춘다.
#[tauri::command]
fn cmd_close_window(webview_window: tauri::WebviewWindow, confirmed: bool) {
    registry().close_answered(webview_window.label(), confirmed);
    if confirmed {
        let _ = webview_window.destroy();
    }
}

/// [Task #26 · #59] 업데이트 확인 → **조용한 백그라운드 다운로드** (desktop 전용).
///
/// 사용자에게 묻지 않고 먼저 받아둔다. 상태 변화는 웹뷰에 전달하며, 완료 시
/// `EVT_UPDATE_READY` 로 준비 완료를 알린다. 적용은 사용자의 '업데이트' 선택 후 시작한다.
/// 확인 작업의 시작은 `start_update_check` 가 원자적으로 결정한다.
#[cfg(desktop)]
async fn check_update(app: tauri::AppHandle) {
    use tauri_plugin_updater::UpdaterExt;

    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            eprintln!("[updater] 초기화 실패: {e}");
            set_update_state(
                &app,
                UpdateState::Error {
                    message: e.to_string(),
                    retryable: false,
                },
            );
            return;
        }
    };

    let update = match updater.check().await {
        Ok(Some(u)) => u,
        Ok(None) => {
            set_update_state(
                &app,
                UpdateState::UpToDate {
                    version: app.package_info().version.to_string(),
                },
            );
            return;
        }
        Err(e) => {
            eprintln!("[updater] 확인 실패: {e}");
            set_update_state(
                &app,
                UpdateState::Error {
                    message: e.to_string(),
                    retryable: false,
                },
            );
            return;
        }
    };

    let version = update.version.clone();
    let current_version = update.current_version.clone();
    let notes = update.body.clone();

    // 진행률 콜백은 청크 단위(누적 아님)라 여기서 누산한다. Content-Length가 없으면
    // total=None을 그대로 전달하여 웹뷰가 임의의 퍼센트를 표시하지 않게 한다.
    let mut downloaded: u64 = 0;
    let mut last_progress_emit = None;
    set_update_state(
        &app,
        UpdateState::Downloading {
            downloaded: 0,
            total: None,
        },
    );
    let bytes = match update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                let next = UpdateState::Downloading { downloaded, total };
                if should_emit_download_progress(&mut last_progress_emit, Instant::now()) {
                    set_update_state(&app, next);
                } else {
                    // 바이트는 모든 청크에서 정확히 저장한다. 이벤트만 제한하여 큰 파일의
                    // 다운로드가 웹뷰 IPC와 진행 화면 갱신을 과도하게 만들지 않게 한다.
                    store_update_state(next);
                }
            },
            // SDK는 다운로드 완료 콜백 이후 서명을 검증한다. 검증 성공 전 Ready로
            // 표시하지 않아, 받은 파일에 문제가 있으면 적용 버튼이 활성화되지 않는다.
            || set_update_state(&app, UpdateState::Verifying),
        )
        .await
    {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[updater] 다운로드 실패: {e}");
            set_update_state(
                &app,
                UpdateState::Error {
                    message: e.to_string(),
                    retryable: false,
                },
            );
            return;
        }
    };

    let ready_state = if let Ok(mut slot) = UPDATE_SLOT.lock() {
        slot.finish_download(Arc::new(ReadyUpdate { update, bytes }), version.clone());
        Some(slot.state.clone())
    } else {
        None
    };
    if let Some(state) = ready_state {
        let _ = app.emit(EVT_UPDATE_STATUS, state);
    } else {
        return;
    }

    // 웹뷰가 아직 없으면 emit 이 실패해도 무방하다 — 웹뷰는 초기화 시
    // `cmd_update_status` 로 현재 상태를 직접 조회한다(펜딩 문서와 동일한 유실 방지).
    let _ = app.emit(
        EVT_UPDATE_READY,
        UpdateReadyPayload {
            version,
            current_version,
            notes,
        },
    );
}

/// 업데이트 준비 완료 알림 payload.
#[cfg(desktop)]
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateReadyPayload {
    version: String,
    current_version: String,
    notes: Option<String>,
}

#[cfg(desktop)]
fn store_update_state(next: UpdateState) -> bool {
    if let Ok(mut slot) = UPDATE_SLOT.lock() {
        slot.state = next;
        true
    } else {
        false
    }
}

#[cfg(desktop)]
fn set_update_state(app: &tauri::AppHandle, next: UpdateState) {
    if store_update_state(next.clone()) {
        // 락을 놓은 뒤 emit한다. 웹뷰의 상태 재조회가 같은 락을 기다리지 않게 한다.
        let _ = app.emit(EVT_UPDATE_STATUS, next);
    }
}

#[cfg(desktop)]
fn start_update_check(app: tauri::AppHandle) {
    let started = UPDATE_SLOT
        .lock()
        .map(|mut slot| slot.begin_check())
        .unwrap_or(false);
    if !started {
        return;
    }
    let _ = app.emit(EVT_UPDATE_STATUS, UpdateState::Checking);
    tauri::async_runtime::spawn(check_update(app));
}

/// [#156] 다른 창에 저장하지 않은 문서가 있으면 그 창을 보여 주고 안내 문구를 돌려준다.
#[cfg(desktop)]
fn update_blocker(app: &tauri::AppHandle, label: &str) -> Option<String> {
    let (blocking, title) = {
        let reg = registry();
        let blocking = reg.dirty_windows_except(label).first()?.clone();
        let title = reg
            .entry(&blocking)
            .map(|e| e.title.clone())
            .unwrap_or_default();
        (blocking, title)
    };
    focus_window(app, &blocking);
    let which = if title.is_empty() {
        "다른 창".to_string()
    } else {
        format!("다른 창({title})")
    };
    Some(format!(
        "{which}에 저장하지 않은 문서가 있습니다. 저장하거나 창을 닫은 뒤 다시 업데이트해 주세요."
    ))
}

/// [#156] 업데이트를 누른 창이 저장 확인·AI 도우미 정리를 하기 전에, 다른 창 때문에 적용할 수 없는지 확인한다.
#[cfg(desktop)]
#[tauri::command]
fn cmd_update_preflight(
    app: tauri::AppHandle,
    webview_window: tauri::WebviewWindow,
) -> Option<String> {
    update_blocker(&app, webview_window.label())
}

/// [#156] 업데이트 준비 알림은 실행마다 앱 전체에서 버전당 한 번 — 창마다 다시 띄우지 않는다.
#[cfg(desktop)]
static NUDGE_CLAIMED: Mutex<Option<String>> = Mutex::new(None);

/// 이 버전의 준비 알림을 처음 띄우는 창이면 `true`.
#[cfg(desktop)]
#[tauri::command]
fn cmd_claim_update_nudge(version: String) -> bool {
    let mut claimed = NUDGE_CLAIMED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if claimed.as_deref() == Some(version.as_str()) {
        return false;
    }
    *claimed = Some(version);
    true
}

/// 현재 업데이트 상태(웹뷰 초기화·수동 확인 시 조회).
#[cfg(desktop)]
#[tauri::command]
fn cmd_update_status() -> UpdateState {
    UPDATE_SLOT
        .lock()
        .map(|slot| slot.state.clone())
        .unwrap_or(UpdateState::Idle)
}

/// 수동 확인(메뉴) — 이미 받아둔 게 없으면 확인/다운로드를 다시 시작한다.
#[cfg(desktop)]
#[tauri::command]
fn cmd_update_check(app: tauri::AppHandle) {
    start_update_check(app);
}

/// 받아둔 바이트를 작업 스레드에서 적용한다. 압축 해제·파일 교체가 웹뷰를 막지 않게 하며
/// macOS 권한 요청도 SDK가 메인 스레드로 전달할 수 있게 둔다. macOS는 적용 후 재시작,
/// Windows는 설치 프로그램 실행 후 종료하므로 성공 응답은 대개 웹뷰에 도달하지 않는다.
///
/// [#156] 재시작은 모든 창을 닫는다. 다른 창에 저장하지 않은 문서가 있으면 적용하지 않고 그 창을 보여 준다.
#[cfg(desktop)]
#[tauri::command]
async fn cmd_update_apply(
    app: tauri::AppHandle,
    webview_window: tauri::WebviewWindow,
) -> Result<(), String> {
    if let Some(message) = update_blocker(&app, webview_window.label()) {
        return Err(message);
    }
    let (ready, applying_state) = {
        let mut slot = UPDATE_SLOT.lock().map_err(|e| e.to_string())?;
        let version = slot
            .ready
            .as_ref()
            .map(|ready| ready.update.version.clone())
            .unwrap_or_default();
        let ready = slot.begin_apply(version)?;
        (ready, slot.state.clone())
    };
    let _ = app.emit(EVT_UPDATE_STATUS, applying_state);
    mark_update_relaunch(&app, &ready.update.version);

    // 파일은 복사하지 않고 공유한다. worker 자체가 실패하더라도 이 command의 Arc가
    // 검증된 바이트를 보존하므로 사용자 재시도에 재다운로드가 필요하지 않다.
    let installer = Arc::clone(&ready);
    let result = tauri::async_runtime::spawn_blocking(move || {
        installer
            .update
            .install(&installer.bytes)
            .map_err(|e| e.to_string())
    })
    .await;

    let message = match result {
        Ok(Ok(())) => {
            app.restart(); // macOS 경로. Windows 는 install 내부에서 프로세스가 종료된다.
        }
        Ok(Err(message)) => message,
        Err(error) => error.to_string(),
    };
    clear_update_relaunch(&app);
    let failed_state = if let Ok(mut slot) = UPDATE_SLOT.lock() {
        slot.fail_apply(ready, message.clone());
        Some(slot.state.clone())
    } else {
        None
    };
    if let Some(state) = failed_state {
        let _ = app.emit(EVT_UPDATE_STATUS, state);
    }
    Err(message)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    // single-instance 는 가장 먼저 등록해야 한다(Tauri 권장). Win/Linux 에서 2번째
    // 실행 argv 의 .hwp/.hwpx 경로를 캡처해 기존 창으로 넘긴다(macOS 는 Opened 사용).
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            let paths = document_paths_from_args(argv, Path::new(&cwd));
            let ready = match STARTUP_GATE.lock() {
                Ok(mut gate) => gate.admit(paths),
                Err(_) => paths,
            };
            // 문서는 받을 창이 앞으로 나온다. 문서 없이 다시 실행했으면 가장 최근 창을 보여 주고,
            // 창이 하나도 없으면(만들기 실패 등) 빈 창을 띄운다.
            if ready.is_empty() {
                if app.webview_windows().is_empty() {
                    let route = registry().route(None, Target::NewWindow);
                    if let Route::New(label) = route {
                        spawn_document_window(app, label);
                    }
                } else {
                    let front = registry().front().map(str::to_string);
                    if let Some(label) = front {
                        focus_window(app, &label);
                    }
                }
            }
            for path in ready {
                open_path(app, path);
            }
        }));

        // [Task #26] updater: 새 릴리스 확인/다운로드/설치 (desktop 전용).
        // 시작 시 자동 확인·메뉴 핸들러는 Stage 2 에서 추가한다.
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        // [#156] 첫 창의 크기·위치만 기억한다. 문서 창은 매번 비스듬히 새로 띄운다.
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_filter(|label| label == windows::MAIN_LABEL)
                .build(),
        )
        .setup(|app| {
            // [#121] 콜드 스타트 파일 연결(Win/Linux): 문서 경로가 이 프로세스의 argv 로 온다.
            // single-instance 콜백은 두 번째 실행에서만 불리므로 첫 실행 argv 는 여기서
            // 읽는다. macOS 는 LaunchServices 가 argv 대신 `RunEvent::Opened` 로 넘긴다.
            // 업데이트 직후 자동 재실행이면 처음 연 문서를 다시 열지 않는다(`RelaunchMarker`).
            #[cfg(desktop)]
            {
                let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
                let relaunch = take_update_relaunch(app.handle());
                let version = app.package_info().version.to_string();
                if !is_update_relaunch(
                    relaunch.as_ref(),
                    &launch_args_key(&args),
                    &version,
                    unix_now(),
                ) {
                    let cwd = std::env::current_dir().unwrap_or_default();
                    for path in document_paths_from_args(args, &cwd) {
                        open_path(app.handle(), path);
                    }
                }
                let early = STARTUP_GATE
                    .lock()
                    .map(|mut gate| gate.open())
                    .unwrap_or_default();
                for path in early {
                    open_path(app.handle(), path);
                }
            }
            // [Task #26] 시작 시 백그라운드 업데이트 확인(조용히; 새 버전이면 알림).
            #[cfg(desktop)]
            {
                let h = app.handle().clone();
                start_update_check(h);
            }
            // 네이티브 메뉴는 macOS 시스템 메뉴바 전용. Win/Linux 는 창 내부에 그려져
            // 웹 UI 메뉴(#menu-bar)와 중복되므로 부착하지 않는다(이슈 #7).
            #[cfg(target_os = "macos")]
            {
                let recent = load_recent(app.handle());
                let menu = build_app_menu(app.handle(), &recent)?;
                app.set_menu(menu)?;
            }
            Ok(())
        })
        .on_menu_event(|app, event| {
            let id = event.id().0.as_str();
            if let Some(path) = id.strip_prefix("recent:") {
                if path != "__none__" {
                    open_path(app, PathBuf::from(path));
                }
                return;
            }
            #[cfg(target_os = "macos")]
            if id == MENU_QUIT {
                request_quit(app);
                return;
            }
            // 사용자 정의 항목 id = rhwp-studio 커맨드 id(업데이트 확인 포함) → 웹뷰 브리지가 dispatch.
            // [#156] 메뉴는 앱 전체에 하나라 가장 최근 창에만 보낸다. 모든 창에 보내면 저장·붙이기
            // 같은 명령이 창마다 실행된다.
            let front = registry().front().map(str::to_string);
            if let Some(label) = front {
                let _ = app.emit_to(label.as_str(), EVT_MENU, id.to_string());
            }
        })
        .on_window_event(|window, event| {
            let label = window.label().to_string();
            match event {
                tauri::WindowEvent::Focused(true) => registry().focus(&label),
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    let decision = registry().close_requested(&label, Instant::now());
                    match decision {
                        CloseDecision::Allow => {}
                        CloseDecision::Wait => api.prevent_close(),
                        CloseDecision::Ask => {
                            api.prevent_close();
                            ask_close(window.app_handle(), &label);
                        }
                        CloseDecision::ConfirmNative => {
                            api.prevent_close();
                            confirm_close_natively(window.app_handle(), label.clone());
                        }
                    }
                }
                tauri::WindowEvent::Destroyed => {
                    let step = {
                        let mut reg = registry();
                        reg.remove(&label);
                        reg.next_quit_step()
                    };
                    if let Some(step) = step {
                        apply_quit_step(window.app_handle(), step);
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            cmd_open_document,
            cmd_save_document,
            cmd_take_pending_documents,
            cmd_report_window_state,
            cmd_open_in_new_window,
            cmd_set_window_title,
            cmd_close_ack,
            cmd_close_window,
            cmd_update_status,
            cmd_update_check,
            cmd_update_apply,
            cmd_update_preflight,
            cmd_claim_update_nudge
        ])
        .build(tauri::generate_context!())
        .expect("error while building HanPage desktop application")
        .run(|app_handle, event| {
            // [#156] 새 창을 만드는 중에 마지막 창이 닫혀도 앱을 끝내지 않는다 — 그 창 몫의 문서를 잃는다.
            if let tauri::RunEvent::ExitRequested {
                code: None, api, ..
            } = &event
            {
                let mut reg = registry();
                if reg.creating() > 0 {
                    api.prevent_exit();
                    // 종료는 끝내지 못했다. 새 창이 뜬 뒤 엉뚱하게 이어지지 않게 정리한다.
                    reg.abandon_quit();
                }
            }
            // macOS: 파일 연결 더블클릭은 Opened{urls} 로 전달된다.
            #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
            if let tauri::RunEvent::Opened { urls } = &event {
                handle_opened(app_handle, urls);
            }
            let _ = (&app_handle, &event); // 플랫폼별 미사용 경고 억제
        });
}

#[cfg(all(test, desktop))]
mod tests {
    use super::*;

    #[test]
    fn launch_args_skip_executable_and_keep_only_documents() {
        let cwd = std::env::temp_dir();
        let doc = cwd.join("보고서 최종.HWP");
        let args = vec![
            cwd.join("HanPage.hwp").into_os_string(), // argv[0]: 확장자와 무관하게 실행 파일
            "--flag".into(),
            doc.clone().into_os_string(),
            cwd.join("note.txt").into_os_string(),
            cwd.join("양식.hwpx").into_os_string(),
            cwd.join("목록.HML").into_os_string(),
            cwd.join("스캔.pdf").into_os_string(),
        ];
        assert_eq!(
            document_paths_from_args(args, &cwd),
            vec![
                doc,
                cwd.join("양식.hwpx"),
                cwd.join("목록.HML"),
                cwd.join("스캔.pdf")
            ]
        );
    }

    #[test]
    fn new_windows_step_past_occupied_spots_and_wrap_inside_the_work_area() {
        let size = (1200.0, 840.0);
        let area = Some((0.0, 25.0, 1728.0, 1117.0));
        // main 을 다시 앞으로 가져온 뒤 새 창: doc-1 자리를 피한다.
        let main = (424.0, 88.0);
        assert_eq!(
            next_cascade_position(main, &[main, (452.0, 116.0)], size, area),
            (480.0, 144.0)
        );
        // 오른쪽 아래가 넘치면 작업 영역 왼쪽 위로 돌아간다.
        assert_eq!(
            next_cascade_position((500.0, 260.0), &[], size, area),
            (CASCADE_OFFSET, 25.0 + CASCADE_OFFSET)
        );
        assert_eq!(
            next_cascade_position((100.0, 100.0), &[], size, None),
            (128.0, 128.0)
        );
    }

    #[test]
    fn open_dialog_offers_pdf_only_when_import_is_available() {
        assert_eq!(open_dialog_filter(false).1, &["hwp", "hwpx", "hml"]);
        assert_eq!(open_dialog_filter(true).1, &["hwp", "hwpx", "hml", "pdf"]);
    }

    #[test]
    fn save_dialog_matches_the_chosen_format_and_never_offers_pdf() {
        assert_eq!(save_dialog_filter("보고서.HWPX").1, &["hwpx"]);
        assert_eq!(save_dialog_filter("목록.hml").1, &["hml"]);
        assert_eq!(save_dialog_filter("보고서.hwp").1, &["hwp"]);
        assert_eq!(save_dialog_filter("스캔.pdf").1, &["hwp"]);
        assert_eq!(save_dialog_filter("이름없음").1, &["hwp"]);
        assert!(is_pdf_path(Path::new("/tmp/원본.PDF")));
        assert!(!is_pdf_path(Path::new("/tmp/원본.hwpx")));
    }

    #[test]
    fn relative_launch_args_resolve_against_sender_cwd() {
        let cwd = std::env::temp_dir().join("hanpage-second-instance");
        let paths = document_paths_from_args(
            ["HanPage.exe".to_string(), "sub/문서.hwp".to_string()],
            &cwd,
        );
        assert_eq!(paths, vec![cwd.join("sub/문서.hwp")]);
    }

    #[test]
    fn startup_gate_keeps_early_instance_paths_after_startup_documents() {
        let mut gate = StartupGate::new();
        let early = std::env::temp_dir().join("두번째.hwp");
        assert!(gate.admit(vec![early.clone()]).is_empty());
        assert_eq!(gate.open(), vec![early]);
        let late = std::env::temp_dir().join("세번째.hwpx");
        assert_eq!(gate.admit(vec![late.clone()]), vec![late]);
        assert!(gate.open().is_empty());
    }

    #[test]
    fn update_relaunch_matches_args_even_after_nsis_strips_quotes() {
        let original = launch_args_key(["HanPage.exe", r"C:\Users\me\Desktop\보고서 최종.hwp"]);
        // NSIS GetOptions 가 /ARGS 의 따옴표를 벗겨 공백에서 인자가 쪼개진 재실행.
        let relaunched =
            launch_args_key(["HanPage.exe", r"C:\Users\me\Desktop\보고서", "최종.hwp"]);
        assert_eq!(original, relaunched);
        let marker = RelaunchMarker {
            args: original,
            version: "0.9.0".into(),
            at: 1_000,
        };
        assert!(is_update_relaunch(
            Some(&marker),
            &relaunched,
            "0.9.0",
            1_100
        ));
    }

    #[test]
    fn update_relaunch_does_not_hide_real_file_opens() {
        let marker = RelaunchMarker {
            args: launch_args_key(["HanPage.exe", "/docs/a.hwp"]),
            version: "0.9.0".into(),
            at: 1_000,
        };
        let same = launch_args_key(["HanPage.exe", "/docs/a.hwp"]);
        let other = launch_args_key(["HanPage.exe", "/docs/b.hwp"]);
        assert!(!is_update_relaunch(None, &same, "0.9.0", 1_100));
        // 다른 파일, 설치 실패로 이전 버전 실행, 유효 시간 경과는 실제 열기로 본다.
        assert!(!is_update_relaunch(Some(&marker), &other, "0.9.0", 1_100));
        assert!(!is_update_relaunch(Some(&marker), &same, "0.8.9", 1_100));
        assert!(!is_update_relaunch(
            Some(&marker),
            &same,
            "0.9.0",
            1_000 + RELAUNCH_MARKER_TTL_SECS + 1
        ));
        let empty = RelaunchMarker {
            args: String::new(),
            ..marker
        };
        assert!(!is_update_relaunch(Some(&empty), "", "0.9.0", 1_100));
    }

    #[test]
    fn launch_args_without_documents_open_nothing() {
        let cwd = std::env::temp_dir();
        assert!(document_paths_from_args(["HanPage.exe"], &cwd).is_empty());
        assert!(document_paths_from_args(Vec::<String>::new(), &cwd).is_empty());
    }
}
