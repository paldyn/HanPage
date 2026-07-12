//! Issue #2214 Stage 2 contracts.
//!
//! Production 수정 전에 warm layout-cache RED와 cell-flow 경계를 고정한다.

use std::fs;
use std::path::Path;

use rhwp::document_core::DocumentCore;
use rhwp::model::control::Control;
use rhwp::model::paragraph::Paragraph;
use rhwp::renderer::render_tree::{RenderNode, RenderNodeType};
use rhwp::wasm_api::HwpDocument;
use serde::Deserialize;

const HWP_SAMPLE: &str = "samples/issue1949_giant_cell_nested_tables_perf.hwp";
const HWPX_SAMPLE: &str = "samples/issue1949_giant_cell_nested_tables_perf.hwpx";

const SECTION: usize = 0;
const PARENT_PARAGRAPH: usize = 0;
const TABLE_CONTROL: usize = 2;
const CELL: usize = 2;
const TARGET_PARAGRAPH: usize = 5;
const INSERT_OFFSET: usize = 130;
const CELL_PATH: &str = r#"[{"controlIndex":2,"cellIndex":2,"cellParaIndex":5}]"#;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursorRect {
    page_index: u32,
    x: f64,
    y: f64,
    height: f64,
    cell_bounds: Bounds,
    cell_overflowed: bool,
}

#[derive(Debug, Deserialize)]
struct Bounds {
    h: f64,
}

fn load_sample(relative_path: &str) -> HwpDocument {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {relative_path}: {e}"));
    HwpDocument::from_bytes(&bytes).unwrap_or_else(|e| panic!("parse {relative_path}: {e}"))
}

fn target_paragraph(core: &DocumentCore) -> &Paragraph {
    match &core.document().sections[SECTION].paragraphs[PARENT_PARAGRAPH].controls[TABLE_CONTROL] {
        Control::Table(table) => &table.cells[CELL].paragraphs[TARGET_PARAGRAPH],
        other => panic!("target control is not a table: {other:?}"),
    }
}

fn next_paragraph_vpos(core: &DocumentCore) -> i32 {
    match &core.document().sections[SECTION].paragraphs[PARENT_PARAGRAPH].controls[TABLE_CONTROL] {
        Control::Table(table) => {
            table.cells[CELL].paragraphs[TARGET_PARAGRAPH + 1]
                .line_segs
                .first()
                .expect("target next paragraph line")
                .vertical_pos
        }
        other => panic!("target control is not a table: {other:?}"),
    }
}

fn line_starts(doc: &HwpDocument) -> Vec<usize> {
    target_paragraph(doc)
        .line_segs
        .iter()
        .map(|seg| seg.text_start as usize)
        .collect()
}

fn relative_flow_advance(para: &Paragraph) -> i64 {
    let first = para.line_segs.first().expect("target first line");
    let last = para.line_segs.last().expect("target last line");
    i64::from(last.vertical_pos) + i64::from(last.line_height) + i64::from(last.line_spacing)
        - i64::from(first.vertical_pos)
}

fn insert_batch(doc: &mut HwpDocument, count: usize) {
    doc.insert_text_in_cell_native_deferred_pagination(
        SECTION,
        PARENT_PARAGRAPH,
        TABLE_CONTROL,
        CELL,
        TARGET_PARAGRAPH,
        INSERT_OFFSET,
        &"1".repeat(count),
    )
    .expect("batch deferred insert");
}

fn insert_sequential(doc: &mut HwpDocument, count: usize) {
    for inserted in 0..count {
        doc.insert_text_in_cell_native_deferred_pagination(
            SECTION,
            PARENT_PARAGRAPH,
            TABLE_CONTROL,
            CELL,
            TARGET_PARAGRAPH,
            INSERT_OFFSET + inserted,
            "1",
        )
        .expect("sequential deferred insert");
    }
}

fn warm_target_layout(doc: &HwpDocument) {
    doc.build_page_render_tree(0)
        .expect("warm target page tree");
    doc.get_cursor_rect_in_cell_native(
        SECTION,
        PARENT_PARAGRAPH,
        TABLE_CONTROL,
        CELL,
        TARGET_PARAGRAPH,
        INSERT_OFFSET,
    )
    .expect("warm target cursor");
}

