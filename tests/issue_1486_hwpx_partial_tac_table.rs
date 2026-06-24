//! Issue #1486: HWPX 분할 표 내부 TAC 중첩 표가 오른쪽 밖으로 밀리는 회귀 방지.

use std::fs;
use std::path::Path;

use rhwp::renderer::render_tree::{BoundingBox, RenderNode, RenderNodeType};

const SAMPLE: &str = "samples/hwpx_sample2.hwpx";
const TARGET_PAGE: u32 = 8; // 9쪽, 0-based

fn find_body_bbox(node: &RenderNode) -> Option<BoundingBox> {
    if matches!(node.node_type, RenderNodeType::Body { .. }) {
        return Some(node.bbox);
    }

    node.children.iter().find_map(find_body_bbox)
}

fn collect_issue_1486_tables<'a>(node: &'a RenderNode, out: &mut Vec<&'a RenderNode>) {
    if let RenderNodeType::Table(table) = &node.node_type {
        let b = &node.bbox;
        if table.para_index.is_none()
            && table.control_index.is_none()
            && b.y < 220.0
            && b.width > 600.0
            && b.width < 680.0
            && b.height > 100.0
            && b.height < 220.0
        {
            out.push(node);
        }
    }

    for child in &node.children {
        collect_issue_1486_tables(child, out);
    }
}

fn render_tree_contains_text(node: &RenderNode, needle: &str) -> bool {
    if let RenderNodeType::TextRun(run) = &node.node_type {
        if run.text.contains(needle) {
            return true;
        }
    }

    node.children
        .iter()
        .any(|child| render_tree_contains_text(child, needle))
}

#[test]
fn issue_1486_partial_table_tac_nested_table_stays_inside_page_body() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(SAMPLE);
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {SAMPLE}: {e}"));
    let doc = rhwp::wasm_api::HwpDocument::from_bytes(&bytes)
        .unwrap_or_else(|e| panic!("parse {SAMPLE}: {e}"));

    let tree = doc
        .build_page_render_tree(TARGET_PAGE)
        .unwrap_or_else(|e| panic!("render {SAMPLE} page {}: {e}", TARGET_PAGE + 1));

    let body = find_body_bbox(&tree.root).expect("Body bbox");
    let page_right = tree.root.bbox.x + tree.root.bbox.width;
    let expected_body_right = page_right - body.x;

    let mut candidates = Vec::new();
    collect_issue_1486_tables(&tree.root, &mut candidates);
    assert!(
        !candidates.is_empty(),
        "9쪽 상단의 문제 TAC 중첩 표를 찾지 못함"
    );

    let table = candidates
        .into_iter()
        .min_by(|a, b| a.bbox.y.partial_cmp(&b.bbox.y).unwrap())
        .expect("candidate table");
    let table_right = table.bbox.x + table.bbox.width;

    eprintln!(
        "[issue_1486] page={} body_x={:.2} page_right={:.2} table=[x={:.2} y={:.2} w={:.2} h={:.2}] right={:.2} expected_body_right={:.2}",
        TARGET_PAGE + 1,
        body.x,
        page_right,
        table.bbox.x,
        table.bbox.y,
        table.bbox.width,
        table.bbox.height,
        table_right,
        expected_body_right,
    );

    assert!(
        table.bbox.x < body.x + 120.0,
        "분할 표 내부 TAC 중첩 표가 본문 좌측에서 과도하게 밀림: table_x={:.2}, body_x={:.2}",
        table.bbox.x,
        body.x,
    );
    assert!(
        table_right <= expected_body_right + 1.0,
        "분할 표 내부 TAC 중첩 표가 페이지 본문 오른쪽을 초과함: table_right={:.2}, expected_body_right={:.2}",
        table_right,
        expected_body_right,
    );
}

#[test]
fn issue_1486_terminal_rowbreak_sliver_does_not_push_pdf_page22_content() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(SAMPLE);
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {SAMPLE}: {e}"));
    let doc = rhwp::wasm_api::HwpDocument::from_bytes(&bytes)
        .unwrap_or_else(|e| panic!("parse {SAMPLE}: {e}"));

    let page22 = doc
        .build_page_render_tree(21)
        .expect("render issue #1486 page 22");
    let page23 = doc
        .build_page_render_tree(22)
        .expect("render issue #1486 page 23");

    assert!(
        render_tree_contains_text(&page22.root, "lisfranc"),
        "한컴 PDF 기준 22쪽 하단의 lisfranc 줄이 rhwp 22쪽에 있어야 함"
    );
    assert!(
        !render_tree_contains_text(&page23.root, "lisfranc"),
        "무가시 RowBreak terminal sliver 때문에 lisfranc 줄이 23쪽으로 밀리면 안 됨"
    );
}
