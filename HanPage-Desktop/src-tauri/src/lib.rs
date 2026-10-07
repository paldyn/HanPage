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
//   store/window-state/single-instance 모두 Rust 측에서만 사용하므로 JS ACL 권한
//   추가가 불필요하다(core:default 유지). 프런트 브리지는 web-inert
//   (`rhwp-studio/src/core/desktop-bridge.ts`)로, 브라우저에서는 완전한 no-op 이다.

use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

#[cfg(desktop)]
use std::sync::Arc;
#[cfg(desktop)]
use std::time::Instant;

#[cfg(desktop)]
mod update_state;
#[cfg(desktop)]
use update_state::{should_emit_download_progress, UpdateSlot, UpdateState};

use serde::Serialize;
// 네이티브 메뉴는 macOS 시스템 메뉴바 전용(이슈 #7). 비-macOS 는 메뉴 미부착.
#[cfg(target_os = "macos")]
use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_store::StoreExt;

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

/// 파일 연결/최근 문서로 열린 문서를 웹뷰가 가져갈 때까지 보관하는 큐.
/// (콜드 스타트 시 웹뷰가 준비되기 전에 도착한 문서를 잃지 않기 위함.)
///
/// [#50] Tauri managed state 가 아니라 **프로세스 전역**으로 둔다. macOS 는 파일 연결
/// 더블클릭을 `RunEvent::Opened` 로 전달하는데, 콜드 스타트에서는 이 이벤트가
/// `setup()` 의 `app.manage(..)` 보다 먼저 도착할 수 있다(실측: 웹뷰 생성 816ms 전).
/// 예전 구현은 `try_state()` 가 `None` 이면 문서를 조용히 버려서, 더블클릭으로 연 문서가
/// 영영 열리지 않았다. 전역 큐는 프로그램 시작 시점부터 존재하므로 순서와 무관하다.
static PENDING_DOCUMENTS: LazyLock<Mutex<Vec<OpenedFile>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

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

/// `.hwp`/`.hwpx` 확장자인지(대소문자 무시) — 파일 연결 대상과 같다.
#[cfg(desktop)]
fn is_document_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("hwp") || e.eq_ignore_ascii_case("hwpx"))
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

// ─── 펜딩 큐(파일 연결/최근 문서 → 웹뷰) ──────────────────────────────────────
/// 문서를 펜딩 큐에 넣고 최근 목록에 등록한 뒤 웹뷰에 도착 신호를 보낸다.
fn queue_document(app: &tauri::AppHandle, file: OpenedFile) {
    record_recent(app, &file.path, &file.name);
    if let Ok(mut q) = PENDING_DOCUMENTS.lock() {
        q.push(file);
    }
    // 웹뷰가 이미 떠 있으면(웜 스타트) 즉시 알린다. 아직 없으면(콜드 스타트) 웹뷰가
    // 초기화 시 `cmd_take_pending_documents` 로 직접 가져가므로 유실되지 않는다.
    let _ = app.emit(EVT_DOCS_READY, ());
}

/// 읽지 못한 문서를 실패 항목으로 큐에 넣고 웹뷰에 도착 신호를 보낸다.
/// 문서와 같은 큐를 써서 콜드 스타트에서도 유실되지 않고 도착 순서도 지킨다.
fn queue_failure(app: &tauri::AppHandle, path: &Path, message: String) {
    if let Ok(mut q) = PENDING_DOCUMENTS.lock() {
        q.push(OpenedFile {
            name: display_name(path),
            path: path.to_string_lossy().into_owned(),
            data: Vec::new(),
            error: Some(message),
        });
    }
    let _ = app.emit(EVT_DOCS_READY, ());
}

/// 경로를 읽어 펜딩 큐에 넣는다(파일 연결/최근 문서 클릭/single-instance 공통).
/// [#121] 읽지 못하면 실패 항목을 넣는다 — 조용히 버리면 사용자는 빈 창만 보고 다시 열어야 한다.
fn open_path(app: &tauri::AppHandle, path: PathBuf) {
    match read_document(&path) {
        Ok(file) => queue_document(app, file),
        Err(e) => {
            eprintln!("[HanPage] 파일 열기 실패 {}: {}", path.display(), e);
            queue_failure(app, &path, e);
        }
    }
}

