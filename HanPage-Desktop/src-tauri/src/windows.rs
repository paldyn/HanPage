//! 문서마다 창 하나 — 창 목록, 문서를 받을 창 결정(라우팅), 닫기·종료 판단.
//!
//! Tauri 형식에 의존하지 않는 순수 상태다. `lib.rs` 가 이 결정을 받아 창을 만들거나 앞으로
//! 가져오고 이벤트를 보낸다. 대기 문서는 창마다 따로 둔다 — 큐가 하나면 먼저 비운 창이
//! 다른 창 몫까지 가져가 편집 중인 문서를 덮을 수 있다.
//!
//! 규칙: 문서는 지금 창(가장 최근에 포커스한 창)이 손대지 않은 빈 창일 때만 그 창으로 가고,
//! 아니면 새 창을 만든다. 이미 어떤 창에 열린 파일이면 그 창을 앞으로 가져온다.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// 설정 파일(tauri.conf.json)이 만드는 첫 창.
pub const MAIN_LABEL: &str = "main";
/// 실행 중에 만드는 문서 창 레이블 접두어. 번호는 다시 쓰지 않는다.
pub const DOC_LABEL_PREFIX: &str = "doc-";
/// 닫기 확인을 보낸 뒤 웹뷰가 받았다는 응답이 없을 때, 다음 닫기를 그대로 허용하기까지의 시간.
/// 멈췄거나 다시 불러오다 실패한 창을 영영 닫지 못하게 두지 않는다.
pub const CLOSE_ACK_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// 창을 만들었고 웹뷰가 아직 대기 문서를 가져가지 않았다. 넣은 문서는 시작할 때 연다.
    Booting,
    /// 첫 대기 문서를 가져간 뒤 시작 흐름(자동 저장 복구 제안, 빈 문서) 중이다. 문서를 넣지 않는다 —
    /// 복구 대화상자 뒤에서 연 문서를 복구본이 덮을 수 있다.
    Startup,
    /// 시작 흐름이 끝나 웹뷰가 상태를 보고했다.
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CloseState {
    Idle,
    /// 닫기 확인을 보냈고 아직 받았다는 응답이 없다.
    Asked(Instant),
    /// 웹뷰가 받아서 사용자에게 묻는 중이다. 답할 때까지 다시 묻지 않는다.
    Acknowledged,
}

#[derive(Debug)]
pub struct WindowEntry<D> {
    pub phase: Phase,
    /// 손대지 않은 빈 문서라 문서를 받을 수 있다. 문서를 배정하면 바로 거짓이 된다(예약).
    pub pristine: bool,
    pub dirty: bool,
    /// 이 창에 열린(또는 열 예정인) 파일 경로.
    pub path: Option<PathBuf>,
    pub title: String,
    pub pending: VecDeque<D>,
    close: CloseState,
}

impl<D> WindowEntry<D> {
    fn new(phase: Phase, pristine: bool) -> Self {
        Self {
            phase,
            pristine,
            dirty: false,
            path: None,
            title: String::new(),
            pending: VecDeque::new(),
            close: CloseState::Idle,
        }
    }

    fn accepts_document(&self) -> bool {
        match self.phase {
            Phase::Booting => self.pristine,
            Phase::Startup => false,
            Phase::Ready => self.pristine && !self.dirty,
        }
    }
}

/// 문서를 받을 창 후보.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target<'a> {
    /// 가장 최근에 포커스한 창(파일 연결, 최근 문서).
    Front,
    /// 이 창(앱 안 파일 > 열기를 부른 창).
    Window(&'a str),
    /// 항상 새 창(손대지 않은 창이 아니라서 넘겨받은 문서).
    NewWindow,
}

/// 문서를 어디로 보낼지.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// 이미 이 창에 열린 파일이다. 앞으로 가져오기만 한다.
    Focus(String),
    /// 손대지 않은 이 창에서 연다.
    Into(String),
    /// 새 창을 만든다(레이블을 이미 등록했다).
    New(String),
}

/// 창 닫기 요청에 대한 판단.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseDecision {
    /// 그대로 닫는다.
    Allow,
    /// 닫기를 막고 웹뷰에 저장 여부를 묻게 한다.
    Ask,
    /// 이미 묻는 중이다. 이번 요청은 무시한다.
    Wait,
}

