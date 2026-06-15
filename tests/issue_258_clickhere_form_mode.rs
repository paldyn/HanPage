//! Issue #258: 누름틀 양식 모드 편집 가능 속성 회귀 가드.

use std::fs;
use std::path::Path;

use rhwp::document_core::DocumentCore;
use rhwp::model::control::FieldType;

fn make_doc_with_inserted_clickhere() -> DocumentCore {
    let mut core = DocumentCore::new_empty();
    core.create_blank_document_native()
        .expect("create blank document");
    core.insert_text_native(0, 0, 0, "ABC")
        .expect("insert base text");
    core.insert_click_here_field_at(0, 0, 1, "입력하세요", "메모", "name01", true)
        .expect("insert clickhere field");
    core
}

fn assert_clickhere_form_editable(path: &Path) {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
    let core = DocumentCore::from_bytes(&bytes)
        .unwrap_or_else(|e| panic!("parse {}: {:?}", path.display(), e));

    let fields = core.collect_all_fields();
    let click_fields: Vec<_> = fields
        .iter()
        .filter(|f| f.field.field_type == FieldType::ClickHere)
        .collect();

    assert_eq!(
        click_fields.len(),
        2,
        "{} should contain two ClickHere fields",
        path.display()
    );

    for field in click_fields {
        assert!(
            field.field.is_editable_in_form(),
            "{} ClickHere field should be editable in form mode",
            path.display()
        );

        assert!(
            field.location.nested_path.is_empty(),
            "{} sample ClickHere field should be in body text",
            path.display()
        );
        let para = &core.document().sections[field.location.section_index].paragraphs
            [field.location.para_index];
        let range = &para.field_ranges[field.field_range_index];
        let info = core.get_field_info_at(
            field.location.section_index,
            field.location.para_index,
            range.start_char_idx,
        );
        assert!(
            info.contains("\"editableInForm\":true"),
            "field info should expose editableInForm=true for {}: {}",
            path.display(),
            info
        );
    }

    let list_json = core.get_field_list_json();
    assert!(
        list_json.contains("\"editableInForm\":true"),
        "field list should expose editableInForm=true for {}: {}",
        path.display(),
        list_json
    );
    assert!(
        list_json.contains("\"startCharIdx\":"),
        "field list should expose startCharIdx for form navigation: {}",
        list_json
    );
    assert!(
        list_json.contains("\"endCharIdx\":"),
        "field list should expose endCharIdx for form navigation: {}",
        list_json
    );
}

#[test]
fn clickhere_form_editable_attribute_is_preserved_in_hwp_and_hwpx() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_clickhere_form_editable(&repo_root.join("samples/누름틀-2024.hwp"));
    assert_clickhere_form_editable(&repo_root.join("samples/누름틀-2024.hwpx"));
}