/// macOS/모바일: 파일 연결 더블클릭 시 전달되는 URL 들을 처리한다.
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
fn handle_opened(app: &tauri::AppHandle, urls: &[tauri::Url]) {
    for url in urls {
        if let Ok(path) = url.to_file_path() {
            open_path(app, path);
        }
    }
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
    let app_menu = SubmenuBuilder::new(app, "HanPage")
        .about(None)
        .item(&check_update)
        .separator()
        .quit()
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
/// 네이티브 열기 dialog → 선택 파일 바이트 반환. 취소 시 `Ok(None)`.
///
/// async command 는 메인 스레드가 아닌 async 런타임 워커에서 실행되므로 여기서
/// `blocking_pick_file()`(내부적으로 dialog 를 메인 스레드에 디스패치)을 호출해도
/// 데드락이 없다. 메인 스레드에서 직접 호출하면 안 된다.
#[tauri::command]
async fn cmd_open_document(app: tauri::AppHandle) -> Result<Option<OpenedFile>, String> {
    let picked = app
        .dialog()
        .file()
        .add_filter("한글 문서 (HWP/HWPX)", &["hwp", "hwpx"])
        .blocking_pick_file();

    let Some(file_path) = picked else {
        return Ok(None); // 사용자 취소
    };
    let path = file_path.into_path().map_err(|e| e.to_string())?;
    let file = read_document(&path)?;
    record_recent(&app, &file.path, &file.name);
    Ok(Some(file))
}

/// 네이티브 저장 dialog → 선택 경로에 바이트 기록. 취소 시 `SaveOutcome::Cancelled`.
#[tauri::command]
async fn cmd_save_document(
    app: tauri::AppHandle,
    suggested_name: String,
    data: Vec<u8>,
) -> Result<SaveOutcome, String> {
    let picked = app
        .dialog()
        .file()
        .set_file_name(&suggested_name)
        .add_filter("한글 문서 (HWP)", &["hwp"])
        .blocking_save_file();

    let Some(file_path) = picked else {
        return Ok(SaveOutcome::Cancelled); // 사용자 취소
    };
    let path = file_path.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, &data).map_err(|e| e.to_string())?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document.hwp")
        .to_string();
    Ok(SaveOutcome::Saved {
        path: path.to_string_lossy().into_owned(),
        name,
    })
}

/// 펜딩 큐를 비우고 반환한다(웹뷰가 init 시점·도착 신호 수신 시 호출).
#[tauri::command]
fn cmd_take_pending_documents() -> Vec<OpenedFile> {
    PENDING_DOCUMENTS
        .lock()
        .map(|mut q| std::mem::take(&mut *q))
        .unwrap_or_default()
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
#[cfg(desktop)]
#[tauri::command]
async fn cmd_update_apply(app: tauri::AppHandle) -> Result<(), String> {
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
            for path in ready {
                open_path(app, path);
            }
            if let Some(w) = app.webview_windows().values().next() {
                let _ = w.set_focus();
            }
        }));

        // [Task #26] updater: 새 릴리스 확인/다운로드/설치 (desktop 전용).
        // 시작 시 자동 확인·메뉴 핸들러는 Stage 2 에서 추가한다.
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
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
            } else if id == "app:check-update" {
                // [#59] 메뉴 수동 확인 → 웹뷰와 동일 흐름. 확인/다운로드는 조용히 진행하고
                // 안내(내려받는 중·준비됨·최신·오류)는 웹뷰 토스트가 담당한다.
                let _ = app.emit(EVT_MENU, id.to_string());
            } else {
                // 사용자 정의 항목 id = rhwp-studio 커맨드 id → 웹뷰 브리지가 dispatch.
                let _ = app.emit(EVT_MENU, id.to_string());
            }
        })
        .invoke_handler(tauri::generate_handler![
            cmd_open_document,
            cmd_save_document,
            cmd_take_pending_documents,
            cmd_update_status,
            cmd_update_check,
            cmd_update_apply
        ])
        .build(tauri::generate_context!())
        .expect("error while building HanPage desktop application")
        .run(|app_handle, event| {
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
        ];
        assert_eq!(
            document_paths_from_args(args, &cwd),
            vec![doc, cwd.join("양식.hwpx")]
        );
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