/// Cmd+Q 종료 진행의 다음 단계.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuitStep {
    /// 저장할 것이 없어 바로 닫을 창.
    pub close_now: Vec<String>,
    /// 다음으로 저장 여부를 물을 창.
    pub ask: Option<String>,
}

#[derive(Debug)]
pub struct Registry<D> {
    windows: HashMap<String, WindowEntry<D>>,
    /// 앞쪽이 가장 최근에 포커스한 창.
    mru: Vec<String>,
    next_id: u64,
    /// 만들기를 요청했지만 아직 다 만들지 않은 창 수. 그사이 마지막 창이 닫혀도 종료하지 않는다.
    creating: usize,
    quitting: bool,
}

impl<D> Default for Registry<D> {
    fn default() -> Self {
        Self::new()
    }
}

impl<D> Registry<D> {
    /// 첫 창(`main`)은 프로그램 시작부터 있다고 본다. macOS 는 콜드 스타트에서 파일 연결 이벤트가
    /// 창 생성보다 먼저 올 수 있다(#50).
    pub fn new() -> Self {
        let mut windows = HashMap::new();
        windows.insert(
            MAIN_LABEL.to_string(),
            WindowEntry::new(Phase::Booting, true),
        );
        Self {
            windows,
            mru: vec![MAIN_LABEL.to_string()],
            next_id: 1,
            creating: 0,
            quitting: false,
        }
    }

    /// 가장 최근에 포커스한 창.
    pub fn front(&self) -> Option<&str> {
        self.mru.first().map(String::as_str)
    }

    /// `label` 을 뺀 가장 최근 창(새 창 위치의 기준).
    pub fn front_except(&self, label: &str) -> Option<&str> {
        self.mru.iter().map(String::as_str).find(|l| *l != label)
    }

    #[cfg(test)]
    pub fn contains(&self, label: &str) -> bool {
        self.windows.contains_key(label)
    }

    pub fn entry(&self, label: &str) -> Option<&WindowEntry<D>> {
        self.windows.get(label)
    }

    /// 문서를 받을 창을 정하고 예약한다. 후보 창이 문서를 받을 수 없으면 새 창을 등록한다.
    pub fn route(&mut self, path: Option<&Path>, target: Target<'_>) -> Route {
        if let Some(path) = path {
            if let Some(label) = self.label_with_path(path) {
                return Route::Focus(label);
            }
        }
        let candidate = match target {
            Target::Front => self.mru.first().cloned(),
            Target::Window(label) => Some(label.to_string()),
            Target::NewWindow => None,
        };
        if let Some(label) = candidate {
            if let Some(entry) = self.windows.get_mut(&label) {
                if entry.accepts_document() {
                    entry.pristine = false;
                    entry.path = path.map(Path::to_path_buf);
                    return Route::Into(label);
                }
            }
        }
        let label = format!("{DOC_LABEL_PREFIX}{}", self.next_id);
        self.next_id += 1;
        let mut entry = WindowEntry::new(Phase::Booting, false);
        entry.path = path.map(Path::to_path_buf);
        self.windows.insert(label.clone(), entry);
        // 새 창은 곧 포커스를 받는다. 바로 이어 오는 문서도 이 창이 아니라 또 새 창으로 가게 맨 앞에 둔다.
        self.mru.insert(0, label.clone());
        self.creating += 1;
        Route::New(label)
    }

    fn label_with_path(&self, path: &Path) -> Option<String> {
        self.mru
            .iter()
            .find(|label| {
                self.windows
                    .get(*label)
                    .and_then(|entry| entry.path.as_deref())
                    .is_some_and(|open| same_document(open, path))
            })
            .cloned()
    }

    /// 창의 대기 문서에 넣는다. 창이 없으면(그사이 닫힘) 문서를 돌려준다.
    pub fn enqueue(&mut self, label: &str, doc: D) -> Result<(), D> {
        match self.windows.get_mut(label) {
            Some(entry) => {
                entry.pending.push_back(doc);
                Ok(())
            }
            None => Err(doc),
        }
    }