fn target_cut_fingerprints(doc: &HwpDocument) -> Vec<String> {
    (0..doc.page_count())
        .map(|page| {
            let matches = doc
                .dump_page_items(Some(page))
                .lines()
                .filter(|line| line.contains("PartialTable") && line.contains("pi=0 ci=2  rows="))
                .map(|line| line.trim().to_string())
                .collect::<Vec<_>>();
            assert_eq!(
                matches.len(),
                1,
                "page {page}: exactly one target PartialTable fragment"
            );
            matches[0].clone()
        })
        .collect()
}

fn target_tree_end(doc: &HwpDocument) -> usize {
    fn visit(node: &RenderNode, ranges: &mut Vec<(usize, usize)>) {
        if let RenderNodeType::TextRun(run) = &node.node_type {
            if let (Some(start), Some(ctx)) = (run.char_start, run.cell_context.as_ref()) {
                let exact_path = ctx.parent_para_index == PARENT_PARAGRAPH
                    && ctx.path.len() == 1
                    && ctx.path.first().is_some_and(|entry| {
                        entry.control_index == TABLE_CONTROL
                            && entry.cell_index == CELL
                            && entry.cell_para_index == TARGET_PARAGRAPH
                    });
                if exact_path {
                    assert!(run.char_overlap.is_none(), "target run must not overlap");
                    assert_eq!(
                        run.text.chars().count(),
                        run.text.encode_utf16().count(),
                        "fixture target run must be BMP"
                    );
                    ranges.push((start, start + run.text.encode_utf16().count()));
                }
            }
        }
        for child in &node.children {
            visit(child, ranges);
        }
    }

    let tree = doc
        .build_page_render_tree(0)
        .expect("build target page tree");
    let mut ranges = Vec::new();
    visit(&tree.root, &mut ranges);
    ranges.sort_unstable();
    assert!(!ranges.is_empty(), "target page must have text runs");
    let mut end = 0;
    for (start, next_end) in ranges {
        assert_eq!(start, end, "target ranges must have no gap or overlap");
        assert!(next_end > start, "target run must advance");
        end = next_end;
    }
    end
}

fn path_rect(doc: &HwpDocument, offset: usize) -> CursorRect {
    let raw = doc
        .get_cursor_rect_by_path_near(
            SECTION as u32,
            PARENT_PARAGRAPH as u32,
            CELL_PATH,
            offset as u32,
            0,
        )
        .expect("path-near cursor rect");
    serde_json::from_str(&raw).expect("cursor rect json")
}

fn approx_eq(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 0.2
}

