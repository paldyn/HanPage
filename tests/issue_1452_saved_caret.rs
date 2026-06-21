//! Issue #1452: 텍스트 없이 TAC 그림만 있는 문단의 저장 커서 위치 복원.

use rhwp::document_core::DocumentCore;
use rhwp::model::control::Control;
use serde_json::Value;

fn parse_json(label: &str, json: &str) -> Value {
    serde_json::from_str(json).unwrap_or_else(|e| panic!("parse {label} json `{json}`: {e}"))
}

fn load_transparency_core() -> DocumentCore {
    let repo_root = env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(repo_root).join("samples/투명도0-50.hwp");
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    DocumentCore::from_bytes(&bytes).expect("load samples/투명도0-50.hwp")
}

fn picture_count(core: &DocumentCore, para_idx: usize) -> usize {
    core.document().sections[0].paragraphs[para_idx]
        .controls
        .iter()
        .filter(|ctrl| matches!(ctrl, Control::Picture(_)))
        .count()
}

#[test]
fn transparency_sample_restores_saved_caret_after_second_inline_picture() {
    let repo_root = env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(repo_root).join("samples/투명도0-50.hwp");
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let parsed = rhwp::parser::parse_hwp(&bytes).expect("parse raw samples/투명도0-50.hwp");
    let props = &parsed.doc_properties;
    assert_eq!(props.caret_list_id, 0);
    assert_eq!(props.caret_para_id, 0);
    assert_eq!(props.caret_char_pos, 32);

    let mut doc =
        rhwp::wasm_api::HwpDocument::from_bytes(&bytes).expect("load samples/투명도0-50.hwp");

    let caret_json = doc.get_caret_position().expect("getCaretPosition");
    let caret = parse_json("caret", &caret_json);
    assert_eq!(caret["sectionIndex"], 0);
    assert_eq!(caret["paragraphIndex"], 0);
    assert_eq!(caret["charOffset"], 2);

    let first_line_rect = parse_json(
        "first-line cursor rect",
        &doc.get_cursor_rect_native(0, 0, 0)
            .expect("first line cursor rect"),
    );
    let saved_rect = parse_json(
        "saved cursor rect",
        &doc.get_cursor_rect_native(0, 0, 2)
            .expect("saved cursor rect"),
    );

    assert!(
        saved_rect["y"].as_f64().unwrap() > first_line_rect["y"].as_f64().unwrap(),
        "저장 커서는 첫 줄 시작이 아니라 두 번째 TAC 그림 뒤에 있어야 함: first={first_line_rect}, saved={saved_rect}"
    );
    assert!(
        saved_rect["x"].as_f64().unwrap() > first_line_rect["x"].as_f64().unwrap() + 400.0,
        "저장 커서는 두 번째 줄 첫머리가 아니라 두 번째 TAC 그림 오른쪽 끝에 있어야 함: first={first_line_rect}, saved={saved_rect}"
    );
    assert!(
        saved_rect["height"].as_f64().unwrap() < 40.0,
        "TAC 그림 뒤 캐럿 높이는 그림 높이가 아니라 글자 높이여야 함: saved={saved_rect}"
    );

    doc.set_show_paragraph_marks(true);
    let visible_before_first_picture_rect = parse_json(
        "visible before-first-picture cursor rect",
        &doc.get_cursor_rect_native(0, 0, 0)
            .expect("visible before-first-picture cursor rect"),
    );
    let visible_mark_rect = parse_json(
        "visible paragraph-mark cursor rect",
        &doc.get_cursor_rect_native(0, 0, 2)
            .expect("visible paragraph-mark cursor rect"),
    );
    let visible_before_second_picture_rect = parse_json(
        "visible before-second-picture cursor rect",
        &doc.get_cursor_rect_native(0, 0, 1)
            .expect("visible before-second-picture cursor rect"),
    );
    assert!(
        visible_mark_rect["x"].as_f64().unwrap() <= saved_rect["x"].as_f64().unwrap() + 1.0,
        "문단부호 표시 중 캐럿이 문단부호 오른쪽으로 과도하게 밀리면 안 됨: hidden={saved_rect}, visible={visible_mark_rect}"
    );
    assert_eq!(
        visible_mark_rect["y"], saved_rect["y"],
        "문단부호 표시 여부가 그림 bbox 기준 캐럿 y를 임의로 바꾸면 안 됨: hidden={saved_rect}, visible={visible_mark_rect}"
    );
    assert_eq!(visible_mark_rect["height"], saved_rect["height"]);
    assert!(
        visible_before_first_picture_rect["x"].as_f64().unwrap()
            <= visible_before_second_picture_rect["x"].as_f64().unwrap() + 1.0,
        "첫 번째 그림 앞 커서는 첫 번째 그림 왼쪽 기준에 있어야 함: first={visible_before_first_picture_rect}, second={visible_before_second_picture_rect}"
    );
    assert!(
        visible_before_first_picture_rect["y"].as_f64().unwrap()
            < visible_before_second_picture_rect["y"].as_f64().unwrap(),
        "첫 번째 그림 앞 커서는 첫 번째 그림 bbox 기준선에 있어야 함: first={visible_before_first_picture_rect}, second={visible_before_second_picture_rect}"
    );
    assert!(
        visible_before_second_picture_rect["x"].as_f64().unwrap()
            < visible_mark_rect["x"].as_f64().unwrap(),
        "왼쪽 이동 후 커서는 두 번째 그림의 왼쪽에 있어야 함: before_second={visible_before_second_picture_rect}, visible={visible_mark_rect}"
    );
    assert_eq!(
        visible_before_second_picture_rect["y"], visible_mark_rect["y"],
        "왼쪽 이동 후 두 번째 그림 앞 커서도 문단부호 표시 기준선에 맞아야 함: before_second={visible_before_second_picture_rect}, visible={visible_mark_rect}"
    );
}