    /// 웹뷰가 대기 문서를 가져간다. `initial` 은 페이지가 (다시) 시작할 때의 첫 호출이다 —
    /// 다시 불러온 창은 문서가 사라졌으므로 상태를 처음부터 다시 받는다.
    pub fn take_pending(&mut self, label: &str, initial: bool) -> Vec<D> {
        let entry = self
            .windows
            .entry(label.to_string())
            .or_insert_with(|| WindowEntry::new(Phase::Startup, false));
        if !self.mru.iter().any(|l| l == label) {
            self.mru.push(label.to_string());
        }
        if initial {
            entry.phase = Phase::Startup;
            entry.dirty = false;
            entry.close = CloseState::Idle;
            if entry.pending.is_empty() {
                entry.path = None;
            }
        }
        entry.pending.drain(..).collect()
    }

    /// 웹뷰가 보고한 창 상태. 아직 가져가지 않은 문서가 있으면 예약을 유지한다 — 늦게 도착한
    /// 이전 상태 보고 때문에 같은 창에 문서가 두 번 배정되지 않게 한다.
    pub fn report(&mut self, label: &str, pristine: bool, dirty: bool, path: Option<PathBuf>) {
        let Some(entry) = self.windows.get_mut(label) else {
            return;
        };
        entry.phase = Phase::Ready;
        entry.dirty = dirty;
        if entry.pending.is_empty() {
            entry.pristine = pristine && !dirty;
            entry.path = path;
        } else {
            entry.pristine = false;
        }
    }

    pub fn set_title(&mut self, label: &str, title: String) {
        if let Some(entry) = self.windows.get_mut(label) {
            entry.title = title;
        }
    }

    /// 다른 이름으로 저장 등으로 창의 파일이 바뀌었다.
    pub fn set_path(&mut self, label: &str, path: PathBuf) {
        if let Some(entry) = self.windows.get_mut(label) {
            entry.path = Some(path);
        }
    }

    pub fn focus(&mut self, label: &str) {
        if !self.windows.contains_key(label) {
            return;
        }
        self.mru.retain(|l| l != label);
        self.mru.insert(0, label.to_string());
    }

    /// 창이 없어졌다. 아직 열지 못한 문서는 함께 버린다(사용자가 그 창을 닫았다).
    pub fn remove(&mut self, label: &str) -> Option<WindowEntry<D>> {
        self.mru.retain(|l| l != label);
        self.windows.remove(label)
    }

    /// 창 만들기가 끝났다. 실패하면 등록을 지우고 그 창 몫의 문서를 돌려준다.
    pub fn created(&mut self, label: &str, ok: bool) -> Vec<D> {
        self.creating = self.creating.saturating_sub(1);
        if ok {
            return Vec::new();
        }
        self.remove(label)
            .map(|entry| entry.pending.into_iter().collect())
            .unwrap_or_default()
    }

    pub fn creating(&self) -> usize {
        self.creating
    }

    #[cfg(test)]
    pub fn is_quitting(&self) -> bool {
        self.quitting
    }

    /// 창 닫기 요청(닫기 버튼, Alt+F4, 종료 진행).
    pub fn close_requested(&mut self, label: &str, now: Instant) -> CloseDecision {
        let Some(entry) = self.windows.get_mut(label) else {
            return CloseDecision::Allow;
        };
        if !entry.dirty {
            return CloseDecision::Allow;
        }
        match entry.close {
            CloseState::Acknowledged => CloseDecision::Wait,
            CloseState::Asked(at) if now.saturating_duration_since(at) < CLOSE_ACK_TIMEOUT => {
                CloseDecision::Wait
            }
            CloseState::Asked(_) => CloseDecision::Allow,
            CloseState::Idle => {
                entry.close = CloseState::Asked(now);
                CloseDecision::Ask
            }
        }
    }

    /// 웹뷰가 닫기 확인을 받아 사용자에게 묻기 시작했다.
    pub fn close_acknowledged(&mut self, label: &str) {
        if let Some(entry) = self.windows.get_mut(label) {
            if matches!(entry.close, CloseState::Asked(_)) {
                entry.close = CloseState::Acknowledged;
            }
        }
    }

