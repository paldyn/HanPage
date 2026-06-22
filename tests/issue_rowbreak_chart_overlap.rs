//! Regression guard for `samples/rowbreak-problem-pages.hwpx` page 2.
//!
//! The first chart-like TAC table on page 2 (`pi=5 ci=0`) must start below the
//! preceding `<민간 SaaS 연계공통기반 운영체계>` title line. Otherwise the chart
//! border and image are painted under that title text.

use rhwp::renderer::render_tree::{BoundingBox, RenderNode, RenderNodeType};
use std::fs;
use std::path::Path;

const SAMPLE: &str = "samples/rowbreak-problem-pages.hwpx";
const PAGE_INDEX: u32 = 1;

fn find_table_bbox(root: &RenderNode, target_pi: usize, target_ci: usize) -> Option<BoundingBox> {
    if let RenderNodeType::Table(t) = &root.node_type {
        if t.para_index == Some(target_pi) && t.control_index == Some(target_ci) {
            return Some(root.bbox);
        }
    }

    root.children
        .iter()
        .find_map(|child| find_table_bbox(child, target_pi, target_ci))
}

fn find_textrun_bbox_containing(root: &RenderNode, needle: &str) -> Option<BoundingBox> {
    if let RenderNodeType::TextRun(run) = &root.node_type {
        if run.text.contains(needle) {
            return Some(root.bbox);
        }
    }

    root.children
        .iter()
        .find_map(|child| find_textrun_bbox_containing(child, needle))
}

#[test]
fn rowbreak_page2_chart_starts_below_title_line() {
    let repo_root = env!("CARGO_MANIFEST_DIR");
    let sample_path = Path::new(repo_root).join(SAMPLE);
    let bytes = fs::read(&sample_path).unwrap_or_else(|e| panic!("read {}: {}", SAMPLE, e));
    let doc = rhwp::wasm_api::HwpDocument::from_bytes(&bytes)
        .unwrap_or_else(|e| panic!("parse {}: {:?}", SAMPLE, e));
    let tree = doc
        .build_page_render_tree(PAGE_INDEX)
        .unwrap_or_else(|e| panic!("render page {}: {}", PAGE_INDEX + 1, e));

    let chart =
        find_table_bbox(&tree.root, 5, 0).expect("page 2 chart table pi=5 ci=0 should render");
    let title = find_textrun_bbox_containing(&tree.root, "연계공통기반 운영체계")
        .expect("page 2 chart title text should render");

    let title_bottom = title.y + title.height;
    assert!(
        chart.y >= title_bottom - 0.5,
        "page 2 chart overlaps title text: title=[{:.2}..{:.2}], chart_y={:.2}",
        title.y,
        title_bottom,
        chart.y,
    );
}
