//! Issue #1440: 온새미로 35쪽 그림 어울림 본문 줄이 그림 영역을 침범하는 회귀 방지.

use rhwp::model::control::Control;
use rhwp::model::paragraph::Paragraph;
use rhwp::model::shape::{ShapeObject, TextWrap};
use rhwp::renderer::render_tree::{BoundingBox, RenderNode, RenderNodeType};
use std::fs;
use std::path::Path;

const SAMPLES: &[&str] = &[
    "samples/[2027] 온새미로 1 본교재.hwp",
    "samples/[2027] 온새미로 1 본교재.hwpx",
];
const TARGET_PAGE: u32 = 34; // 35쪽, 0-based
const TARGET_PARA: usize = 8;

fn read_fixture(path: &str) -> Vec<u8> {
    fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
        .unwrap_or_else(|e| panic!("read {path}: {e}"))
}

fn collect_nodes<'a>(node: &'a RenderNode, out: &mut Vec<&'a RenderNode>) {
    out.push(node);
    for child in &node.children {
        collect_nodes(child, out);
    }
}

fn vertically_overlaps(a: &BoundingBox, b: &BoundingBox) -> bool {
    a.y < b.y + b.height && a.y + a.height > b.y
}

fn horizontally_overlaps(a: &BoundingBox, b: &BoundingBox) -> bool {
    a.x < b.x + b.width && a.x + a.width > b.x
}

fn control_is_square_picture(ctrl: &Control) -> bool {
    match ctrl {
        Control::Picture(pic) => {
            !pic.common.treat_as_char && matches!(pic.common.text_wrap, TextWrap::Square)
        }
        Control::Shape(shape) => match shape.as_ref() {
            ShapeObject::Picture(pic) => {
                !pic.common.treat_as_char && matches!(pic.common.text_wrap, TextWrap::Square)
            }
            other => {
                !other.common().treat_as_char
                    && matches!(other.common().text_wrap, TextWrap::Square)
            }
        },
        _ => false,
    }
}

fn collect_source_paragraphs<'a>(
    paragraphs: &'a [Paragraph],
    path: &str,
    out: &mut Vec<(String, &'a Paragraph)>,
) {
    for (pi, para) in paragraphs.iter().enumerate() {
        let para_path = format!("{path}/p{pi}");
        out.push((para_path.clone(), para));

        for (ci, ctrl) in para.controls.iter().enumerate() {
            match ctrl {
                Control::Table(table) => {
                    for (cell_idx, cell) in table.cells.iter().enumerate() {
                        collect_source_paragraphs(
                            &cell.paragraphs,
                            &format!("{para_path}/c{ci}/cell{cell_idx}"),
                            out,
                        );
                    }
                    if let Some(caption) = &table.caption {
                        collect_source_paragraphs(
                            &caption.paragraphs,
                            &format!("{para_path}/c{ci}/table_caption"),
                            out,
                        );
                    }
                }
                Control::Picture(pic) => {
                    if let Some(caption) = &pic.caption {
                        collect_source_paragraphs(
                            &caption.paragraphs,
                            &format!("{para_path}/c{ci}/picture_caption"),
                            out,
                        );
                    }
                }
                Control::Shape(shape) => {
                    if let Some(drawing) = shape.drawing() {
                        if let Some(text_box) = &drawing.text_box {
                            collect_source_paragraphs(
                                &text_box.paragraphs,
                                &format!("{para_path}/c{ci}/shape_text"),
                                out,
                            );
                        }
                        if let Some(caption) = &drawing.caption {
                            collect_source_paragraphs(
                                &caption.paragraphs,
                                &format!("{para_path}/c{ci}/shape_caption"),
                                out,
                            );
                        }
                    }
                    if let ShapeObject::Picture(pic) = shape.as_ref() {
                        if let Some(caption) = &pic.caption {
                            collect_source_paragraphs(
                                &caption.paragraphs,
                                &format!("{para_path}/c{ci}/shape_picture_caption"),
                                out,
                            );
                        }
                    }
                    if let ShapeObject::Group(group) = shape.as_ref() {
                        for (child_idx, child) in group.children.iter().enumerate() {
                            if let Some(drawing) = child.drawing() {
                                if let Some(text_box) = &drawing.text_box {
                                    collect_source_paragraphs(
                                        &text_box.paragraphs,
                                        &format!("{para_path}/c{ci}/group{child_idx}_text"),
                                        out,
                                    );
                                }
                                if let Some(caption) = &drawing.caption {
                                    collect_source_paragraphs(
                                        &caption.paragraphs,
                                        &format!("{para_path}/c{ci}/group{child_idx}_caption"),
                                        out,
                                    );
                                }
                            }
                        }
                    }
                }
                Control::HiddenComment(comment) => {
                    collect_source_paragraphs(
                        &comment.paragraphs,
                        &format!("{para_path}/c{ci}/hidden_comment"),
                        out,
                    );
                }
                Control::Field(field) => {
                    collect_source_paragraphs(
                        &field.memo_paragraphs,
                        &format!("{para_path}/c{ci}/field_memo"),
                        out,
                    );
                }
                _ => {}
            }
        }
    }
}