    /// 사용자가 답했다. 닫지 않기로 했으면 진행 중인 종료도 멈춘다.
    pub fn close_answered(&mut self, label: &str, confirmed: bool) {
        if let Some(entry) = self.windows.get_mut(label) {
            entry.close = CloseState::Idle;
        }
        if !confirmed {
            self.quitting = false;
        }
    }

    /// 저장하지 않은 변경이 있는 다른 창(가장 최근 순).
    pub fn dirty_windows_except(&self, label: &str) -> Vec<String> {
        self.mru
            .iter()
            .filter(|l| l.as_str() != label)
            .filter(|l| self.windows.get(*l).is_some_and(|e| e.dirty))
            .cloned()
            .collect()
    }

    /// Cmd+Q: 저장할 것이 없는 창은 바로 닫고, 저장할 것이 있는 창은 하나씩 묻는다.
    pub fn begin_quit(&mut self) -> QuitStep {
        self.quitting = true;
        let close_now = self
            .mru
            .iter()
            .filter(|l| self.windows.get(*l).is_some_and(|e| !e.dirty))
            .cloned()
            .collect();
        QuitStep {
            close_now,
            ask: self.next_dirty_to_ask(),
        }
    }

    /// 종료 중 한 창이 닫힌 뒤 다음으로 물을 창. 종료 중이 아니거나 이미 묻는 중이면 없다.
    pub fn next_quit_ask(&mut self) -> Option<String> {
        if !self.quitting {
            return None;
        }
        let busy = self
            .windows
            .values()
            .any(|e| !matches!(e.close, CloseState::Idle));
        if busy {
            return None;
        }
        self.next_dirty_to_ask()
    }

    fn next_dirty_to_ask(&mut self) -> Option<String> {
        let label = self
            .mru
            .iter()
            .find(|l| self.windows.get(*l).is_some_and(|e| e.dirty))
            .cloned()?;
        if let Some(entry) = self.windows.get_mut(&label) {
            entry.close = CloseState::Asked(Instant::now());
        }
        Some(label)
    }
}

