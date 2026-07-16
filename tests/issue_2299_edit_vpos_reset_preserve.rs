//! Issue #2299 — 편집발 vpos 재계산이 저장 단/쪽-상대 vpos 리셋(단 경계 인코딩)을
//! 파괴해 다단 단-밴드가 소멸하던 회귀 핀.
//!
//! shortcut.hwp(1단 제목 + 2단 배분 zone 이 카테고리마다 반복, 저장 리셋 76곳 전부
//! vpos=0)의 앞 문단을 편집하면 `recalculate_section_vpos` 의 선형 누적이 하류 리셋
//! 전부를 덮어써 typeset(#321/#470/#702)·pagination 의 단/쪽 진행 신호가 소멸 —
//! 7쪽(한글 2022 정합)이 9쪽으로, 0쪽 Column 이 col=[0]만으로 붕괴하던 결함.
//! 편집 문단의 높이가 변하지 않는 편집(1자 삽입)에서도 발생한다.
//!
//! 수정: 직전 문단의 "이동 전(저장)" end 대비 현재 문단의 저장 first 가 감소하면
//! 단/쪽 경계 인코딩으로 보고 next_vpos 를 저장 first 로 되돌려 보존.
//! 근거·임계 불요 사유는 `recalculate_section_vpos` doc comment 참조.

use rhwp::renderer::render_tree::{RenderNode, RenderNodeType};
use rhwp::wasm_api::HwpDocument;

fn load_shortcut() -> HwpDocument {
    let bytes = std::fs::read("samples/basic/shortcut.hwp").expect("read shortcut.hwp");
    HwpDocument::from_bytes(&bytes).expect("parse shortcut.hwp")
}

fn collect_cols(node: &RenderNode, out: &mut Vec<u16>) {
    if let RenderNodeType::Column(c) = node.node_type {
        out.push(c);
    }
    for child in &node.children {
        collect_cols(child, out);
    }
}

/// 0쪽 렌더트리의 Column 노드 col 인덱스 나열 (문서 순서).
fn page0_cols(doc: &HwpDocument) -> Vec<u16> {
    let tree = doc.build_page_render_tree(0).expect("page 0 render tree");
    let mut cols = Vec::new();
    collect_cols(&tree.root, &mut cols);
    cols
}

#[test]
fn front_paragraph_edit_preserves_column_bands_and_page_count() {
    let mut doc = load_shortcut();
    assert_eq!(doc.page_count(), 7, "원본 쪽수 전제 (한글 2022 = 7쪽)");
    let cols_before = page0_cols(&doc);
    assert!(
        cols_before.contains(&1),
        "원본 0쪽에 우측 단(col=1) 밴드 존재 전제: {cols_before:?}"
    );

    doc.insert_text_native(0, 0, 0, "X").expect("insert");

    assert_eq!(
        doc.page_count(),
        7,
        "앞 문단 1자 삽입 후에도 7쪽 유지 — 9쪽이면 저장 vpos 리셋 파괴(#2299) 회귀"
    );
    assert_eq!(
        page0_cols(&doc),
        cols_before,
        "편집 후 0쪽 단-밴드 배치 보존 — col=1 소멸이면 #2299 회귀"
    );
}

#[test]
fn delete_edit_preserves_layout() {
    let mut doc = load_shortcut();
    doc.delete_text_native(0, 2, 0, 1).expect("delete");
    assert_eq!(doc.page_count(), 7, "삭제 편집 후에도 7쪽 유지");
    assert!(
        page0_cols(&doc).contains(&1),
        "삭제 편집 후에도 우측 단(col=1) 밴드 보존"
    );
}

#[test]
fn editing_reset_paragraph_itself_preserves_layout() {
    // pi=16 은 0쪽 2단 zone 의 col1 첫 문단(저장 vpos=0 리셋 문단).
    // 리셋 문단 자체를 편집해도 reflow 가 첫 LineSeg vpos 를 보존하고
    // 재계산이 그 리셋을 유지해야 한다.
    let mut doc = load_shortcut();
    doc.insert_text_native(0, 16, 0, "X")
        .expect("insert at reset para");
    assert_eq!(doc.page_count(), 7, "리셋 문단 자체 편집 후에도 7쪽 유지");
    assert!(
        page0_cols(&doc).contains(&1),
        "리셋 문단 자체 편집 후에도 col=1 밴드 보존"
    );
}

#[test]
fn last_paragraph_edit_stays_stable() {
    // 마지막 문단은 아래 재계산 대상이 없어 수정 전에도 7쪽이 유지되던 케이스 —
    // 보존 로직 추가 후에도 불변임을 고정한다.
    let mut doc = load_shortcut();
    let last = doc.document().sections[0].paragraphs.len() - 1;
    doc.insert_text_native(0, last, 0, "X")
        .expect("insert at last para");
    assert_eq!(doc.page_count(), 7, "끝 문단 편집은 7쪽 불변");
}
