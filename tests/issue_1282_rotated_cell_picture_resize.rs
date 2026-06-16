use rhwp::document_core::DocumentCore;
use rhwp::model::control::Control;
use rhwp::model::document::Document;
use rhwp::model::image::Picture;
use rhwp::model::table::{Cell, Table};
use rhwp::parser::parse_document;

fn read_fixture(path: &str) -> Vec<u8> {
    std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
        .unwrap_or_else(|e| panic!("read {path}: {e}"))
}

fn target_table(doc: &Document) -> &Table {
    match &doc.sections[0].paragraphs[0].controls[2] {
        Control::Table(table) => table,
        other => panic!("expected target table at para 0 control 2, got {other:?}"),
    }
}

fn target_cell(doc: &Document) -> &Cell {
    &target_table(doc).cells[2]
}

fn target_picture(doc: &Document) -> &Picture {
    let cell = target_cell(doc);
    match &cell.paragraphs[0].controls[0] {
        Control::Picture(pic) => pic,
        other => panic!("expected target cell picture, got {other:?}"),
    }
}

fn required_cell_height_for_picture(cell: &Cell, pic: &Picture) -> u32 {
    pic.common
        .vertical_offset
        .saturating_add(pic.common.height)
        .saturating_add(cell.padding.top as u32)
        .saturating_add(cell.padding.bottom as u32)
}

#[test]
fn issue_1282_resizing_rotated_cell_picture_grows_owner_cell_height() {
    let bytes = read_fixture("samples/ta-pic-001-r.hwp");
    let mut core = DocumentCore::from_bytes(&bytes).expect("load HWP fixture");

    core.set_cell_picture_properties_by_path_native(
        0,
        0,
        r#"[{"controlIdx":2,"cellIdx":2,"cellParaIdx":0}]"#,
        0,
        r#"{"height":30000}"#,
    )
    .expect("resize rotated cell picture");

    let doc = core.document();
    let table = target_table(doc);
    let cell = target_cell(doc);
    let pic = target_picture(doc);
    let required_height = required_cell_height_for_picture(cell, pic);

    assert!(
        cell.height >= required_height,
        "owner cell must grow to contain resized rotated picture: cell.height={}, required={}, pic.vertOffset={}, pic.height={}, pad=({}, {})",
        cell.height,
        required_height,
        pic.common.vertical_offset,
        pic.common.height,
        cell.padding.top,
        cell.padding.bottom
    );
    assert!(
        table.common.height >= cell.height,
        "table common height must follow grown cell height: table.common.height={}, cell.height={}",
        table.common.height,
        cell.height
    );

    let exported = core.export_hwp_native().expect("export edited HWP");
    let reparsed = parse_document(&exported).expect("reparse edited HWP");
    let reparsed_table = target_table(&reparsed);
    let reparsed_cell = target_cell(&reparsed);
    let reparsed_pic = target_picture(&reparsed);
    let reparsed_required = required_cell_height_for_picture(reparsed_cell, reparsed_pic);

    assert!(
        reparsed_cell.height >= reparsed_required,
        "exported HWP must preserve grown owner cell height: cell.height={}, required={}",
        reparsed_cell.height,
        reparsed_required
    );
    assert!(
        reparsed_table.common.height >= reparsed_cell.height,
        "exported HWP table height must preserve grown cell height"
    );
}