/// 같은 파일인지. 호출자가 정규화한 경로를 넘긴다. Windows 파일 시스템은 대소문자를 구분하지 않는다.
pub fn same_document(a: &Path, b: &Path) -> bool {
    if cfg!(windows) {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    } else {
        a == b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(path: &str) -> PathBuf {
        PathBuf::from(path)
    }

    #[test]
    fn cold_start_document_goes_to_main_and_the_next_one_to_a_new_window() {
        let mut r: Registry<&str> = Registry::new();
        assert_eq!(
            r.route(Some(&p("/a.hwp")), Target::Front),
            Route::Into("main".into())
        );
        assert_eq!(
            r.route(Some(&p("/b.hwp")), Target::Front),
            Route::New("doc-1".into())
        );
        assert_eq!(
            r.route(Some(&p("/c.hwp")), Target::Front),
            Route::New("doc-2".into())
        );
        assert_eq!(r.creating(), 2);
        assert_eq!(r.front(), Some("doc-2"));
    }

    #[test]
    fn the_same_file_brings_its_window_forward() {
        let mut r: Registry<&str> = Registry::new();
        r.route(Some(&p("/a.hwp")), Target::Front);
        r.route(Some(&p("/b.hwp")), Target::Front);
        assert_eq!(
            r.route(Some(&p("/a.hwp")), Target::Front),
            Route::Focus("main".into())
        );
        assert_eq!(
            r.route(Some(&p("/b.hwp")), Target::Front),
            Route::Focus("doc-1".into())
        );
    }

    #[test]
    fn a_pristine_ready_window_is_reused_but_not_while_starting_or_dirty() {
        let mut r: Registry<&str> = Registry::new();
        assert!(r.take_pending("main", true).is_empty());
        // 시작 흐름(복구 제안·빈 문서) 중에는 받지 않는다.
        assert_eq!(r.route(None, Target::Front), Route::New("doc-1".into()));
        r.remove("doc-1");
        r.report("main", true, false, None);
        assert_eq!(
            r.route(Some(&p("/a.hwp")), Target::Front),
            Route::Into("main".into())
        );
        // 문서를 연 창, 편집 중인 빈 창은 받지 않는다.
        r.report("main", false, false, Some(p("/a.hwp")));
        assert_eq!(
            r.route(Some(&p("/b.hwp")), Target::Front),
            Route::New("doc-2".into())
        );
        r.report("main", true, true, None);
        r.focus("main");
        assert_eq!(
            r.route(Some(&p("/c.hwp")), Target::Front),
            Route::New("doc-3".into())
        );
    }

    #[test]
    fn only_the_front_window_is_considered_for_reuse() {
        let mut r: Registry<&str> = Registry::new();
        r.take_pending("main", true);
        r.report("main", true, false, None);
        assert_eq!(
            r.route(Some(&p("/a.hwp")), Target::Front),
            Route::Into("main".into())
        );
        let Route::New(doc) = r.route(Some(&p("/b.hwp")), Target::Front) else {
            panic!("새 창이어야 한다");
        };
        r.take_pending(&doc, true);
        r.report(&doc, false, false, Some(p("/b.hwp")));
        // 뒤에 손대지 않은 창이 있어도 앞 창이 문서를 열고 있으면 새 창을 만든다.
        r.report("main", true, false, None);
        r.focus(&doc);
        assert!(matches!(
            r.route(Some(&p("/c.hwp")), Target::Front),
            Route::New(_)
        ));
    }

    #[test]
    fn preferred_window_is_used_only_when_pristine() {
        let mut r: Registry<&str> = Registry::new();
        r.take_pending("main", true);
        r.report("main", true, false, None);
        assert_eq!(
            r.route(Some(&p("/a.hwp")), Target::Window("main")),
            Route::Into("main".into())
        );
        r.report("main", false, false, Some(p("/a.hwp")));
        assert_eq!(
            r.route(Some(&p("/b.hwp")), Target::Window("main")),
            Route::New("doc-1".into())
        );
    }

    #[test]
    fn handed_off_documents_always_get_a_new_window_unless_already_open() {
        let mut r: Registry<&str> = Registry::new();
        r.take_pending("main", true);
        // 웹뷰는 손대지 않은 창이 아니라고 판단해 넘겼다. 등록부가 아직 빈 창으로 알아도 되돌려 보내지 않는다.
        r.report("main", true, false, None);
        assert_eq!(r.route(None, Target::NewWindow), Route::New("doc-1".into()));
        r.report("main", false, false, Some(p("/a.hwp")));
        assert_eq!(
            r.route(Some(&p("/a.hwp")), Target::NewWindow),
            Route::Focus("main".into())
        );
    }

    #[test]
    fn stale_report_does_not_release_a_reservation_with_pending_documents() {
        let mut r: Registry<&str> = Registry::new();
        r.take_pending("main", true);
        r.report("main", true, false, None);
        assert_eq!(
            r.route(Some(&p("/a.hwp")), Target::Front),
            Route::Into("main".into())
        );
        r.enqueue("main", "a").unwrap();
        // 문서를 가져가기 전에 도착한 이전 보고.
        r.report("main", true, false, None);
        assert_eq!(
            r.route(Some(&p("/a.hwp")), Target::Front),
            Route::Focus("main".into())
        );
        assert!(matches!(
            r.route(Some(&p("/b.hwp")), Target::Front),
            Route::New(_)
        ));
        assert_eq!(r.take_pending("main", false), vec!["a"]);
    }

    #[test]
    fn reload_resets_the_window_and_forgets_the_lost_document() {
        let mut r: Registry<&str> = Registry::new();
        r.take_pending("main", true);
        r.report("main", false, true, Some(p("/a.hwp")));
        // 페이지가 다시 시작했다 — 문서와 변경은 사라졌다.
        r.take_pending("main", true);
        let entry = r.entry("main").unwrap();
        assert_eq!(entry.phase, Phase::Startup);
        assert!(!entry.dirty);
        assert_eq!(entry.path, None);
        assert_eq!(
            r.close_requested("main", Instant::now()),
            CloseDecision::Allow
        );
    }

    #[test]
    fn failed_window_creation_returns_its_documents() {
        let mut r: Registry<&str> = Registry::new();
        r.route(Some(&p("/a.hwp")), Target::Front);
        let Route::New(doc) = r.route(Some(&p("/b.hwp")), Target::Front) else {
            panic!("새 창이어야 한다");
        };
        r.enqueue(&doc, "b").unwrap();
        assert_eq!(r.created(&doc, false), vec!["b"]);
        assert!(!r.contains(&doc));
        assert_eq!(r.creating(), 0);
        assert_eq!(r.enqueue(&doc, "late"), Err("late"));
    }

    #[test]
    fn closing_a_dirty_window_asks_once_and_waits_for_the_answer() {
        let mut r: Registry<&str> = Registry::new();
        let t0 = Instant::now();
        r.take_pending("main", true);
        r.report("main", false, false, Some(p("/a.hwp")));
        assert_eq!(r.close_requested("main", t0), CloseDecision::Allow);
        r.report("main", false, true, Some(p("/a.hwp")));
        assert_eq!(r.close_requested("main", t0), CloseDecision::Ask);
        assert_eq!(r.close_requested("main", t0), CloseDecision::Wait);
        r.close_acknowledged("main");
        // 사용자가 오래 고민해도 두 번째 닫기로 저장 확인을 건너뛰지 않는다.
        assert_eq!(
            r.close_requested("main", t0 + CLOSE_ACK_TIMEOUT * 10),
            CloseDecision::Wait
        );
        r.close_answered("main", false);
        assert_eq!(r.close_requested("main", t0), CloseDecision::Ask);
    }

    #[test]
    fn an_unresponsive_window_can_be_closed_after_the_ack_timeout() {
        let mut r: Registry<&str> = Registry::new();
        let t0 = Instant::now();
        r.take_pending("main", true);
        r.report("main", false, true, None);
        assert_eq!(r.close_requested("main", t0), CloseDecision::Ask);
        assert_eq!(
            r.close_requested("main", t0 + CLOSE_ACK_TIMEOUT - Duration::from_millis(1)),
            CloseDecision::Wait
        );
        assert_eq!(
            r.close_requested("main", t0 + CLOSE_ACK_TIMEOUT),
            CloseDecision::Allow
        );
    }

    #[test]
    fn update_apply_sees_other_dirty_windows() {
        let mut r: Registry<&str> = Registry::new();
        r.take_pending("main", true);
        r.report("main", false, true, None);
        let Route::New(doc) = r.route(Some(&p("/b.hwp")), Target::Front) else {
            panic!("새 창이어야 한다");
        };
        r.take_pending(&doc, true);
        r.report(&doc, false, false, Some(p("/b.hwp")));
        assert_eq!(r.dirty_windows_except(&doc), vec!["main".to_string()]);
        assert!(r.dirty_windows_except("main").is_empty());
    }

    #[test]
    fn quit_closes_clean_windows_and_asks_dirty_ones_one_at_a_time() {
        let mut r: Registry<&str> = Registry::new();
        r.take_pending("main", true);
        r.report("main", false, true, Some(p("/a.hwp")));
        let Route::New(b) = r.route(Some(&p("/b.hwp")), Target::Front) else {
            panic!("새 창이어야 한다");
        };
        r.take_pending(&b, true);
        r.report(&b, false, false, Some(p("/b.hwp")));
        let Route::New(c) = r.route(Some(&p("/c.hwp")), Target::Front) else {
            panic!("새 창이어야 한다");
        };
        r.take_pending(&c, true);
        r.report(&c, false, true, Some(p("/c.hwp")));

        let step = r.begin_quit();
        assert_eq!(step.close_now, vec![b.clone()]);
        assert_eq!(step.ask, Some(c.clone()));
        assert_eq!(r.next_quit_ask(), None, "묻는 중에는 다음 창을 묻지 않는다");
        r.close_answered(&c, true);
        r.remove(&c);
        r.remove(&b);
        assert_eq!(r.next_quit_ask(), Some("main".to_string()));
        // 사용자가 취소하면 종료를 멈춘다.
        r.close_answered("main", false);
        assert!(!r.is_quitting());
        assert_eq!(r.next_quit_ask(), None);
    }
}
