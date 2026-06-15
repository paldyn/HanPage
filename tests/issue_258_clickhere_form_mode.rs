//! Issue #258: 누름틀 양식 모드 편집 가능 속성 회귀 가드.

use std::fs;
use std::path::Path;

use rhwp::document_core::DocumentCore;
use rhwp::model::control::FieldType;

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
}

#[test]
fn clickhere_form_editable_attribute_is_preserved_in_hwp_and_hwpx() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_clickhere_form_editable(&repo_root.join("samples/누름틀-2024.hwp"));
    assert_clickhere_form_editable(&repo_root.join("samples/누름틀-2024.hwpx"));
}