#[test]
fn enter_before_second_tac_picture_splits_inline_controls() {
    let mut core = load_transparency_core();

    core.split_paragraph_native(0, 0, 1)
        .expect("두 TAC 그림 사이 Enter");

    assert_eq!(core.document().sections[0].paragraphs.len(), 2);
    assert_eq!(
        picture_count(&core, 0),
        1,
        "첫 문단에는 첫 번째 그림만 남아야 한다"
    );
    assert_eq!(
        picture_count(&core, 1),
        1,
        "새 문단에는 두 번째 그림이 이동해야 한다"
    );
}

#[test]
fn repeated_enter_before_second_tac_picture_keeps_picture_after_blank_lines() {
    let mut core = load_transparency_core();

    core.split_paragraph_native(0, 0, 1)
        .expect("두 TAC 그림 사이 Enter");
    core.split_paragraph_native(0, 1, 0)
        .expect("두 번째 그림 앞 Enter 1회 추가");
    core.split_paragraph_native(0, 2, 0)
        .expect("두 번째 그림 앞 Enter 2회 추가");

    assert_eq!(core.document().sections[0].paragraphs.len(), 4);
    assert_eq!(picture_count(&core, 0), 1);
    assert_eq!(picture_count(&core, 1), 0);
    assert_eq!(picture_count(&core, 2), 0);
    assert_eq!(
        picture_count(&core, 3),
        1,
        "반복 Enter 후에도 두 번째 그림은 빈 문단들 뒤에 남아야 한다"
    );
}

#[test]
fn repeated_enter_after_second_tac_picture_appends_blank_lines() {
    let mut core = load_transparency_core();

    core.split_paragraph_native(0, 0, 2)
        .expect("두 번째 TAC 그림 뒤 Enter");
    core.split_paragraph_native(0, 1, 0)
        .expect("두 번째 그림 뒤 Enter 1회 추가");
    core.split_paragraph_native(0, 2, 0)
        .expect("두 번째 그림 뒤 Enter 2회 추가");

    assert_eq!(core.document().sections[0].paragraphs.len(), 4);
    assert_eq!(
        picture_count(&core, 0),
        2,
        "두 번째 그림 뒤 Enter는 두 그림을 앞 문단에 유지해야 한다"
    );
    assert_eq!(picture_count(&core, 1), 0);
    assert_eq!(picture_count(&core, 2), 0);
    assert_eq!(picture_count(&core, 3), 0);
}

#[test]
fn arrow_up_from_second_tac_picture_end_moves_to_first_picture_end() {
    let doc = {
        let repo_root = env!("CARGO_MANIFEST_DIR");
        let path = std::path::Path::new(repo_root).join("samples/투명도0-50.hwp");
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        rhwp::wasm_api::HwpDocument::from_bytes(&bytes).expect("load samples/투명도0-50.hwp")
    };

    let moved = parse_json(
        "move up from second picture end",
        &doc.move_vertical(0, 0, 2, -1, -1.0, u32::MAX, u32::MAX, u32::MAX, u32::MAX)
            .expect("moveVertical ArrowUp"),
    );
    assert_eq!(moved["sectionIndex"], 0);
    assert_eq!(moved["paragraphIndex"], 0);
    assert_eq!(
        moved["charOffset"], 1,
        "두 번째 TAC 그림 끝에서 위쪽 이동하면 첫 번째 TAC 그림 끝으로 가야 한다: {moved}"
    );
}