#[test]
fn clickhere_insert_api_creates_empty_editable_field() {
    let core = make_doc_with_inserted_clickhere();

    let fields = core.collect_all_fields();
    let click_fields: Vec<_> = fields
        .iter()
        .filter(|f| f.field.field_type == FieldType::ClickHere)
        .collect();
    assert_eq!(click_fields.len(), 1);

    let field = click_fields[0];
    assert_eq!(field.value, "");
    assert_eq!(field.field.guide_text(), Some("입력하세요"));
    assert_eq!(field.field.memo_text(), Some("메모"));
    assert_eq!(field.field.field_name(), Some("name01"));
    assert!(field.field.is_editable_in_form());

    let para = &core.document().sections[0].paragraphs[0];
    let range = &para.field_ranges[field.field_range_index];
    assert_eq!(range.start_char_idx, 1);
    assert_eq!(range.end_char_idx, 1);
    assert_eq!(para.char_offsets, vec![16, 33, 34]);
    assert_eq!(para.char_offsets[1] - (para.char_offsets[0] + 1), 16);

    let info = core.get_field_info_at(0, 0, 1);
    assert!(info.contains(r#""inField":true"#), "{}", info);
    assert!(info.contains(r#""isGuide":true"#), "{}", info);
    assert!(info.contains(r#""editableInForm":true"#), "{}", info);

    let list_json = core.get_field_list_json();
    assert!(
        list_json.contains(r#""guide":"입력하세요""#),
        "{}",
        list_json
    );
    assert!(list_json.contains(r#""name":"name01""#), "{}", list_json);
    assert!(list_json.contains(r#""startCharIdx":1"#), "{}", list_json);
    assert!(list_json.contains(r#""endCharIdx":1"#), "{}", list_json);
}

#[test]
fn clickhere_end_boundary_insert_respects_active_field_state() {
    let mut core = make_doc_with_inserted_clickhere();

    core.insert_text_native(0, 0, 1, "값")
        .expect("first input should fill empty clickhere");
    let fields = core.collect_all_fields();
    let field = fields
        .iter()
        .find(|f| f.field.field_type == FieldType::ClickHere)
        .expect("clickhere field");
    let range = &core.document().sections[0].paragraphs[0].field_ranges[field.field_range_index];
    assert_eq!((range.start_char_idx, range.end_char_idx), (1, 2));
    assert_eq!(field.value, "값");

    assert!(
        core.set_active_field(0, 0, 2),
        "field end should be an editable active boundary"
    );
    core.insert_text_native(0, 0, 2, "1")
        .expect("active field end should append to clickhere value");
    let fields = core.collect_all_fields();
    let field = fields
        .iter()
        .find(|f| f.field.field_type == FieldType::ClickHere)
        .expect("clickhere field after active append");
    let range = &core.document().sections[0].paragraphs[0].field_ranges[field.field_range_index];
    assert_eq!((range.start_char_idx, range.end_char_idx), (1, 3));
    assert_eq!(field.value, "값1");

    core.clear_active_field();
    core.insert_text_native(0, 0, 3, "밖")
        .expect("inactive field end should insert outside clickhere");
    let fields = core.collect_all_fields();
    let field = fields
        .iter()
        .find(|f| f.field.field_type == FieldType::ClickHere)
        .expect("clickhere field after outside insert");
    let para = &core.document().sections[0].paragraphs[0];
    let range = &para.field_ranges[field.field_range_index];
    assert_eq!((range.start_char_idx, range.end_char_idx), (1, 3));
    assert_eq!(field.value, "값1");
    assert_eq!(para.text, "A값1밖BC");
}

#[test]
fn inserted_clickhere_roundtrips_hwp_and_hwpx() {
    let core = make_doc_with_inserted_clickhere();

    let hwp = core.export_hwp_native().expect("export hwp");
    let reparsed_hwp = DocumentCore::from_bytes(&hwp).expect("reparse exported hwp");
    assert_inserted_clickhere_roundtrip(&reparsed_hwp, "HWP");

    let hwpx = core.export_hwpx_native().expect("export hwpx");
    let reparsed_hwpx = DocumentCore::from_bytes(&hwpx).expect("reparse exported hwpx");
    assert_inserted_clickhere_roundtrip(&reparsed_hwpx, "HWPX");
}

fn assert_inserted_clickhere_roundtrip(core: &DocumentCore, label: &str) {
    let fields = core.collect_all_fields();
    let click_fields: Vec<_> = fields
        .iter()
        .filter(|f| f.field.field_type == FieldType::ClickHere)
        .collect();
    assert_eq!(click_fields.len(), 1, "{} ClickHere count", label);

    let field = click_fields[0];
    assert_eq!(field.value, "", "{} value", label);
    assert_eq!(
        field.field.guide_text(),
        Some("입력하세요"),
        "{} guide",
        label
    );
    assert_eq!(field.field.memo_text(), Some("메모"), "{} memo", label);
    assert_eq!(field.field.field_name(), Some("name01"), "{} name", label);
    assert!(
        field.field.is_editable_in_form(),
        "{} editableInForm",
        label
    );
}
