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
//   - 파일 연결: macOS 는 `RunEvent::Opened{urls}`, Win/Linux 는 single-instance argv 로
//     경로를 받아 바이트를 읽고 "펜딩 큐"에 적재 후 웹뷰에 신호(emit)를 보낸다.
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
use std::sync::Mutex;

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

/// 웹뷰로 보내는 이벤트 이름.
const EVT_MENU: &str = "hanpage://menu"; // 메뉴 액션 → 스튜디오 커맨드 id
const EVT_DOCS_READY: &str = "hanpage://documents-ready"; // 펜딩 문서 도착 신호

/// 열기 dialog/파일 연결로 읽은 문서. `data` 는 파일 바이트(JSON 배열 직렬화).
#[derive(Serialize)]
struct OpenedFile {
    name: String,
    path: String,
    data: Vec<u8>,
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
#[derive(Default)]
struct PendingDocuments(Mutex<Vec<OpenedFile>>);

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
    })
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
    if let Some(state) = app.try_state::<PendingDocuments>() {
        if let Ok(mut q) = state.0.lock() {
            q.push(file);
        }
    }
    let _ = app.emit(EVT_DOCS_READY, ());
}

/// 경로를 읽어 펜딩 큐에 넣는다(파일 연결/최근 문서 클릭/single-instance 공통).
fn open_path(app: &tauri::AppHandle, path: PathBuf) {
    match read_document(&path) {
        Ok(file) => queue_document(app, file),
        Err(e) => eprintln!("[HanPage] 파일 열기 실패 {}: {}", path.display(), e),
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
fn cmd_take_pending_documents(state: tauri::State<'_, PendingDocuments>) -> Vec<OpenedFile> {
    state
        .0
        .lock()
        .map(|mut q| std::mem::take(&mut *q))
        .unwrap_or_default()
}

/// [Task #26] 업데이트 확인·설치 (desktop 전용).
/// - `manual=false`(시작 시): 새 버전이면 알림, 최신/오류는 조용히(로그만).
/// - `manual=true`(메뉴): 최신·오류도 대화상자로 안내.
#[cfg(desktop)]
async fn check_update(app: tauri::AppHandle, manual: bool) {
    use tauri_plugin_dialog::MessageDialogButtons;
    use tauri_plugin_updater::UpdaterExt;

    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            eprintln!("[updater] 초기화 실패: {e}");
            return;
        }
    };
    match updater.check().await {
        Ok(Some(update)) => {
            let ver = update.version.clone();
            let app2 = app.clone();
            app.dialog()
                .message(format!(
                    "새 버전 {ver} 이(가) 있습니다.\n지금 설치하면 내려받은 뒤 자동으로 다시 시작됩니다."
                ))
                .title("업데이트 있음")
                .buttons(MessageDialogButtons::OkCancelCustom(
                    "지금 설치".to_string(),
                    "나중에".to_string(),
                ))
                .show(move |install| {
                    if !install {
                        return;
                    }
                    tauri::async_runtime::spawn(async move {
                        match update.download_and_install(|_, _| {}, || {}).await {
                            Ok(_) => app2.restart(),
                            Err(e) => {
                                app2.dialog()
                                    .message(format!("업데이트 설치에 실패했습니다: {e}"))
                                    .title("업데이트")
                                    .show(|_| {});
                            }
                        }
                    });
                });
        }
        Ok(None) => {
            if manual {
                app.dialog()
                    .message("이미 최신 버전입니다.")
                    .title("업데이트")
                    .show(|_| {});
            }
        }
        Err(e) => {
            eprintln!("[updater] 확인 실패: {e}");
            if manual {
                app.dialog()
                    .message(format!("업데이트 확인에 실패했습니다: {e}"))
                    .title("업데이트")
                    .show(|_| {});
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    // single-instance 는 가장 먼저 등록해야 한다(Tauri 권장). Win/Linux 에서 2번째
    // 실행 argv 의 .hwp/.hwpx 경로를 캡처해 기존 창으로 넘긴다(macOS 는 Opened 사용).
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            for arg in argv.iter().skip(1) {
                let path = PathBuf::from(arg);
                let is_doc = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("hwp") || e.eq_ignore_ascii_case("hwpx"))
                    .unwrap_or(false);
                if is_doc {
                    open_path(app, path);
                }
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
            app.manage(PendingDocuments::default());
            // [Task #26] 시작 시 백그라운드 업데이트 확인(조용히; 새 버전이면 알림).
            #[cfg(desktop)]
            {
                let h = app.handle().clone();
                tauri::async_runtime::spawn(check_update(h, false));
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
                // [Task #26] 메뉴 수동 확인 — 최신/오류도 안내(manual=true).
                #[cfg(desktop)]
                {
                    let h = app.clone();
                    tauri::async_runtime::spawn(check_update(h, true));
                }
            } else {
                // 사용자 정의 항목 id = rhwp-studio 커맨드 id → 웹뷰 브리지가 dispatch.
                let _ = app.emit(EVT_MENU, id.to_string());
            }
        })
        .invoke_handler(tauri::generate_handler![
            cmd_open_document,
            cmd_save_document,
            cmd_take_pending_documents
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
