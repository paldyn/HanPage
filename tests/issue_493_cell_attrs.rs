//! Issue #493: 셀 보호, 셀 필드 이름, 양식 모드 편집 가능 속성 회귀 가드.

use std::fs;
use std::io::Read;
use std::path::Path;

use rhwp::model::control::Control;
use rhwp::model::document::Document;
use rhwp::parser::hwpx::parse_hwpx;
use rhwp::serializer::hwpx::serialize_hwpx;
use rhwp::{parse_document, wasm_api::HwpDocument};
use serde_json::Value;

#[derive(Clone, Copy)]
struct TablePos {
    section: usize,
    para: usize,
    control: usize,
}

fn sample_bytes(rel: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read(&path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e))
}

fn find_first_table(doc: &Document) -> TablePos {
    for (section, section_model) in doc.sections.iter().enumerate() {
        for (para, paragraph) in section_model.paragraphs.iter().enumerate() {
            for (control, ctrl) in paragraph.controls.iter().enumerate() {
                if matches!(ctrl, Control::Table(_)) {
                    return TablePos {
                        section,
                        para,
                        control,
                    };
                }
            }
        }
    }
    panic!("sample should contain a table");
}

fn assert_cell_attrs(doc: &Document, pos: TablePos) {
    let Control::Table(table) =
        &doc.sections[pos.section].paragraphs[pos.para].controls[pos.control]
    else {
        panic!("expected table control");
    };
    assert!(table.cells.len() >= 3, "sample should contain >= 3 cells");

    assert!(table.cells[0].cell_protect(), "0번 셀은 보호 상태");
    assert!(table.cells[1].cell_protect(), "1번 셀은 보호 상태");
    assert!(table.cells[2].cell_protect(), "2번 셀은 보호 상태");

    assert_eq!(table.cells[2].field_name.as_deref(), Some("name"));
    assert!(
        table.cells[2].editable_in_form(),
        "필드 셀은 양식 모드 편집 가능"
    );
}

fn assert_api_attrs(bytes: &[u8], pos: TablePos) {
    let doc = HwpDocument::from_bytes(bytes).expect("load HwpDocument");
    let json = doc
        .get_cell_properties(pos.section as u32, pos.para as u32, pos.control as u32, 2)
        .expect("getCellProperties");
    let props: Value = serde_json::from_str(&json).expect("parse cell properties");
    assert_eq!(props["cellProtect"].as_bool(), Some(true), "{json}");
    assert_eq!(props["fieldName"].as_str(), Some("name"), "{json}");
    assert_eq!(props["editableInForm"].as_bool(), Some(true), "{json}");

    let fields: Value = serde_json::from_str(&doc.get_field_list()).expect("parse getFieldList");
    let field = fields
        .as_array()
        .expect("field list array")
        .iter()
        .find(|field| field["name"].as_str() == Some("name"))
        .expect("cell field in getFieldList");
    assert_eq!(field["value"].as_str(), Some("12334"), "{fields}");
    assert_eq!(field["editableInForm"].as_bool(), Some(true), "{fields}");
}

fn hwpx_section0_xml(bytes: &[u8]) -> String {
    let reader = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(reader).expect("open hwpx zip");
    for index in 0..zip.len() {
        let mut file = zip.by_index(index).expect("zip entry");
        if file.name().contains("section0.xml") {
            let mut xml = String::new();
            file.read_to_string(&mut xml).expect("read section0.xml");
            return xml;
        }
    }
    panic!("section0.xml not found");
}

fn named_cell_opening_tag(xml: &str) -> &str {
    let start = xml
        .find(r#"<hp:tc name="name""#)
        .expect("named cell opening tag");
    let end = xml[start..].find('>').expect("opening tag end") + start;
    &xml[start..=end]
}

#[test]
fn cell_protect_field_name_and_form_editable_are_parsed_from_hwp_and_hwpx() {
    for rel in ["samples/셀보호.hwp", "samples/셀보호.hwpx"] {
        let bytes = sample_bytes(rel);
        let doc = parse_document(&bytes).unwrap_or_else(|e| panic!("parse {rel}: {e:?}"));
        let pos = find_first_table(&doc);
        assert_cell_attrs(&doc, pos);
        assert_api_attrs(&bytes, pos);
    }
}

#[test]
fn cell_protect_and_form_editable_survive_hwpx_roundtrip() {
    let bytes = sample_bytes("samples/셀보호.hwpx");
    let doc = parse_hwpx(&bytes).expect("parse 셀보호.hwpx");
    let pos = find_first_table(&doc);
    assert_cell_attrs(&doc, pos);

    let serialized = serialize_hwpx(&doc).expect("serialize hwpx");
    let xml = hwpx_section0_xml(&serialized);
    assert_eq!(
        xml.matches(r#"protect="1""#).count(),
        3,
        "serialized section0.xml should keep three protected cells"
    );
    assert_eq!(
        xml.matches(r#"editable="1""#).count(),
        1,
        "serialized section0.xml should keep one form-editable cell"
    );
    let named_cell = named_cell_opening_tag(&xml);
    assert!(
        named_cell.contains(r#"protect="1""#) && named_cell.contains(r#"editable="1""#),
        "serialized named cell should keep protect/editable attrs: {named_cell}"
    );

    let reparsed = parse_hwpx(&serialized).expect("reparse serialized hwpx");
    let pos2 = find_first_table(&reparsed);
    assert_cell_attrs(&reparsed, pos2);
}