fn all_source_paragraphs(doc: &rhwp::wasm_api::HwpDocument) -> Vec<(String, &Paragraph)> {
    let mut out = Vec::new();
    for (si, section) in doc.document().sections.iter().enumerate() {
        collect_source_paragraphs(&section.paragraphs, &format!("s{si}"), &mut out);
    }
    out
}

#[test]
fn issue_1440_page35_text_lines_do_not_cross_square_picture() {
    for sample in SAMPLES {
        let bytes = read_fixture(sample);
        let doc = rhwp::wasm_api::HwpDocument::from_bytes(&bytes)
            .unwrap_or_else(|e| panic!("parse {sample}: {e}"));
        let tree = doc
            .build_page_render_tree(TARGET_PAGE)
            .expect("build page 35 render tree");

        let mut nodes = Vec::new();
        collect_nodes(&tree.root, &mut nodes);

        let target_image = nodes
            .iter()
            .filter(|node| matches!(node.node_type, RenderNodeType::Image(_)))
            .map(|node| &node.bbox)
            .filter(|bbox| bbox.width > 150.0 && bbox.height > 120.0)
            .max_by(|a, b| {
                (a.width * a.height)
                    .partial_cmp(&(b.width * b.height))
                    .unwrap()
            })
            .expect("35쪽 대상 어울림 그림 bbox");

        let mut offenders = Vec::new();
        for node in nodes {
            let RenderNodeType::TextLine(line) = &node.node_type else {
                continue;
            };
            if line.para_index != Some(TARGET_PARA) {
                continue;
            }
            if vertically_overlaps(&node.bbox, target_image)
                && horizontally_overlaps(&node.bbox, target_image)
            {
                offenders.push((
                    line.line_index.unwrap_or(u32::MAX),
                    node.bbox.x,
                    node.bbox.y,
                    node.bbox.width,
                    node.bbox.height,
                ));
            }
        }

        assert!(
            offenders.is_empty(),
            "{sample}: 35쪽 pi={TARGET_PARA} 본문 줄이 그림 bbox를 침범함: image=[x={:.1} y={:.1} w={:.1} h={:.1}], offenders={:?}",
            target_image.x,
            target_image.y,
            target_image.width,
            target_image.height,
            offenders
        );
    }
}

#[test]
fn issue_1440_source_linesegs_encode_wrap_zone_for_target_paragraph() {
    for sample in SAMPLES {
        let bytes = read_fixture(sample);
        let doc = rhwp::wasm_api::HwpDocument::from_bytes(&bytes)
            .unwrap_or_else(|e| panic!("parse {sample}: {e}"));
        let paragraphs = all_source_paragraphs(&doc);
        let picture_hosts: Vec<_> = paragraphs
            .iter()
            .filter(|(_, para)| para.controls.iter().any(control_is_square_picture))
            .collect();
        assert!(
            !picture_hosts.is_empty(),
            "{sample}: square-wrap picture host paragraph"
        );

        let wrap_text_paras: Vec<_> = paragraphs
            .iter()
            .filter(|(_, para)| {
                para.line_segs.iter().any(|seg| {
                    seg.column_start > 0 || (seg.segment_width > 0 && seg.segment_width < 30_000)
                })
            })
            .collect();
        assert!(
            !wrap_text_paras.is_empty(),
            "{sample}: source should carry precomputed wrap-zone line segments"
        );

        let target_text_para = paragraphs
            .iter()
            .find(|(path, para)| path == "s3/p8" && para.text.contains("기차 안에서처럼"))
            .expect("35쪽 대상 본문 문단 s3/p8");
        assert!(
            target_text_para
                .1
                .line_segs
                .iter()
                .take(7)
                .all(|seg| seg.column_start == 850 && seg.segment_width == 20_999),
            "{sample}: 35쪽 대상 본문 첫 7줄은 그림 왼쪽 wrap-zone LineSeg여야 함"
        );
    }
}