/// Desired public behavior. 현재 warm cache에서는 HWP/HWPX 모두 tree=129와 첫 줄
/// fallback을 반환하므로 명시 실행 시 RED여야 한다. Stage 3 scoped eviction 뒤 GREEN으로
/// 전환하고 `#[ignore]`를 제거한다.
#[test]
#[ignore = "RED contract: warm deferred edit must expose the latest tree and exact cursor"]
fn issue_2214_warm_deferred_tree_and_cursor_are_exact_red() {
    let mut failures = Vec::new();
    for (label, sample) in [("hwp", HWP_SAMPLE), ("hwpx", HWPX_SAMPLE)] {
        let mut doc = load_sample(sample);
        assert_eq!(doc.page_count(), 115, "{label}: initial page count");
        assert_eq!(
            target_paragraph(&doc).text.encode_utf16().count(),
            INSERT_OFFSET
        );
        let original_text = target_paragraph(&doc).text.clone();
        warm_target_layout(&doc);
        let pagination_before = target_cut_fingerprints(&doc);
        insert_sequential(&mut doc, 44);

        let expected_end = INSERT_OFFSET + 44;
        assert_eq!(
            target_paragraph(&doc).text.encode_utf16().count(),
            expected_end
        );
        assert_eq!(
            target_paragraph(&doc).text,
            format!("{original_text}{}", "1".repeat(44)),
            "{label}: deferred edit must append exactly 44 characters"
        );
        assert_eq!(line_starts(&doc), vec![0, 44, 84, 122, 129]);
        assert_eq!(
            doc.page_count(),
            115,
            "{label}: deferred edit must not paginate"
        );
        let pagination_after = target_cut_fingerprints(&doc);
        assert_eq!(
            pagination_after, pagination_before,
            "{label}: deferred edit must preserve all 115 pagination fragments"
        );
        assert!(
            pagination_after[0].contains("end_cut=[37]"),
            "{label}: page zero must retain the pre-flush cut"
        );

        // 실제 Studio 순서처럼 path-near를 첫 observer로 둔다.
        let rect = path_rect(&doc, expected_end);
        assert!(
            approx_eq(rect.cell_bounds.h, 945.9),
            "{label}: deferred edit must retain pre-flush cell bounds: {rect:?}"
        );
        let tree_end = target_tree_end(&doc);
        let exact = tree_end == expected_end
            && rect.page_index == 0
            && approx_eq(rect.x, 569.7)
            && approx_eq(rect.y, 341.1)
            && approx_eq(rect.height, 16.0)
            && !rect.cell_overflowed;
        eprintln!("#2214 {label}: model={expected_end} tree={tree_end} rect={rect:?}");
        if !exact {
            failures.push(format!(
                "{label}: model={expected_end} tree={tree_end} page={} x={:.1} y={:.1} bounds_h={:.1}",
                rect.page_index, rect.x, rect.y, rect.cell_bounds.h
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "warm deferred tree/cursor must be exact:\n{}",
        failures.join("\n")
    );
}

/// Production 신호의 입력 권위값: line-count 자체가 아니라 상대 flow advance 변화다.
#[test]
fn issue_2214_cell_flow_transition_baseline() {
    for (label, sample) in [("hwp", HWP_SAMPLE), ("hwpx", HWPX_SAMPLE)] {
        let mut doc28 = load_sample(sample);
        let original28 = target_paragraph(&doc28).text.clone();
        let initial_advance = relative_flow_advance(target_paragraph(&doc28));
        let initial_next_vpos = next_paragraph_vpos(&doc28);
        insert_batch(&mut doc28, 28);
        assert_eq!(line_starts(&doc28), vec![0, 44, 84, 122]);
        assert_eq!(
            relative_flow_advance(target_paragraph(&doc28)),
            initial_advance,
            "{label}: 28-char edit must not change cell flow"
        );
        assert_eq!(
            next_paragraph_vpos(&doc28),
            initial_next_vpos,
            "{label}: 28-char edit must preserve next paragraph vpos"
        );
        assert_eq!(
            target_paragraph(&doc28).text,
            format!("{original28}{}", "1".repeat(28)),
            "{label}: 28-char batch text"
        );

        let mut doc50 = load_sample(sample);
        let original50 = target_paragraph(&doc50).text.clone();
        let mut changed_inputs = Vec::new();
        for inserted in 0..50 {
            let before = relative_flow_advance(target_paragraph(&doc50));
            let next_vpos_before = next_paragraph_vpos(&doc50);
            doc50
                .insert_text_in_cell_native_deferred_pagination(
                    SECTION,
                    PARENT_PARAGRAPH,
                    TABLE_CONTROL,
                    CELL,
                    TARGET_PARAGRAPH,
                    INSERT_OFFSET + inserted,
                    "1",
                )
                .expect("per-key deferred insert");
            let delta = relative_flow_advance(target_paragraph(&doc50)) - before;
            let expected = if inserted == 43 { 1920 } else { 0 };
            assert_eq!(
                delta,
                expected,
                "{label}: input {} flow delta",
                inserted + 1
            );
            assert_eq!(
                i64::from(next_paragraph_vpos(&doc50) - next_vpos_before),
                expected,
                "{label}: input {} next paragraph vpos delta",
                inserted + 1
            );
            if delta != 0 {
                changed_inputs.push(inserted + 1);
            }
        }
        assert_eq!(
            changed_inputs,
            vec![44],
            "{label}: exactly one flow boundary"
        );
        assert_eq!(line_starts(&doc50), vec![0, 44, 84, 122, 129]);
        assert_eq!(
            target_paragraph(&doc50).text,
            format!("{original50}{}", "1".repeat(50)),
            "{label}: 50-char sequential text"
        );
        assert_eq!(doc28.page_count(), 115, "{label}: 28-char page count");
        assert_eq!(doc50.page_count(), 115, "{label}: 50-char page count");
    }
}
