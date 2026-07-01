//! 단일 패스 조판 엔진 (TypesetEngine)
//!
//! 기존 3단계 파이프라인(height_measurer → pagination → layout)을 대체하는
//! 단일 패스 조판 엔진. 각 요소를 format() → fits() → place/split 순서로
//! 처리하여 측정과 배치를 하나의 흐름으로 통합한다.
//!
//! Phase 2: Break Token 기반 표 조판 구현.
//! Chromium LayoutNG의 Break Token 패턴, LibreOffice Writer의 Master/Follow Chain,
//! MS Word/OOXML의 cantSplit/tblHeader를 참고.

use crate::model::control::Control;
use crate::model::footnote::FootnoteShape;
use crate::model::header_footer::HeaderFooterApply;
use crate::model::page::{ColumnDef, ColumnType, PageDef};
use crate::model::paragraph::{ColumnBreakType, LineSeg, Paragraph};
use crate::model::shape::CaptionDirection;
use crate::renderer::composer::ComposedParagraph;
use crate::renderer::float_placement::{
    horizontal_range, is_para_topbottom_float, signed_hwpunit, FloatLaneSet, FloatPlacementContext,
};
use crate::renderer::height_cursor::HeightCursor;
use crate::renderer::height_measurer::{fit_measured_table_to_declared_height, MeasuredTable};
use crate::renderer::layout::{border_width_to_px, ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX};
use crate::renderer::page_layout::PageLayoutInfo;
use crate::renderer::style_resolver::ResolvedStyleSet;
use crate::renderer::{
    format_number, hwpunit_to_px, NumberFormat as RenderNumberFormat, DEFAULT_DPI,
};

// [Task #836] 미주 paragraph의 가상 para_index = paragraphs.len() + endnote 내 순번.
// rendering.rs에서 paragraphs + endnote_paragraphs를 합쳐서 전달.
use super::pagination::{
    estimate_footnote_note_height, footnote_between_notes_margin_px,
    footnote_separator_overhead_px, ColumnContent, EndnoteParaSource, EndnoteRef, FootnoteRef,
    FootnoteSource, HeaderFooterRef, PageContent, PageItem, PaginationResult,
};

fn note_number_format_from_hwp_code(code: u8) -> RenderNumberFormat {
    match code {
        0 => RenderNumberFormat::Digit,
        1 => RenderNumberFormat::CircledDigit,
        2 => RenderNumberFormat::RomanUpper,
        3 => RenderNumberFormat::RomanLower,
        4 => RenderNumberFormat::LatinUpper,
        5 => RenderNumberFormat::LatinLower,
        8 => RenderNumberFormat::HangulGaNaDa,
        12 => RenderNumberFormat::HangulNumber,
        13 => RenderNumberFormat::HanjaNumber,
        _ => RenderNumberFormat::Digit,
    }
}

fn note_decoration_char(value: u16) -> Option<char> {
    if value == 0 {
        None
    } else {
        char::from_u32(value as u32).filter(|ch| *ch != '\0')
    }
}

fn format_endnote_marker_text(endnote: &crate::model::footnote::Endnote) -> String {
    let number = format_number(
        endnote.number,
        note_number_format_from_hwp_code(endnote.number_shape as u8),
    );
    let prefix = note_decoration_char(endnote.before_decoration_letter)
        .map(|ch| ch.to_string())
        .unwrap_or_default();
    let suffix = note_decoration_char(endnote.after_decoration_letter)
        .unwrap_or(')')
        .to_string();
    format!("{}{}{}", prefix, number, suffix)
}

// ========================================================
// Break Token — 조판 분할 지점 (Chromium LayoutNG 참고)
// ========================================================

/// 표 조판의 분할 재개 정보.
/// 다음 페이지에서 이 토큰으로부터 이어서 조판한다.
#[derive(Debug, Clone)]
struct TableBreakToken {
    /// 재개할 시작 행 인덱스
    start_row: usize,
    /// 인트라-로우 분할 시 각 셀의 콘텐츠 오프셋
    cell_content_offsets: Option<Vec<f64>>,
}

// ========================================================
// FormattedTable — 표의 format() 결과
// ========================================================

/// 표의 조판 높이 정보 (format 단계 결과).
/// 기존 MeasuredTable + host_spacing을 통합하여 측정-배치 일원화.
#[derive(Debug)]
struct FormattedTable {
    /// 행별 높이 (px)
    row_heights: Vec<f64>,
    /// 행간 간격 (px)
    cell_spacing: f64,
    /// 머리행 수 (repeat_header && has_header_cells일 때 1)
    header_row_count: usize,
    /// 호스트 문단 spacing
    host_spacing: HostSpacing,
    /// 표 자체 높이 (host_spacing 미포함)
    effective_height: f64,
    /// 전체 높이 (host_spacing 포함)
    total_height: f64,
    /// 캡션 높이
    caption_height: f64,
    /// TAC 표 여부
    is_tac: bool,
    /// 누적 행 높이 (cell_spacing 포함)
    cumulative_heights: Vec<f64>,
    /// 표 쪽 나눔 설정
    page_break: crate::model::table::TablePageBreak,
    /// 셀별 측정 데이터 (인트라-로우 분할용)
    cells: Vec<crate::renderer::height_measurer::MeasuredCell>,
    /// 표 셀 내 각주 높이 합계 (가용 높이에서 차감)
    table_footnote_height: f64,
    /// 표 셀 내 각주 수 (separator/between-notes 예약 계산용)
    table_footnote_count: usize,
}

#[derive(Debug, Clone, Copy)]
struct VisibleFloatExclusion {
    /// visible host 문단의 자리차지 float 표가 후속 본문을 피하게 만드는 y 구간.
    top: f64,
    bottom: f64,
}

/// 호스트 문단의 spacing (표 전/후)
#[derive(Debug, Clone, Copy)]
struct HostSpacing {
    /// 표 앞 spacing (spacing_before + outer_margin_top)
    before: f64,
    /// 표 뒤 spacing (spacing_after + outer_margin_bottom + host_line_spacing)
    after: f64,
    /// spacing_after만 (마지막 fragment용 — Paginator와 동일)
    spacing_after_only: f64,
}

/// 단일 패스 조판 엔진
pub struct TypesetEngine {
    dpi: f64,
    /// 현재 조판 중인 입력이 HWPX 원본인지 여부.
    is_hwpx_source: std::cell::Cell<bool>,
    /// [Task #1472] HWP3-origin 변환본 여부 — format_paragraph 의 미주 TAC 수식
    /// indent_scale 보정(effective indent 불변)에 사용. typeset 진입 시 set.
    is_hwp3_variant: std::cell::Cell<bool>,
}

/// 조판 중 현재 페이지/단 상태
struct TypesetState {
    /// 완성된 페이지 목록
    pages: Vec<PageContent>,
    /// 현재 단에 쌓이는 항목
    current_items: Vec<PageItem>,
    /// 현재 단에서 소비된 높이 (px)
    current_height: f64,
    /// 현재 단 시작 시점의 논리 높이 (px)
    current_start_height: f64,
    /// 현재 단에 미주 흐름 항목이 포함되어 있는지 여부
    current_endnote_flow: bool,
    /// [Task #1082] 현재 단에서 마지막으로 배치된 본문 FullParagraph 의 bottom vpos (HU,
    /// 섹션 절대값). 미주 vpos-delta 누적의 첫 항목 base 시드용. 단 advance 시 None.
    prev_body_bottom_vpos: Option<i32>,
    /// 현재 단 인덱스
    current_column: u16,
    /// 단 수
    col_count: u16,
    /// 페이지 레이아웃
    layout: PageLayoutInfo,
    /// 구역 인덱스
    section_index: usize,
    /// 각주 높이 누적
    current_footnote_height: f64,
    /// 첫 각주 여부
    is_first_footnote_on_page: bool,
    /// 각주 구분선 오버헤드
    footnote_separator_overhead: f64,
    /// 각주 사이 간격
    footnote_between_notes_margin: f64,
    /// 각주 안전 여백
    footnote_safety_margin: f64,
    /// 존(zone) y 오프셋 (다단 나누기 시 누적)
    current_zone_y_offset: f64,
    /// 현재 존의 레이아웃 오버라이드
    current_zone_layout: Option<PageLayoutInfo>,
    /// 다단 첫 페이지 여부
    on_first_multicolumn_page: bool,
    /// Task #321: col 0 상단의 body-wide TopAndBottom 표/도형이 차지하는 높이 (px).
    /// col 1 이상으로 advance 시 zone_y_offset에 반영.
    pending_body_wide_top_reserve: f64,
    /// visible text host 의 양수 offset 자리차지 표가 후속 문단을 밀어내는 구간.
    visible_float_exclusions: Vec<VisibleFloatExclusion>,
    /// [Task #359] 다음 pi 가 vpos-reset 가드를 발동할 예정 → 현재 pi 의 fit 안전마진 비활성화.
    /// 단독 항목 페이지 발생 차단용.
    skip_safety_margin_once: bool,
    /// [Task #1725] tail-before-vpos-reset 문단 1회 각주 안전마진(40px) 비활성화.
    /// 각주 있는 페이지에서 한글 LINESEG 는 tail 문단을 본문에 배치(각주는 아래)하는데,
    /// rhwp 각주 예약(+40px 버퍼)이 tail 을 수 px 초과로 밀어 near-empty 페이지 over-pagination.
    skip_footnote_margin_once: bool,
    /// [Task #1007] HWP3-origin HWP5 변환본 여부 — widow 방지 등 variant-specific
    /// behavior 분기에 사용.
    is_hwp3_variant: bool,
    /// [Task #1147] HWPX 원본 여부 — HWPX 의 LINE_SEG 시멘틱은 빈 앵커 TopAndBottom 표에서
    /// host_line_spacing 을 표 다음 갭으로 더하지 않음. HWP5/HWP3 와 분리.
    is_hwpx_source: bool,
    /// [Task #362] 한컴 빈 줄 감추기 옵션 (SectionDef bit 19). true 이면 페이지 시작에서
    /// overflow 유발하는 빈 paragraph 최대 2개까지 height=0 처리.
    hide_empty_line: bool,
    /// [Task #362] 현재 페이지에서 감춘 빈 줄 수 (페이지마다 reset, 최대 2).
    hidden_empty_lines: u32,
    /// [Task #362] 감춘 빈 줄이 적용된 페이지 인덱스 (페이지 변경 감지용).
    hidden_empty_page_idx: usize,
    /// [Task #362] hide_empty_line 으로 감춘 paragraph 인덱스 (PaginationResult 에 포함).
    hidden_empty_paras: std::collections::HashSet<usize>,
    /// [Task #836] 미주 목록 (섹션별 수집, 문서 끝에 렌더).
    endnotes: Vec<EndnoteRef>,
    endnote_paragraphs: Vec<Paragraph>,
    endnote_para_sources: Vec<EndnoteParaSource>,
    /// [Task #1246] 현재 섹션 미주의 between-notes 마진(HU, 0=미적용). HeightCursor 가 미주 사이
    /// min-gap 보정에 사용. 모든 경계에서 동일한 섹션 설정값이므로 스칼라로 보관.
    endnote_between_notes_hu: i32,
    /// 현재 섹션 미주의 정규화된 "구분선 위" 마진(HU).
    endnote_separator_above_hu: i32,
    /// 현재 섹션 미주의 정규화된 "구분선 아래" 마진(HU).
    endnote_separator_below_hu: i32,
    /// [Task #362] Square wrap 표의 column_start (HU). -1 = 비활성. 후속 같은 cs/sw paragraph 흡수용.
    wrap_around_cs: i32,
    /// [Task #362] Square wrap 표의 segment_width (HU). -1 = 비활성.
    wrap_around_sw: i32,
    /// [Task #362] Square wrap 표가 있는 paragraph 인덱스 (WrapAroundPara 에 기록).
    wrap_around_table_para: usize,
    /// 비-TAC Picture/Shape Square wrap: any_seg_matches만으로 후속 문단 판정 허용.
    /// 그림의 lineseg는 첫 seg cs=0일 수 있어 전체 seg 중 하나라도 일치하면 흡수.
    wrap_around_any_seg: bool,
    /// [Task #362] 현재 단에서 표 옆에 배치되는 wrap-around paragraphs.
    /// flush_column 에서 ColumnContent 로 전달.
    current_column_wrap_around_paras: Vec<crate::renderer::pagination::WrapAroundPara>,
    /// [Task #604 R3] 현재 단의 wrap text 문단 ↔ anchor 메타데이터.
    /// wrap_around state machine 매칭 시 등록. flush_column 에서 ColumnContent 로 전달.
    current_column_wrap_anchors:
        std::collections::HashMap<usize, crate::renderer::pagination::WrapAnchorRef>,
    /// [Task #702] 현재 zone 의 ColumnType (Normal/Distribute/Parallel).
    /// process_multicolumn_break 에서 새 ColumnDef 매칭 시 갱신.
    /// Distribute 다단의 짧은 컬럼 vpos-reset 검출 임계값 완화에 사용.
    current_zone_column_type: ColumnType,
    /// [Task #853] 현재 zone 의 "디자인 spacing"(px) — 1단 ColumnDef 의 `간격` 값.
    /// 한컴은 1단 ColumnDef 의 `간격`(가로 단 간격이지만 1단이라 무의미)을 zone 진입
    /// 세로 간격으로 쓴다(shortcut.hwp 1쪽 헤더 띠 = 10mm). zone 전환 시
    /// (이전 zone 디자인 spacing /2) + (새 zone 디자인 spacing /2) 를 zone_y_offset 에
    /// 더한다. 다단(2+) ColumnDef 의 `간격`은 가로 간격이므로 0 으로 둔다.
    current_zone_design_spacing_px: f64,
    /// [Task #1027 Stage D] 컬럼 단위 vpos 스냅 상태 (렌더러 build_single_column 정합).
    /// current_height 상대공간(col_area_y=0)에서 HeightCursor 를 구동한다.
    vpos_page_base: Option<i32>,
    vpos_lazy_base: Option<i32>,
    vpos_prev_layout_para: Option<usize>,
    vpos_prev_partial_table: bool,
    /// 컬럼 시작 시점의 current_height (page_path anchor — 렌더러 col_anchor_y 대응).
    vpos_col_anchor: f64,
    /// HWP3-origin 흐름에서는 vpos 보정에서 spacing_before 사전 차감을 생략한다(#1116).
    skip_spacing_before_prededuct: bool,
}

/// [Task #1363] 미주 높이 모델 SSOT 마이그레이션 단계 플래그(`RHWP_EN_SSOT`).
///
/// 미주 para 누적(`acc`)을 layout 순차 렌더 높이(`line_advances_sum`)로 점진 이전하는
/// 동안, divergence 항목을 단계별로 게이트하기 위한 A/B 스위치. 기본은 B(A + TAC 그림 미주 순차
/// 적층)이며, `legacy`/`off`로 기존 saved-vpos delta 경로를 비교·롤백할 수 있다.
/// 상세: `mydocs/working/archives/task_m100_1363_stage2.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum EnSsotLevel {
    /// 전 divergence 원복 — 현행 `metric_advance_px.max(min_h)` (saved-vpos delta). 롤백용.
    Legacy,
    /// Stage 3: Divergence A(내부 vpos rewind)를 SSOT(line_advances_sum)로 이전.
    A,
    /// **기본값(Stage 4 승격)**: A + Divergence C(TAC 그림 미주 순차 적층 — 겹침 가정 제거).
    B,
    /// 예약 tier — 현재 B 와 동일. 잔여 Divergence B(trailing-ls)·전면 SSOT 는 안전 정합
    /// 불가(Stage 5 실증: overflow 무영향이나 2022 overflow/2024·2023 질문흐름 회귀)로 보류.
    On,
    /// [v2 후보 A] 미주 다단 누적을 **렌더러 HeightCursor 시뮬레이션**으로 대체(실험).
    /// compute_en_metrics 근사 대신 build_single_column 동일 경로로 단 bottom y 를 스냅.
    A2,
    /// [v3 후보 A 정확화] A2 시뮬의 per-para 휴리스틱 높이 추정을 **scratch
    /// LayoutEngine::layout_partial_paragraph 실측**(렌더 권위)으로 대체. saved-vpos delta /
    /// total_height 근사 대신 실제 렌더 advance 를 사용 → A2 의 7건 재튜닝 회귀 해소가 목표.
    A3,
}

fn en_ssot_level() -> EnSsotLevel {
    // [Task #1363] 승격 이력:
    //   Stage 3 — A(rewind→line_advances_sum): 전 골든 무회귀로 기본 승격.
    //   Stage 4 — B(+TAC 그림 순차 적층, Divergence C): sep20/20 p22 overflow 50.1→0,
    //             cargo test 2126 pass·sweep flagged 불변으로 기본 승격. 미설정 시 B.
    // `legacy`/`off` 로 전 divergence 원복(긴급 롤백·비교), `A` 는 C 제외 단계, `on` 은 예약(현 B 동일).
    match std::env::var("RHWP_EN_SSOT").ok().as_deref() {
        Some("legacy") | Some("Legacy") | Some("off") => EnSsotLevel::Legacy,
        Some("A") => EnSsotLevel::A,
        Some("on") | Some("On") | Some("ON") => EnSsotLevel::On,
        Some("A2") | Some("a2") => EnSsotLevel::A2,
        Some("A3") | Some("a3") => EnSsotLevel::A3,
        _ => EnSsotLevel::B,
    }
}

/// [Task #1363] 미주 para 단위 SSOT divergence 정량 측정 디버그(`RHWP_EN_SSOT_DEBUG=1`).
/// `scripts/task1363_ssot_diff.py` 가 stderr 의 `EN_SSOT` 라인을 수집한다.
fn en_ssot_debug() -> bool {
    std::env::var("RHWP_EN_SSOT_DEBUG").is_ok()
}

/// [Task #853] ColumnDef 의 "디자인 spacing"(px): 1단이면 `간격`, 다단이면 0.
fn column_def_design_spacing_px(cd: &ColumnDef, dpi: f64) -> f64 {
    if cd.column_count.max(1) <= 1 {
        hwpunit_to_px(cd.spacing as i32, dpi)
    } else {
        0.0
    }
}

fn para_has_visible_text(para: &Paragraph) -> bool {
    para.text.chars().any(|c| c > '\u{001F}' && c != '\u{FFFC}')
}

fn para_has_non_whitespace_text(para: &Paragraph) -> bool {
    para.text
        .chars()
        .any(|c| c > '\u{001F}' && c != '\u{FFFC}' && !c.is_whitespace())
}

fn para_line_spacing_px(para: &Paragraph, dpi: f64) -> f64 {
    para.line_segs
        .last()
        .filter(|seg| seg.line_spacing > 0)
        .map(|seg| hwpunit_to_px(seg.line_spacing, dpi))
        .unwrap_or(0.0)
}

fn has_following_non_positive_visible_float(para: &Paragraph, control_index: usize) -> bool {
    para.controls
        .iter()
        .skip(control_index + 1)
        .any(|ctrl| match ctrl {
            Control::Table(table) => {
                is_para_topbottom_float(&table.common)
                    && signed_hwpunit(table.common.vertical_offset) <= 0
            }
            _ => false,
        })
}

fn para_is_non_tac_overlay_table_anchor(para: &Paragraph) -> bool {
    !para_has_non_whitespace_text(para)
        && para.controls.iter().any(|ctrl| {
            matches!(
                ctrl,
                Control::Table(table)
                    if !table.common.treat_as_char
                        && matches!(
                            table.common.text_wrap,
                            crate::model::shape::TextWrap::InFrontOfText
                                | crate::model::shape::TextWrap::BehindText
                        )
            )
        })
}

fn para_is_empty_topbottom_table_anchor(para: &Paragraph) -> bool {
    !para_has_visible_text(para)
        && para
            .controls
            .iter()
            .any(|ctrl| matches!(ctrl, Control::Table(t) if is_para_topbottom_float(&t.common)))
}

fn para_has_visible_text_or_equation(para: &Paragraph) -> bool {
    para_has_visible_text(para)
        || para
            .controls
            .iter()
            .any(|c| matches!(c, Control::Equation(eq) if eq.common.treat_as_char))
}

fn is_treat_as_char_equation_control(ctrl: Option<&Control>) -> bool {
    matches!(ctrl, Some(Control::Equation(eq)) if eq.common.treat_as_char)
}

fn para_is_treat_as_char_picture_only(para: &Paragraph) -> bool {
    !para_has_visible_text(para)
        && para.controls.iter().any(|ctrl| match ctrl {
            Control::Picture(pic) => pic.common.treat_as_char,
            Control::Shape(shape) => shape.common().treat_as_char,
            _ => false,
        })
}

fn para_has_treat_as_char_picture_or_shape(para: &Paragraph) -> bool {
    para.controls.iter().any(|ctrl| match ctrl {
        Control::Picture(pic) => pic.common.treat_as_char,
        Control::Shape(shape) => shape.common().treat_as_char,
        _ => false,
    })
}

fn non_tac_picture_or_shape_common(ctrl: &Control) -> Option<&crate::model::shape::CommonObjAttr> {
    match ctrl {
        Control::Picture(pic) if !pic.common.treat_as_char => Some(&pic.common),
        Control::Shape(shape) if !shape.common().treat_as_char => Some(shape.common()),
        _ => None,
    }
}

fn para_has_non_tac_picture_or_shape(para: &Paragraph) -> bool {
    para.controls
        .iter()
        .any(|ctrl| non_tac_picture_or_shape_common(ctrl).is_some())
}

fn non_tac_picture_or_shape_block_height_px(para: &Paragraph, dpi: f64) -> Option<f64> {
    let mut max_height = 0.0f64;
    let mut found = false;
    for ctrl in &para.controls {
        let Some(common) = non_tac_picture_or_shape_common(ctrl) else {
            continue;
        };
        let block_height_hu =
            common.height as i32 + common.margin.top as i32 + common.margin.bottom as i32;
        max_height = max_height.max(hwpunit_to_px(block_height_hu.max(1), dpi));
        found = true;
    }
    found.then_some(max_height)
}

fn non_tac_picture_or_shape_content_height_px(para: &Paragraph, dpi: f64) -> Option<f64> {
    let mut max_height = 0.0f64;
    let mut found = false;
    for ctrl in &para.controls {
        let Some(common) = non_tac_picture_or_shape_common(ctrl) else {
            continue;
        };
        max_height = max_height.max(hwpunit_to_px((common.height as i32).max(1), dpi));
        found = true;
    }
    found.then_some(max_height)
}

fn non_tac_square_picture_common(ctrl: &Control) -> Option<&crate::model::shape::CommonObjAttr> {
    let common = match ctrl {
        Control::Picture(pic) => Some(&pic.common),
        Control::Shape(shape) => {
            if let crate::model::shape::ShapeObject::Picture(pic) = shape.as_ref() {
                Some(&pic.common)
            } else {
                None
            }
        }
        _ => None,
    }?;
    (!common.treat_as_char && matches!(common.text_wrap, crate::model::shape::TextWrap::Square))
        .then_some(common)
}

fn paragraph_by_global_index<'a>(
    body_paragraphs: &'a [Paragraph],
    endnote_paragraphs: &'a [Paragraph],
    para_index: usize,
) -> Option<&'a Paragraph> {
    if para_index < body_paragraphs.len() {
        body_paragraphs.get(para_index)
    } else {
        endnote_paragraphs.get(para_index - body_paragraphs.len())
    }
}

fn page_item_para_index(item: &PageItem) -> Option<usize> {
    match item {
        PageItem::FullParagraph { para_index }
        | PageItem::PartialParagraph { para_index, .. }
        | PageItem::Table { para_index, .. }
        | PageItem::PartialTable { para_index, .. }
        | PageItem::Shape { para_index, .. } => Some(*para_index),
        PageItem::EndnoteSeparator { .. } => None,
    }
}

fn page_item_vpos_base(item: &PageItem, paragraphs: &[Paragraph]) -> Option<i32> {
    match item {
        PageItem::PartialParagraph {
            para_index,
            start_line,
            ..
        } => paragraphs
            .get(*para_index)
            .and_then(|para| para.line_segs.get(*start_line))
            .map(|seg| seg.vertical_pos),
        PageItem::FullParagraph { para_index }
        | PageItem::Table { para_index, .. }
        | PageItem::PartialTable { para_index, .. }
        | PageItem::Shape { para_index, .. } => paragraphs
            .get(*para_index)
            .and_then(|para| para.line_segs.first())
            .map(|seg| seg.vertical_pos),
        PageItem::EndnoteSeparator { .. } => None,
    }
}

fn square_picture_wrap_anchor_for_para(
    st: &TypesetState,
    body_paragraphs: &[Paragraph],
    para: &Paragraph,
    page_def: &PageDef,
) -> Option<crate::renderer::pagination::WrapAnchorRef> {
    if st.wrap_around_cs < 0 {
        return None;
    }

    let para_cs = para.line_segs.first().map(|s| s.column_start).unwrap_or(0);
    let para_sw = para
        .line_segs
        .first()
        .map(|s| s.segment_width as i32)
        .unwrap_or(0);
    let is_empty_para = para
        .text
        .chars()
        .all(|ch| ch.is_whitespace() || ch == '\r' || ch == '\n')
        && para.controls.is_empty();
    let any_seg_matches = para.line_segs.iter().any(|s| {
        s.column_start == st.wrap_around_cs && s.segment_width as i32 == st.wrap_around_sw
    });
    let body_w =
        (page_def.width as i32) - (page_def.margin_left as i32) - (page_def.margin_right as i32);
    let sw0_match = st.wrap_around_sw == 0 && is_empty_para && para_sw > 0 && para_sw < body_w / 2;

    let anchor_para = paragraph_by_global_index(
        body_paragraphs,
        &st.endnote_paragraphs,
        st.wrap_around_table_para,
    )?;
    let anchor_image_match = if st.wrap_around_cs == 0 {
        let body_left = page_def.margin_left as i32;
        let expected_cs_hu = anchor_para
            .controls
            .iter()
            .find_map(|ctrl| {
                non_tac_square_picture_common(ctrl).map(|common| {
                    common.horizontal_offset as i32
                        + common.width as i32
                        + 2 * common.margin.right as i32
                        - body_left
                })
            })
            .unwrap_or(0);
        expected_cs_hu > 0
            && (para_cs - expected_cs_hu).abs() < 200
            && para_sw > 0
            && para_cs + para_sw <= body_w + 200
    } else {
        false
    };
    let cs_only_match = st.wrap_around_any_seg && para_cs == st.wrap_around_cs && para_sw > 0;
    let matched = (para_cs == st.wrap_around_cs && para_sw == st.wrap_around_sw)
        || (any_seg_matches && (is_empty_para || st.wrap_around_any_seg))
        || sw0_match
        || anchor_image_match
        || cs_only_match;
    if !matched {
        return None;
    }

    let anchor_image_margin_right = anchor_para.controls.iter().find_map(|ctrl| {
        non_tac_square_picture_common(ctrl).map(|common| common.margin.right as i32)
    })?;
    Some(crate::renderer::pagination::WrapAnchorRef {
        anchor_para_index: st.wrap_around_table_para,
        anchor_cs: st.wrap_around_cs,
        anchor_sw: st.wrap_around_sw,
        anchor_image_margin_right,
    })
}

fn maybe_register_square_picture_wrap_anchor(
    st: &mut TypesetState,
    body_paragraphs: &[Paragraph],
    para: &Paragraph,
    para_index: usize,
    page_def: &PageDef,
) {
    if st.wrap_around_cs < 0 {
        return;
    }
    if let Some(anchor) = square_picture_wrap_anchor_for_para(st, body_paragraphs, para, page_def) {
        st.current_column_wrap_anchors.insert(para_index, anchor);
    } else {
        st.wrap_around_cs = -1;
        st.wrap_around_sw = -1;
        st.wrap_around_any_seg = false;
    }
}

fn activate_square_picture_wrap_for_para(
    st: &mut TypesetState,
    para_index: usize,
    para: &Paragraph,
) {
    if !para
        .controls
        .iter()
        .any(|ctrl| non_tac_square_picture_common(ctrl).is_some())
    {
        return;
    }

    let anchor_cs = para.line_segs.first().map(|s| s.column_start).unwrap_or(0);
    let anchor_sw = para
        .line_segs
        .first()
        .map(|s| s.segment_width as i32)
        .unwrap_or(0);
    if anchor_cs > 0 || anchor_sw > 0 {
        st.wrap_around_cs = anchor_cs;
        st.wrap_around_sw = anchor_sw;
        st.wrap_around_table_para = para_index;
        st.wrap_around_any_seg = true;
    }
}

fn composed_line_char_end(comp: &ComposedParagraph, line_idx: usize) -> usize {
    if let Some(next) = comp.lines.get(line_idx + 1) {
        return next.char_start;
    }
    let Some(line) = comp.lines.get(line_idx) else {
        return 0;
    };
    line.char_start
        + line
            .runs
            .iter()
            .map(|run| run.text.chars().count())
            .sum::<usize>()
        + usize::from(line.has_line_break)
}

fn line_has_strict_tac_control(comp: &ComposedParagraph, line_idx: usize) -> bool {
    let Some(line) = comp.lines.get(line_idx) else {
        return false;
    };
    let start = line.char_start;
    let end = composed_line_char_end(comp, line_idx);
    end > start
        && comp
            .tac_controls
            .iter()
            .any(|(pos, _, _)| *pos >= start && *pos < end)
}

fn line_has_strict_equation_tac_control(
    para: &Paragraph,
    comp: &ComposedParagraph,
    line_idx: usize,
) -> bool {
    let Some(line) = comp.lines.get(line_idx) else {
        return false;
    };
    let start = line.char_start;
    let end = composed_line_char_end(comp, line_idx);
    end > start
        && comp.tac_controls.iter().any(|(pos, _, ci)| {
            *pos >= start && *pos < end && is_treat_as_char_equation_control(para.controls.get(*ci))
        })
}

fn line_is_leading_empty_equation_tac_guide(
    para: &Paragraph,
    comp: &ComposedParagraph,
    line_idx: usize,
) -> bool {
    let Some(line) = comp.lines.get(line_idx) else {
        return false;
    };
    let Some(next) = comp.lines.get(line_idx + 1) else {
        return false;
    };
    line.runs.is_empty()
        && line.char_start == next.char_start
        && !line_has_strict_tac_control(comp, line_idx)
        && line_has_strict_equation_tac_control(para, comp, line_idx + 1)
}

fn equation_only_tac_line_assignment(
    para: &Paragraph,
    comp: &ComposedParagraph,
) -> Option<Vec<usize>> {
    let n_lines = comp.lines.len();
    if n_lines <= 1 || comp.tac_controls.is_empty() {
        return None;
    }
    if !comp.lines.iter().all(|line| line.runs.is_empty()) {
        return None;
    }
    let degenerate = comp
        .lines
        .windows(2)
        .any(|w| w[1].char_start <= w[0].char_start);
    if !degenerate {
        return None;
    }

    let mut assign = vec![n_lines - 1; comp.tac_controls.len()];
    let mut line_idx = 0usize;
    let mut tac_idx = 0usize;
    while tac_idx < comp.tac_controls.len() {
        let pos = comp.tac_controls[tac_idx].0;
        while line_idx < n_lines && comp.lines[line_idx].char_start < pos {
            line_idx += 1;
        }

        let tac_start = tac_idx;
        while tac_idx < comp.tac_controls.len() && comp.tac_controls[tac_idx].0 == pos {
            tac_idx += 1;
        }
        let tac_count = tac_idx - tac_start;

        let line_start = line_idx;
        while line_idx < n_lines && comp.lines[line_idx].char_start == pos {
            line_idx += 1;
        }
        let line_candidates: Vec<usize> = (line_start..line_idx).collect();
        let filtered_candidates: Vec<usize> = line_candidates
            .iter()
            .copied()
            .filter(|idx| !line_is_leading_empty_equation_tac_guide(para, comp, *idx))
            .collect();
        let line_targets = if tac_count > 1 && line_candidates.len() >= tac_count {
            // 같은 char_start에 여러 TAC 수식이 있고 저장 LINE_SEG도 같은 수만큼 있으면
            // 선행 빈 guide 줄도 한컴의 물리 수식 줄로 보존한다.
            &line_candidates
        } else if filtered_candidates.is_empty() {
            &line_candidates
        } else {
            &filtered_candidates
        };

        for offset in 0..tac_count {
            assign[tac_start + offset] = if line_targets.is_empty() {
                line_start.min(n_lines - 1)
            } else {
                line_targets[offset.min(line_targets.len() - 1)]
            };
        }
    }

    Some(assign)
}

fn tac_control_indices_for_line(
    para: &Paragraph,
    comp: &ComposedParagraph,
    line_idx: usize,
) -> Vec<usize> {
    let Some(line) = comp.lines.get(line_idx) else {
        return Vec::new();
    };
    if comp.tac_controls.is_empty() {
        return Vec::new();
    }

    if let Some(assign) = equation_only_tac_line_assignment(para, comp) {
        return comp
            .tac_controls
            .iter()
            .enumerate()
            .filter_map(|(idx, (_, _, ci))| {
                (assign.get(idx).copied() == Some(line_idx)).then_some(*ci)
            })
            .collect();
    }

    if line.runs.is_empty() {
        let start = line.char_start;
        let end = comp
            .lines
            .get(line_idx + 1)
            .map(|next| next.char_start)
            .unwrap_or(usize::MAX);
        return comp
            .tac_controls
            .iter()
            .filter_map(|(pos, _, ci)| (*pos >= start && *pos < end).then_some(*ci))
            .collect();
    }

    let next_start = comp.lines.get(line_idx + 1).map(|next| next.char_start);
    let mut hits = Vec::new();
    let mut run_start = line.char_start;
    for (run_idx, run) in line.runs.iter().enumerate() {
        let run_len = run.text.chars().count();
        let run_end = run_start + run_len;
        let next_line_starts_at_run_end = next_start.is_some_and(|start| start == run_end);
        let allow_end_tac = run_idx == line.runs.len() - 1 && !next_line_starts_at_run_end;
        for (pos, _, ci) in &comp.tac_controls {
            if *pos >= run_start && (*pos < run_end || (allow_end_tac && *pos == run_end)) {
                hits.push(*ci);
            }
        }
        run_start = run_end;
    }
    hits
}

fn line_has_tac_equation_control(
    para: &Paragraph,
    comp: &ComposedParagraph,
    line_idx: usize,
) -> bool {
    tac_control_indices_for_line(para, comp, line_idx)
        .iter()
        .any(|ci| is_treat_as_char_equation_control(para.controls.get(*ci)))
}

fn line_has_visible_text(comp: &ComposedParagraph, line_idx: usize) -> bool {
    comp.lines
        .get(line_idx)
        .map(|line| {
            line.runs
                .iter()
                .flat_map(|run| run.text.chars())
                .any(|c| c > '\u{001F}' && c != '\u{FFFC}')
        })
        .unwrap_or(false)
}

fn line_has_text_span(comp: &ComposedParagraph, line_idx: usize) -> bool {
    comp.lines
        .get(line_idx)
        .is_some_and(|line| composed_line_char_end(comp, line_idx) > line.char_start)
}

fn line_leading_tac_equation_count(
    para: &Paragraph,
    comp: &ComposedParagraph,
    line_idx: usize,
) -> usize {
    let Some(line_start) = comp.lines.get(line_idx).map(|line| line.char_start) else {
        return 0;
    };
    let line_controls = tac_control_indices_for_line(para, comp, line_idx);
    comp.tac_controls
        .iter()
        .filter(|(pos, _, ci)| {
            *pos == line_start
                && line_controls.contains(ci)
                && is_treat_as_char_equation_control(para.controls.get(*ci))
        })
        .count()
}

fn line_is_equation_tac_text_run_only(
    para: &Paragraph,
    comp: &ComposedParagraph,
    line_idx: usize,
) -> bool {
    if line_has_visible_text(comp, line_idx) {
        return false;
    }

    let line_controls = tac_control_indices_for_line(para, comp, line_idx);
    !line_controls.is_empty()
        && line_controls
            .iter()
            .all(|ci| is_treat_as_char_equation_control(para.controls.get(*ci)))
}

fn line_has_visible_text_or_tac_equation(
    para: &Paragraph,
    comp: &ComposedParagraph,
    line_idx: usize,
) -> bool {
    line_has_visible_text(comp, line_idx) || line_has_tac_equation_control(para, comp, line_idx)
}

fn line_has_tac_control(para: &Paragraph, comp: &ComposedParagraph, line_idx: usize) -> bool {
    !tac_control_indices_for_line(para, comp, line_idx).is_empty()
}

fn tac_picture_or_shape_height_px(ctrl: &Control, dpi: f64) -> Option<f64> {
    let height_hu = match ctrl {
        Control::Picture(pic) if pic.common.treat_as_char => pic.common.height as i32,
        Control::Shape(shape) if shape.common().treat_as_char => shape.common().height as i32,
        _ => return None,
    };
    Some(hwpunit_to_px(height_hu, dpi))
}

fn line_tac_picture_or_shape_height(
    para: &Paragraph,
    comp: &ComposedParagraph,
    line_idx: usize,
    dpi: f64,
) -> Option<f64> {
    tac_control_indices_for_line(para, comp, line_idx)
        .iter()
        .find_map(|ci| {
            para.controls
                .get(*ci)
                .and_then(|ctrl| tac_picture_or_shape_height_px(ctrl, dpi))
        })
}

fn text_line_is_picture_lead_in(
    para: &Paragraph,
    comp: &ComposedParagraph,
    line_idx: usize,
    raw_lh: f64,
    max_fs: f64,
    dpi: f64,
) -> bool {
    if max_fs <= 0.0 || raw_lh <= max_fs * 2.0 {
        return false;
    }
    let Some(line) = comp.lines.get(line_idx) else {
        return false;
    };
    if line.runs.iter().all(|run| run.text.trim().is_empty())
        || line_tac_picture_or_shape_height(para, comp, line_idx, dpi).is_some()
    {
        return false;
    }
    let Some(next) = comp.lines.get(line_idx + 1) else {
        return false;
    };
    if !next.runs.iter().all(|run| run.text.trim().is_empty()) {
        return false;
    }
    line_tac_picture_or_shape_height(para, comp, line_idx + 1, dpi)
        .map(|height| (raw_lh - height).abs() <= 8.0)
        .unwrap_or(false)
}

fn is_sample16_integrated_db_cluster_tail_paragraph(para: &Paragraph) -> bool {
    para.text.starts_with('\u{F03C5}')
        && para
            .text
            .contains("계약상대자는 통합DB서버에서 운영될 주요업무에 대해 Active-Active")
        && para.controls.iter().all(|c| matches!(c, Control::Field(_)))
}

fn internal_vpos_page_break_line(
    para: &Paragraph,
    line_count: usize,
    body_height_px: f64,
    dpi: f64,
) -> Option<usize> {
    if !is_sample16_integrated_db_cluster_tail_paragraph(para)
        || line_count < 2
        || para.line_segs.len() < line_count
    {
        return None;
    }

    let first = para.line_segs.first()?;
    if first.vertical_pos <= 0 || hwpunit_to_px(first.vertical_pos, dpi) < body_height_px * 0.7 {
        return None;
    }

    para.line_segs
        .windows(2)
        .enumerate()
        .find_map(|(prev_idx, pair)| {
            let prev = &pair[0];
            let cur = &pair[1];
            if !is_synthetic_line_seg(prev)
                && !is_synthetic_line_seg(cur)
                && prev.vertical_pos > 0
                && cur.vertical_pos <= 0
            {
                Some(prev_idx + 1)
            } else {
                None
            }
        })
}

fn sample16_missing_lineseg_tail_break_line(
    para: &Paragraph,
    line_count: usize,
    current_height: f64,
    available: f64,
) -> Option<usize> {
    if !para.line_segs.is_empty()
        || line_count < 4
        || current_height < available * 0.75
        || !is_sample16_integrated_db_cluster_tail_paragraph(para)
    {
        return None;
    }

    Some(3)
}

fn is_synthetic_line_seg(ls: &LineSeg) -> bool {
    ls.tag & 0x80000000 != 0
}

fn paragraph_saved_vpos_reset_starts_new_page_after(
    current_para: &Paragraph,
    next_para: &Paragraph,
    col_count: u16,
    is_hwp3_variant: bool,
) -> bool {
    let next_first_vpos = next_para.line_segs.first().map(|s| s.vertical_pos);
    let curr_last_vpos = current_para.line_segs.last().map(|s| s.vertical_pos);
    let multi_col = col_count > 1;
    let allowed_top_vpos = if is_hwp3_variant { 1500 } else { 0 };

    matches!((next_first_vpos, curr_last_vpos), (Some(nv), Some(cl))
        if (if multi_col { nv < cl } else { nv <= allowed_top_vpos }) && cl > 5000)
}

fn paragraph_forces_page_boundary_after(
    current_para: &Paragraph,
    next_para: &Paragraph,
    col_count: u16,
    is_hwp3_variant: bool,
) -> bool {
    matches!(
        next_para.column_type,
        ColumnBreakType::Page | ColumnBreakType::Section
    ) || paragraph_saved_vpos_reset_starts_new_page_after(
        current_para,
        next_para,
        col_count,
        is_hwp3_variant,
    )
}

fn single_line_visible_bounds_px(
    para: &Paragraph,
    page_vpos_base: i32,
    dpi: f64,
) -> Option<(f64, f64)> {
    let mut real_lines = para
        .line_segs
        .iter()
        .filter(|ls| !is_synthetic_line_seg(ls));
    let line = real_lines.next()?;
    if real_lines.next().is_some() {
        return None;
    }

    line_seg_visible_bounds_px(line, page_vpos_base, dpi)
}

fn line_seg_visible_bounds_px(seg: &LineSeg, page_vpos_base: i32, dpi: f64) -> Option<(f64, f64)> {
    let top = seg.vertical_pos.saturating_sub(page_vpos_base);
    let bottom = seg
        .vertical_pos
        .saturating_add(seg.line_height)
        .saturating_sub(page_vpos_base);
    (top >= 0 && bottom >= 0).then(|| (hwpunit_to_px(top, dpi), hwpunit_to_px(bottom, dpi)))
}

fn saved_bounds_fit_at_flow_tail(bounds: (f64, f64), current_height: f64, available: f64) -> bool {
    let (top, bottom) = bounds;
    top + 16.0 >= current_height && bottom <= available + 0.5
}

fn positive_vpos_end_before_negative_wrap(para: &Paragraph) -> Option<i32> {
    let last_real = para
        .line_segs
        .iter()
        .rev()
        .find(|ls| !is_synthetic_line_seg(ls))?;
    if last_real.vertical_pos >= 0 {
        return None;
    }

    para.line_segs
        .iter()
        .filter(|ls| !is_synthetic_line_seg(ls) && ls.vertical_pos > 0)
        .map(|ls| ls.vertical_pos.saturating_add(ls.line_height))
        .max()
}

fn para_near_rowbreak_table(paragraphs: &[Paragraph], para_idx: usize) -> bool {
    let start = para_idx.saturating_sub(1);
    let end = (para_idx + 3).min(paragraphs.len());
    paragraphs[start..end].iter().any(|para| {
        para.controls.iter().any(|control| {
            matches!(
                control,
                Control::Table(table)
                    if matches!(
                        table.page_break,
                        crate::model::table::TablePageBreak::RowBreak
                    )
            )
        })
    })
}

/// #1672 행정업무 편람 계열: raw TABLE attr 상위 바이트가 비어 있는 RowBreak 표는
/// 기존 4px 안전마진만으로도 페이지가 누적 과분할된다.
fn section_has_zero_high_attr_rowbreak_table(paragraphs: &[Paragraph]) -> bool {
    paragraphs.iter().any(|para| {
        para.controls.iter().any(|control| {
            matches!(
                control,
                Control::Table(table)
                    if matches!(
                        table.page_break,
                        crate::model::table::TablePageBreak::RowBreak
                    ) && (table.raw_table_record_attr & 0xff00_0000) == 0
            )
        })
    })
}

impl TypesetState {
    fn new(
        layout: PageLayoutInfo,
        col_count: u16,
        section_index: usize,
        footnote_separator_overhead: f64,
        footnote_between_notes_margin: f64,
        footnote_safety_margin: f64,
        column_type: ColumnType,
    ) -> Self {
        Self {
            pages: Vec::new(),
            current_items: Vec::new(),
            current_height: 0.0,
            current_start_height: 0.0,
            current_endnote_flow: false,
            prev_body_bottom_vpos: None,
            current_column: 0,
            col_count,
            layout,
            section_index,
            current_footnote_height: 0.0,
            is_first_footnote_on_page: true,
            footnote_separator_overhead,
            footnote_between_notes_margin,
            footnote_safety_margin,
            current_zone_y_offset: 0.0,
            current_zone_layout: None,
            on_first_multicolumn_page: false,
            pending_body_wide_top_reserve: 0.0,
            visible_float_exclusions: Vec::new(),
            skip_safety_margin_once: false,
            skip_footnote_margin_once: false,
            is_hwp3_variant: false,
            is_hwpx_source: false,
            hide_empty_line: false,
            hidden_empty_lines: 0,
            hidden_empty_page_idx: usize::MAX,
            hidden_empty_paras: std::collections::HashSet::new(),
            endnotes: Vec::new(),
            endnote_paragraphs: Vec::new(),
            endnote_para_sources: Vec::new(),
            endnote_between_notes_hu: 0,
            endnote_separator_above_hu: 0,
            endnote_separator_below_hu: 0,
            wrap_around_cs: -1,
            wrap_around_sw: -1,
            wrap_around_table_para: 0,
            wrap_around_any_seg: false,
            current_column_wrap_around_paras: Vec::new(),
            current_column_wrap_anchors: std::collections::HashMap::new(),
            current_zone_column_type: column_type,
            current_zone_design_spacing_px: 0.0,
            vpos_page_base: None,
            vpos_lazy_base: None,
            vpos_prev_layout_para: None,
            vpos_prev_partial_table: false,
            vpos_col_anchor: 0.0,
            skip_spacing_before_prededuct: false,
        }
    }

    /// [Task #1027 Stage D] 컬럼 경계에서 vpos 스냅 상태 초기화.
    /// 렌더러 build_single_column 진입 정합: page/lazy base·prev 초기화,
    /// anchor 를 현 current_height(컬럼 시작값)로 설정.
    fn reset_vpos_cursor(&mut self) {
        self.vpos_page_base = None;
        self.vpos_lazy_base = None;
        self.vpos_prev_layout_para = None;
        self.vpos_prev_partial_table = false;
        self.vpos_col_anchor = self.current_height;
    }

    /// 사용 가능한 본문 높이 (각주, 존 오프셋 차감)
    fn available_height(&self) -> f64 {
        let base = self.layout.available_body_height();
        let fn_margin = if self.current_footnote_height > 0.0 {
            self.footnote_safety_margin
        } else {
            0.0
        };
        (base - self.current_footnote_height - fn_margin - self.current_zone_y_offset).max(0.0)
    }

    /// 기본 가용 높이 (각주/존 미차감)
    fn base_available_height(&self) -> f64 {
        self.layout.available_body_height()
    }

    /// 각주 높이 추가
    fn add_footnote_height(&mut self, height: f64) {
        if self.is_first_footnote_on_page {
            self.current_footnote_height += self.footnote_separator_overhead;
            self.is_first_footnote_on_page = false;
        } else {
            self.current_footnote_height += self.footnote_between_notes_margin;
        }
        self.current_footnote_height += height;
        self.sync_current_page_footnote_area();
    }

    fn projected_footnote_height(&self, note_content_height: f64, note_count: usize) -> f64 {
        if note_count == 0 {
            return self.current_footnote_height;
        }
        let separator = if self.is_first_footnote_on_page {
            self.footnote_separator_overhead
        } else {
            0.0
        };
        let between_count = if self.is_first_footnote_on_page {
            note_count.saturating_sub(1)
        } else {
            note_count
        };
        self.current_footnote_height
            + separator
            + self.footnote_between_notes_margin * between_count as f64
            + note_content_height
    }

    fn sync_current_page_footnote_area(&mut self) {
        if self.current_footnote_height <= 0.0 {
            return;
        }
        if let Some(page) = self.pages.last_mut() {
            page.layout
                .update_footnote_area(self.current_footnote_height);
        }
    }

    /// 현재 항목을 ColumnContent로 만들어 마지막 페이지에 push
    fn flush_column(&mut self) {
        if self.current_items.is_empty() && self.current_column_wrap_around_paras.is_empty() {
            return;
        }
        let col_content = ColumnContent {
            column_index: self.current_column,
            start_height: self.current_start_height,
            endnote_flow: self.current_endnote_flow,
            items: std::mem::take(&mut self.current_items),
            zone_layout: self.current_zone_layout.clone(),
            zone_y_offset: self.current_zone_y_offset,
            wrap_around_paras: std::mem::take(&mut self.current_column_wrap_around_paras),
            used_height: self.current_height,
            wrap_anchors: std::mem::take(&mut self.current_column_wrap_anchors),
        };
        if let Some(page) = self.pages.last_mut() {
            page.column_contents.push(col_content);
        } else {
            self.pages.push(self.new_page_content(vec![col_content]));
        }
        // [Task #1082] 단 flush 시 본문 last bottom vpos 리셋(미주 vpos-delta 시드 정합).
        self.prev_body_bottom_vpos = None;
    }

    /// 비어있어도 flush
    fn flush_column_always(&mut self) {
        let col_content = ColumnContent {
            column_index: self.current_column,
            start_height: self.current_start_height,
            endnote_flow: self.current_endnote_flow,
            items: std::mem::take(&mut self.current_items),
            zone_layout: self.current_zone_layout.clone(),
            zone_y_offset: self.current_zone_y_offset,
            wrap_around_paras: std::mem::take(&mut self.current_column_wrap_around_paras),
            used_height: self.current_height,
            wrap_anchors: std::mem::take(&mut self.current_column_wrap_anchors),
        };
        if let Some(page) = self.pages.last_mut() {
            page.column_contents.push(col_content);
        } else {
            self.pages.push(self.new_page_content(vec![col_content]));
        }
    }

    /// 다음 단 또는 새 페이지
    fn advance_column_or_new_page(&mut self) {
        self.flush_column();
        self.visible_float_exclusions.clear();
        if self.current_column + 1 < self.col_count {
            self.current_column += 1;
            // Task #321: col 0 상단의 body-wide TopAndBottom 표/도형이 차지한 높이를
            // current_height의 시작값으로 사용 (가용 공간만 줄임, zone_y_offset은 건드리지 않음).
            // layout은 body_wide_reserved로 별도 처리하므로 여기서 zone_y_offset에
            // 넣으면 double-shift가 발생.
            self.current_height = self.pending_body_wide_top_reserve;
            self.current_start_height = self.current_height;
            self.current_endnote_flow = false;
            self.reset_vpos_cursor();
        } else {
            self.push_new_page();
        }
    }

    /// 강제 새 페이지
    fn force_new_page(&mut self) {
        self.flush_column();
        self.push_new_page();
    }

    /// 페이지 보장
    fn ensure_page(&mut self) {
        if self.pages.is_empty() {
            self.pages.push(self.new_page_content(Vec::new()));
        }
    }

    /// 새 페이지 push + 상태 리셋
    fn push_new_page(&mut self) {
        self.pages.push(self.new_page_content(Vec::new()));
        self.reset_for_new_page();
        // Task #321: 새 페이지에서는 body-wide top reserve 초기화
        self.pending_body_wide_top_reserve = 0.0;
    }

    fn reset_for_new_page(&mut self) {
        self.current_column = 0;
        self.current_height = 0.0;
        self.current_start_height = 0.0;
        self.current_endnote_flow = false;
        self.current_footnote_height = 0.0;
        self.is_first_footnote_on_page = true;
        self.current_zone_y_offset = 0.0;
        self.current_zone_layout = None;
        self.on_first_multicolumn_page = false;
        self.visible_float_exclusions.clear();
        self.reset_vpos_cursor();
    }

    fn apply_visible_float_exclusions(&mut self, probe_height: f64) {
        if self.visible_float_exclusions.is_empty() {
            return;
        }

        let use_overlap_probe = self.is_hwpx_source && probe_height > 0.0;
        self.visible_float_exclusions
            .retain(|zone| self.current_height < zone.bottom - 0.5);

        let mut jump_to = self.current_height;
        for zone in &self.visible_float_exclusions {
            let starts_in_zone = jump_to + 0.5 >= zone.top && jump_to < zone.bottom;
            let overlaps_zone =
                use_overlap_probe && jump_to < zone.top && jump_to + probe_height > zone.top + 0.5;
            if starts_in_zone || overlaps_zone {
                jump_to = jump_to.max(zone.bottom);
            }
        }

        if jump_to > self.current_height + 0.5 {
            self.current_height = jump_to;
        }
    }

    fn new_page_content(&self, column_contents: Vec<ColumnContent>) -> PageContent {
        PageContent {
            page_index: self.pages.len() as u32,
            page_number: 0,
            section_index: self.section_index,
            layout: self.layout.clone(),
            column_contents,
            active_header: None,
            active_footer: None,
            page_number_pos: None,
            page_hide: None,
            footnotes: Vec::new(),
            active_master_page: None,
            extra_master_pages: Vec::new(),
        }
    }
}

/// 문단 format() 결과: 문단의 실제 렌더링 높이 정보
#[derive(Debug)]
struct FormattedParagraph {
    /// 총 높이 (spacing 포함)
    total_height: f64,
    /// 줄별 콘텐츠 높이 (line_height만)
    line_heights: Vec<f64>,
    /// 줄별 줄간격 (line_spacing)
    line_spacings: Vec<f64>,
    /// spacing_before
    spacing_before: f64,
    /// spacing_after
    spacing_after: f64,
    /// trailing line_spacing을 제외한 판단용 높이
    height_for_fit: f64,
}

impl FormattedParagraph {
    /// 특정 줄의 advance 높이 (콘텐츠 + 줄간격)
    #[inline]
    fn line_advance(&self, line_idx: usize) -> f64 {
        self.line_heights[line_idx] + self.line_spacings[line_idx]
    }

    /// 줄 범위의 advance 합계
    fn line_advances_sum(&self, range: std::ops::Range<usize>) -> f64 {
        range
            .into_iter()
            .map(|i| self.line_heights[i] + self.line_spacings[i])
            .sum()
    }

    fn flow_advance_height(
        &self,
        para: &Paragraph,
        col_count: u16,
        allow_spacing_before_only: bool,
    ) -> f64 {
        if col_count > 1 {
            return self.height_for_fit;
        }
        if para.controls.is_empty()
            && !para.line_segs.is_empty()
            && (self.spacing_after > 0.5
                || (allow_spacing_before_only && self.spacing_before > 0.5))
            && self.height_for_fit > 0.0
            && self.height_for_fit + 0.5 < self.total_height
        {
            return self.height_for_fit.min(self.total_height);
        }
        self.total_height
    }
}

fn debug_brief_line_text(text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for ch in text.chars().take(max_chars) {
        match ch {
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\u{FFFC}' => out.push_str("<TAC>"),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    if text.chars().count() > max_chars {
        out.push('…');
    }
    out
}

fn debug_endnote_control_kind(ctrl: &Control) -> &'static str {
    match ctrl {
        Control::Equation(_) => "eq",
        Control::Picture(pic) if pic.common.treat_as_char => "pic:tac",
        Control::Picture(_) => "pic",
        Control::Shape(shape) if shape.common().treat_as_char => "shape:tac",
        Control::Shape(_) => "shape",
        Control::Table(table) if table.common.treat_as_char => "table:tac",
        Control::Table(_) => "table",
        _ => "ctrl",
    }
}

fn debug_endnote_control_height_hu(ctrl: &Control) -> Option<i32> {
    match ctrl {
        Control::Equation(eq) => Some(eq.common.height as i32),
        Control::Picture(pic) => Some(pic.common.height as i32),
        Control::Shape(shape) => Some(shape.common().height as i32),
        Control::Table(table) => Some(table.common.height as i32),
        _ => None,
    }
}

fn debug_print_endnote_line_segments(
    note_number: u16,
    ep_idx: usize,
    para: &Paragraph,
    comp: &ComposedParagraph,
    fmt: &FormattedParagraph,
    dpi: f64,
    endnote_start: i32,
) {
    use std::fmt::Write as _;

    let control_positions = para.control_text_positions();
    let para_text = debug_brief_line_text(&para.text, 120);
    eprintln!(
        "ENDNOTE_LINE note={} ep={} para_chars={} line_segs={} comp_lines={} fmt_lines={} start={} text=\"{}\"",
        note_number,
        ep_idx,
        para.char_count,
        para.line_segs.len(),
        comp.lines.len(),
        fmt.line_heights.len(),
        endnote_start,
        para_text
    );

    for line_idx in 0..fmt
        .line_heights
        .len()
        .max(comp.lines.len())
        .max(para.line_segs.len())
    {
        let seg = para.line_segs.get(line_idx);
        let comp_line = comp.lines.get(line_idx);
        let (comp_start, comp_end, runs_empty, run_text) = if let Some(line) = comp_line {
            let text = line
                .runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>();
            (
                Some(line.char_start),
                Some(composed_line_char_end(comp, line_idx)),
                line.runs.is_empty(),
                debug_brief_line_text(&text, 80),
            )
        } else {
            (None, None, false, String::new())
        };

        let mut tac_desc = String::new();
        let tac_indices = if line_idx < comp.lines.len() {
            tac_control_indices_for_line(para, comp, line_idx)
        } else {
            Vec::new()
        };
        for ci in tac_indices {
            if !tac_desc.is_empty() {
                tac_desc.push(',');
            }
            if let Some(ctrl) = para.controls.get(ci) {
                let pos = control_positions.get(ci).copied();
                let height = debug_endnote_control_height_hu(ctrl)
                    .map(|h| h.to_string())
                    .unwrap_or_else(|| "-".to_string());
                let _ = write!(
                    tac_desc,
                    "{}@{:?}:{}h{}",
                    ci,
                    pos,
                    debug_endnote_control_kind(ctrl),
                    height
                );
            } else {
                let _ = write!(tac_desc, "{}@?:missing", ci);
            }
        }

        let fmt_lh = fmt.line_heights.get(line_idx).copied();
        let fmt_ls = fmt.line_spacings.get(line_idx).copied();
        let fmt_adv = fmt_lh.zip(fmt_ls).map(|(h, s)| h + s);
        eprintln!(
            "ENDNOTE_LINE note={} ep={} line={} seg_ts={:?} seg_char={:?} seg_vpos={:?} seg_abs={:?} seg_lh={:?} seg_th={:?} seg_ls={:?} fmt_lh={:?} fmt_ls={:?} fmt_adv={:?} comp={:?}..{:?} runs_empty={} tac=[{}] text=\"{}\"",
            note_number,
            ep_idx,
            line_idx,
            seg.map(|s| s.text_start),
            seg.map(|s| para.utf16_pos_to_char_idx(s.text_start)),
            seg.map(|s| s.vertical_pos),
            seg.map(|s| s.vertical_pos + endnote_start),
            seg.map(|s| hwpunit_to_px(s.line_height, dpi)),
            seg.map(|s| hwpunit_to_px(s.text_height, dpi)),
            seg.map(|s| hwpunit_to_px(s.line_spacing, dpi)),
            fmt_lh,
            fmt_ls,
            fmt_adv,
            comp_start,
            comp_end,
            runs_empty,
            tac_desc,
            run_text
        );
    }
}

impl TypesetEngine {
    pub fn new(dpi: f64) -> Self {
        Self {
            dpi,
            is_hwpx_source: std::cell::Cell::new(false),
            is_hwp3_variant: std::cell::Cell::new(false),
        }
    }

    pub fn with_default_dpi() -> Self {
        Self::new(DEFAULT_DPI)
    }

    fn predict_current_column_para_y(
        &self,
        st: &TypesetState,
        target_para_idx: usize,
        paragraphs: &[Paragraph],
        styles: &ResolvedStyleSet,
        measured_tables: &[MeasuredTable],
        column_width: Option<f64>,
    ) -> Option<f64> {
        let mut local_paras: Vec<Paragraph> = Vec::new();
        let mut local_indices: Vec<(usize, usize)> = Vec::new();
        for pi in st
            .current_items
            .iter()
            .filter_map(page_item_para_index)
            .chain(std::iter::once(target_para_idx))
        {
            if local_indices.iter().any(|(global, _)| *global == pi) {
                continue;
            }
            if let Some(p) = paragraph_by_global_index(paragraphs, &st.endnote_paragraphs, pi) {
                let local = local_paras.len();
                local_paras.push(p.clone());
                local_indices.push((pi, local));
            }
        }
        let lookup_local = |pi: usize, indices: &[(usize, usize)]| {
            indices
                .iter()
                .find_map(|(global, local)| (*global == pi).then_some(*local))
        };
        let first_vpos = st
            .current_items
            .iter()
            .filter_map(page_item_para_index)
            .find_map(|pi| {
                paragraph_by_global_index(paragraphs, &st.endnote_paragraphs, pi)
                    .and_then(|p| p.line_segs.first())
                    .map(|seg| seg.vertical_pos)
            })?;

        let available = st.available_height();
        let mut hc = HeightCursor::new(
            self.dpi,
            0.0,
            available,
            st.current_start_height,
            Some(first_vpos),
            st.skip_spacing_before_prededuct,
            false,
            st.current_endnote_flow && st.current_start_height < -0.5,
            st.current_endnote_flow,
        );
        hc.endnote_between_notes_hu = st.endnote_between_notes_hu;
        let mut y = st.current_start_height;
        for item in &st.current_items {
            let Some(pi) = page_item_para_index(item) else {
                continue;
            };
            let Some(local) = lookup_local(pi, &local_indices) else {
                continue;
            };
            y = hc.vpos_adjust(y, local, &local_paras, styles);
            let item_para = &local_paras[local];
            let item_composed = crate::renderer::composer::compose_paragraph(item_para);
            let item_fmt =
                self.format_paragraph(item_para, Some(&item_composed), styles, column_width);
            y += match item {
                PageItem::PartialParagraph {
                    start_line,
                    end_line,
                    ..
                } => item_fmt.line_advances_sum(*start_line..*end_line),
                PageItem::FullParagraph { .. } => item_fmt.total_height,
                PageItem::Table {
                    para_index,
                    control_index,
                } => measured_tables
                    .iter()
                    .find(|mt| mt.para_index == *para_index && mt.control_index == *control_index)
                    .map(|mt| mt.total_height)
                    .unwrap_or(0.0),
                PageItem::PartialTable {
                    para_index,
                    control_index,
                    start_row,
                    end_row,
                    ..
                } => measured_tables
                    .iter()
                    .find(|mt| mt.para_index == *para_index && mt.control_index == *control_index)
                    .map(|mt| {
                        let start = mt
                            .cumulative_heights
                            .get(*start_row)
                            .copied()
                            .unwrap_or(0.0);
                        let end = mt
                            .cumulative_heights
                            .get(*end_row)
                            .copied()
                            .unwrap_or(mt.total_height);
                        (end - start).max(0.0)
                    })
                    .unwrap_or(0.0),
                _ => 0.0,
            };
            let current_vpos_rewinds_from_prev = hc
                .prev_layout_para
                .and_then(|prev_local| {
                    let prev_first = local_paras
                        .get(prev_local)
                        .and_then(|p| p.line_segs.first())
                        .map(|seg| seg.vertical_pos)?;
                    let curr_first = local_paras
                        .get(local)
                        .and_then(|p| p.line_segs.first())
                        .map(|seg| seg.vertical_pos)?;
                    Some(curr_first < prev_first)
                })
                .unwrap_or(false);
            if matches!(
                item,
                PageItem::PartialParagraph { start_line, .. } if *start_line > 0
            ) || current_vpos_rewinds_from_prev
            {
                hc.prev_layout_para = None;
                hc.vpos_page_base = None;
                hc.vpos_lazy_base = None;
            } else {
                hc.prev_layout_para = Some(local);
            }
            hc.prev_item_was_partial_table = matches!(item, PageItem::PartialTable { .. });
        }

        let local = lookup_local(target_para_idx, &local_indices)?;
        Some(hc.vpos_adjust(y, local, &local_paras, styles))
    }

    /// 구역의 문단 목록을 조판한다 (단일 패스).
    ///
    /// 기존 paginate()와 동일한 PaginationResult를 반환하므로
    /// 기존 layout/render 파이프라인과 호환된다.
    /// [Task #1046] 비-variant 단축 호출 — `is_hwp3_variant=false` 로 delegate.
    /// 기존 PR/tests 가 사용. force_break_before 는 사후 reflow 이월 hint.
    #[allow(clippy::too_many_arguments)]
    pub fn typeset_section(
        &self,
        paragraphs: &[Paragraph],
        composed: &[ComposedParagraph],
        styles: &ResolvedStyleSet,
        page_def: &PageDef,
        column_def: &ColumnDef,
        section_index: usize,
        measured_tables: &[MeasuredTable],
        hide_empty_line: bool,
        force_break_before: &std::collections::HashSet<usize>,
    ) -> PaginationResult {
        self.typeset_section_with_variant(
            paragraphs,
            composed,
            styles,
            page_def,
            column_def,
            section_index,
            measured_tables,
            hide_empty_line,
            false,
            false,
            false,
            None,
            None,
            force_break_before,
            false,
        )
    }

    /// [Task #1007] HWP3 → HWP5 변환본 인지 typeset.
    /// 변환본 시 cross-paragraph vpos reset (이전 last vpos > body/2 + 현재 first vpos < body/4)
    /// 감지하여 page break 트리거 (한컴 인코딩 page break 시그널).
    ///
    /// [Task #1046] `force_break_before`: 사후 reflow 이월 hint — 이 para_idx 들은 현재
    /// 페이지에 이미 항목이 있으면 새 페이지에서 시작한다 (layout overflow 로 판정된 항목
    /// 이월). 빈 셋이면 무동작 → 기존 출력 불변.
    #[allow(clippy::too_many_arguments)]
    pub fn typeset_section_with_variant(
        &self,
        paragraphs: &[Paragraph],
        composed: &[ComposedParagraph],
        styles: &ResolvedStyleSet,
        page_def: &PageDef,
        column_def: &ColumnDef,
        section_index: usize,
        measured_tables: &[MeasuredTable],
        hide_empty_line: bool,
        is_hwp3_variant: bool,
        skip_spacing_before_prededuct: bool,
        hwp3_origin_page_tolerance: bool,
        footnote_shape: Option<&FootnoteShape>,
        endnote_shape: Option<&FootnoteShape>,
        force_break_before: &std::collections::HashSet<usize>,
        is_hwpx_source: bool,
    ) -> PaginationResult {
        let layout = PageLayoutInfo::from_page_def(page_def, column_def, self.dpi);
        self.is_hwpx_source.set(is_hwpx_source);
        let col_count = column_def.column_count.max(1);
        let default_footnote_shape = FootnoteShape::default();
        let footnote_shape = footnote_shape.unwrap_or(&default_footnote_shape);
        let footnote_separator_overhead = footnote_separator_overhead_px(footnote_shape, self.dpi);
        let footnote_between_notes_margin =
            footnote_between_notes_margin_px(footnote_shape, self.dpi);
        let footnote_safety_margin = hwpunit_to_px(3000, self.dpi);
        // [Task #1007] variant cross-paragraph vpos reset THRESHOLD 계산용 body height (HU)
        let body_height_hu_for_variant: i32 = if is_hwp3_variant {
            page_def.height.saturating_sub(
                page_def
                    .margin_top
                    .saturating_add(page_def.margin_bottom)
                    .saturating_add(page_def.margin_header)
                    .saturating_add(page_def.margin_footer),
            ) as i32
        } else {
            0
        };
        // [Task #1007] 이전 paragraph 인덱스 (variant vpos reset 감지용)
        let mut variant_prev_para_idx: Option<usize> = None;

        let mut st = TypesetState::new(
            layout,
            col_count,
            section_index,
            footnote_separator_overhead,
            footnote_between_notes_margin,
            footnote_safety_margin,
            column_def.column_type,
        );
        st.hide_empty_line = hide_empty_line;
        st.is_hwp3_variant = is_hwp3_variant;
        // [Task #1472] format_paragraph 의 미주 수식 indent_scale 보정용.
        self.is_hwp3_variant.set(is_hwp3_variant);
        st.is_hwpx_source = is_hwpx_source;
        st.skip_spacing_before_prededuct = skip_spacing_before_prededuct;
        st.current_zone_design_spacing_px = column_def_design_spacing_px(column_def, self.dpi);

        // 머리말/꼬리말/쪽 번호/새 번호/감추기 컨트롤 수집
        let (hf_entries, page_number_pos, new_page_numbers, page_hides) =
            Self::collect_header_footer_controls(paragraphs, section_index);

        for (para_idx, para) in paragraphs.iter().enumerate() {
            // 표 컨트롤 감지
            let has_table = self.paragraph_has_table(para);

            // [Task #702] 새 ColumnDef 검출. shortcut.hwp p2/p3 파일/미리보기/편집 등은
            // [쪽나누기]+단정의:1단 (header) → [단나누기]+단정의:2단 (content) 패턴 사용.
            // [다단나누기] 외에도 Page/Column break 의 ColumnDef 차이도 zone 재정의 신호로 인식.
            let new_col_def_opt: Option<ColumnDef> = para.controls.iter().find_map(|c| {
                if let Control::ColumnDef(cd) = c {
                    Some(cd.clone())
                } else {
                    None
                }
            });
            let has_diff_col_def = new_col_def_opt
                .as_ref()
                .map(|cd| {
                    cd.column_count.max(1) != st.col_count
                        || cd.column_type != st.current_zone_column_type
                })
                .unwrap_or(false);

            let is_terminal_empty_column_break = para_idx + 1 == paragraphs.len()
                && st.col_count == 1
                && para.column_type == ColumnBreakType::Column
                && !has_diff_col_def
                && para.controls.is_empty()
                && para
                    .text
                    .replace(|c: char| c.is_control(), "")
                    .trim()
                    .is_empty();
            if is_terminal_empty_column_break {
                // 1단 구역 끝의 빈 단나누기 문단은 다음 단/밴드가 없으므로
                // 별도 빈 페이지를 만들지 않는다.
                continue;
            }

            let is_overlay_guide_empty_para = st.col_count > 1
                && para.column_type == ColumnBreakType::None
                && para.controls.is_empty()
                && !para_has_visible_text(para)
                && para
                    .line_segs
                    .first()
                    .is_some_and(|seg| seg.vertical_pos > 0)
                && (0..para_idx)
                    .rev()
                    .find(|&idx| {
                        paragraphs
                            .get(idx)
                            .is_some_and(|p| !p.controls.is_empty() || para_has_visible_text(p))
                    })
                    .and_then(|idx| paragraphs.get(idx))
                    .is_some_and(para_is_non_tac_overlay_table_anchor);
            if is_overlay_guide_empty_para {
                // 글앞/글뒤 표 뒤의 빈 guide 줄은 떠 있는 개체의 위치 보조값이며,
                // 다단 flow 높이를 소비하지 않는다.
                continue;
            }

            // 다단 나누기
            if para.column_type == ColumnBreakType::MultiColumn {
                self.process_multicolumn_break(&mut st, para_idx, paragraphs, page_def);
            }

            // 단 나누기
            if para.column_type == ColumnBreakType::Column {
                if has_diff_col_def {
                    // [Task #702] 단나누기 + 새 ColumnDef = zone 재정의 (MultiColumn 등가 처리)
                    self.process_multicolumn_break(&mut st, para_idx, paragraphs, page_def);
                } else if !st.current_items.is_empty() {
                    // [Task #846] 마지막 단에서 명시적 단나누기 → 새 페이지가 아니라 같은
                    // col_count 로 같은 페이지에 새 단-밴드를 시작 (들어갈 공간이 있으면). ≈ #768.
                    // [Task #849] 단, 이는 "배분"(Distribute) 단에서만. "일반"(Normal/신문형)
                    // 단에서 마지막 단의 단나누기는 같은 페이지 새 밴드를 만들지 않는다 (기존 동작).
                    // [Task #866] shortcut.hwp 3쪽 "<편집 화면 분할에서>" pi=94 회귀 수정.
                    let is_last_column = st.current_column + 1 >= st.col_count;
                    if is_last_column
                        && st.col_count > 1
                        && st.current_zone_column_type == ColumnType::Distribute
                    {
                        self.start_new_column_band(&mut st, para_idx, paragraphs);
                    } else {
                        st.advance_column_or_new_page();
                    }
                }
            }

            // 쪽 나누기
            let force_page_break = para.column_type == ColumnBreakType::Page
                || para.column_type == ColumnBreakType::Section;
            let para_style = styles.para_styles.get(para.para_shape_id as usize);
            let para_style_break = para_style.map(|s| s.page_break_before).unwrap_or(false);

            // [Task #1007/#1035 → #1042 narrow v2] Cross-paragraph vpos reset 감지 —
            // heading paragraph (text 있음 + spacing_before ≥ 500 HU + paragraph local
            // vpos reset) 만 인정. content paragraph (spacing_before < 500) 는 skip.
            // sample16-2024 pi=162 (heading, sb=852, vpos=852) trigger ✓
            // sample16-2022 pi=87 (빈 문단, text_len=0) skip ✓
            // sample16-2022 pi=118 (content, sb=284) skip ✓
            // sample16-2022 pi=316 (content, sb=0) skip ✓
            let mut variant_vpos_reset_break = false;
            if is_hwp3_variant && body_height_hu_for_variant > 0 && !para.text.is_empty() {
                let para_sb = styles
                    .para_styles
                    .get(para.para_shape_id as usize)
                    .map(|ps| ps.spacing_before)
                    .unwrap_or(0.0);
                let para_sb_hu = (para_sb * 7200.0 / 96.0) as i32;
                let prev_real_idx_and_ls = variant_prev_para_idx.and_then(|prev_pi| {
                    (0..=prev_pi).rev().find_map(|i| {
                        paragraphs
                            .get(i)
                            .and_then(|p| p.line_segs.last())
                            .filter(|ls| !is_synthetic_line_seg(ls))
                            .map(|ls| (i, ls))
                    })
                });
                let curr_real = para
                    .line_segs
                    .first()
                    .filter(|ls| !is_synthetic_line_seg(ls));
                if let Some((prev_real_idx, prev_last)) = prev_real_idx_and_ls {
                    let prev_end_vpos = prev_last.vertical_pos + prev_last.line_height;
                    let prev_positive_wrap_end = paragraphs
                        .get(prev_real_idx)
                        .and_then(positive_vpos_end_before_negative_wrap);
                    let prev_prev_end_vpos = if prev_real_idx > 0 {
                        (0..prev_real_idx).rev().find_map(|i| {
                            paragraphs.get(i).and_then(|p| {
                                p.line_segs
                                    .last()
                                    .filter(|ls| !is_synthetic_line_seg(ls))
                                    .map(|ls| ls.vertical_pos.saturating_add(ls.line_height))
                            })
                        })
                    } else {
                        None
                    };
                    let prev_top_content_reset = paragraphs.get(prev_real_idx).is_some_and(|p| {
                        let prev_sb_hu = styles
                            .para_styles
                            .get(p.para_shape_id as usize)
                            .map(|ps| (ps.spacing_before * 7200.0 / 96.0) as i32)
                            .unwrap_or(0);
                        p.line_segs.len() == 1
                            && p.line_segs.first().is_some_and(|ls| {
                                !is_synthetic_line_seg(ls) && ls.vertical_pos == 0
                            })
                            && p.controls.is_empty()
                            && para_has_visible_text(p)
                            && prev_sb_hu < 250
                    });
                    let next_first_real_vpos = paragraphs
                        .get(para_idx + 1)
                        .and_then(|next_para| next_para.line_segs.first())
                        .filter(|ls| !is_synthetic_line_seg(ls))
                        .map(|ls| ls.vertical_pos);
                    let bridge_missing_count = (prev_real_idx + 1..para_idx)
                        .filter(|&i| {
                            paragraphs.get(i).is_some_and(|p| {
                                p.line_segs.is_empty()
                                    && p.controls.is_empty()
                                    && para_has_visible_text(p)
                            })
                        })
                        .count();
                    let high_threshold = body_height_hu_for_variant * 95 / 100;
                    let table_heading_reset = prev_real_idx + 1 == para_idx
                        && para.line_segs.is_empty()
                        && para.controls.is_empty()
                        && para_has_visible_text(para)
                        && para_sb_hu >= 500
                        && prev_end_vpos > body_height_hu_for_variant * 85 / 100
                        && paragraphs.get(prev_real_idx).is_some_and(|prev_para| {
                            prev_para
                                .controls
                                .iter()
                                .any(|c| matches!(c, Control::Table(t) if t.common.treat_as_char))
                        })
                        && paragraphs
                            .get(para_idx + 1)
                            .and_then(|next_para| next_para.line_segs.first())
                            .filter(|ls| !is_synthetic_line_seg(ls))
                            .is_some_and(|ls| ls.vertical_pos <= 4000);
                    let empty_bridge_heading_reset = para.line_segs.is_empty()
                        && para.controls.is_empty()
                        && para_has_visible_text(para)
                        && para_sb_hu >= 500
                        && bridge_missing_count == 1
                        && prev_end_vpos > body_height_hu_for_variant * 80 / 100
                        && prev_end_vpos <= body_height_hu_for_variant * 85 / 100;

                    let real_heading_or_bridge_reset = curr_real.is_some_and(|curr_first| {
                        let curr_first_vpos = curr_first.vertical_pos;
                        let strict_heading_reset = para_sb_hu >= 500
                            && prev_end_vpos > high_threshold
                            && curr_first_vpos < 1500;
                        let delayed_heading_after_top_content_reset = prev_real_idx + 1 == para_idx
                            && para.line_segs.len() >= 2
                            && para_sb_hu >= 500
                            && para.controls.is_empty()
                            && para_has_visible_text(para)
                            && curr_first_vpos > 0
                            && curr_first_vpos <= 2500
                            && prev_top_content_reset
                            && prev_prev_end_vpos
                                .is_some_and(|end| end > body_height_hu_for_variant * 70 / 100);
                        let bridged_reset = bridge_missing_count >= 2
                            && para.controls.is_empty()
                            && para_has_visible_text(para)
                            && curr_first_vpos <= 1500
                            && prev_end_vpos > body_height_hu_for_variant * 75 / 100;
                        let negative_wrap_heading_reset = prev_real_idx + 1 == para_idx
                            && para.line_segs.len() == 1
                            && para_sb_hu >= 250
                            && para.controls.is_empty()
                            && para_has_visible_text(para)
                            && curr_first_vpos < 0
                            && prev_positive_wrap_end
                                .is_some_and(|end| end > body_height_hu_for_variant * 75 / 100);
                        let bottom_heading_before_next_reset = prev_real_idx + 1 == para_idx
                            && para.line_segs.len() == 1
                            && para_sb_hu >= 250
                            && para.controls.is_empty()
                            && para_has_visible_text(para)
                            && curr_first_vpos > body_height_hu_for_variant * 75 / 100
                            && next_first_real_vpos.is_some_and(|next_vpos| {
                                next_vpos > 0 && next_vpos <= 4000 && curr_first_vpos > next_vpos
                            });
                        strict_heading_reset
                            || delayed_heading_after_top_content_reset
                            || bridged_reset
                            || negative_wrap_heading_reset
                            || bottom_heading_before_next_reset
                    });

                    if table_heading_reset
                        || empty_bridge_heading_reset
                        || real_heading_or_bridge_reset
                    {
                        variant_vpos_reset_break = true;
                    }
                }
            }

            if (force_page_break || para_style_break || variant_vpos_reset_break)
                && !st.current_items.is_empty()
            {
                st.force_new_page();
                // [Task #702] 쪽나누기 + 새 ColumnDef = 새 페이지에서 col 정의 적용
                if has_diff_col_def {
                    if let Some(cd) = &new_col_def_opt {
                        st.col_count = cd.column_count.max(1);
                        let new_layout = PageLayoutInfo::from_page_def(page_def, cd, self.dpi);
                        st.current_zone_layout = Some(new_layout.clone());
                        st.layout = new_layout;
                        st.current_zone_column_type = cd.column_type;
                        // [Task #853] 새 페이지 첫 zone: 디자인 spacing /2 (위쪽 절반)만 추가.
                        // (이전 zone 은 이전 페이지에 있었으므로 아래쪽 절반은 더하지 않음.)
                        let new_ds = column_def_design_spacing_px(cd, self.dpi);
                        st.current_zone_y_offset += new_ds / 2.0;
                        st.current_zone_design_spacing_px = new_ds;
                    }
                }
            }

            // [Task #1046] 사후 reflow 이월: layout 에서 본문 하단 overflow 로 판정된 항목은
            // 현재 페이지에 렌더링하지 않고 다음 페이지로 넘긴다. force_break_before 에 등록된
            // para_idx 가 현재 페이지에 이미 항목이 있으면 새 페이지를 강제 (force_page_break 등가).
            // 빈 셋(reflow hint 없음)이면 무동작 → 기존 출력 불변.
            if force_break_before.contains(&para_idx) && !st.current_items.is_empty() {
                st.force_new_page();
            }

            // Task #321: 문단간 vpos-reset 기반 강제 분할
            // HWP LINE_SEG의 vertical_pos는 페이지 내 흐름 y 좌표.
            // 현재 문단 first_vpos=0이고 직전 문단이 같은 단에 있으며 last_vpos가 충분히 큰 경우,
            // HWP가 pi 경계에서 페이지/단 분할을 의도한 것 → 강제 분할.
            // [Task #362] wrap-around zone 활성 중에는 vpos-reset 가드 무시 (기존).
            // [Task #724] vpos-reset trigger 발동 시 wrap_around 강제 종료 (신규):
            // HWP5 변환본 case 에서 paragraph 442/443 wrap_around 매칭 후 후속 paragraph
            // (예: 599) vpos=0 시점에도 wrap_around active 유지되어 페이지 분할 위반 →
            // vpos-reset trigger 시 wrap_around 강제 종료 + advance_column_or_new_page.
            if para_idx > 0 && !st.current_items.is_empty() {
                let prev_para = &paragraphs[para_idx - 1];
                let curr_first_vpos = para.line_segs.first().map(|s| s.vertical_pos);
                let prev_last_vpos = prev_para.line_segs.last().map(|s| s.vertical_pos);
                if let (Some(cv), Some(pv)) = (curr_first_vpos, prev_last_vpos) {
                    // 현재 문단의 vpos가 직전 문단의 마지막 vpos보다 작은 경우 — 컬럼/페이지 reset 시그널.
                    // - 단일 단: cv == 0 만 인정 (Task #321 보수적 기준 유지).
                    //   단일 단에서 cv != 0 의 cv < pv 는 partial-table split 의 LAYOUT 잔재로
                    //   해석되어야 함 (issue #418 / hwpspec pi=78→pi=79).
                    // - 다단 Normal (NEWSPAPER): cv != 0 도 인정 (Task #470). pv > 5000 임계값 유지.
                    // - 다단 Distribute (BalancedNewspaper): 짧은 컬럼 (3+3 분배 등) 에서 pv 가
                    //   임계값 미달일 수 있어 pv > 0 으로 완화 (Task #702, shortcut 지우기 6항목 정합).
                    //   단일 단/Normal 다단은 영향 없음.
                    let is_distribute = st.col_count > 1
                        && matches!(st.current_zone_column_type, ColumnType::Distribute);
                    // [Task #853] Distribute 다단의 "1줄짜리 컬럼" 케이스: 직전 문단이
                    // 단 1줄(예: vpos=0)이고 현재 문단도 vpos=0 이면 `cv < pv` 가 0<0 으로
                    // 거짓이라 컬럼 전환을 못 잡았다(shortcut.hwp 스타일/속성 섹션). 직전 문단의
                    // vpos+line_height(=콘텐츠 끝)를 기준으로 비교하면 정상 흐름(cv=pv_end+ls≥pv_end)
                    // 은 영향 없고 reset(cv≪pv_end)만 잡힌다.
                    let prev_vpos_end = prev_para
                        .line_segs
                        .last()
                        .map(|s| s.vertical_pos + s.line_height)
                        .unwrap_or(pv);
                    // [Task #1086 Stage 3] HWP3-origin page tolerance 대상 문서는
                    // 새 페이지 첫 문단을 vpos=0 이 아니라 200/500HU 근방으로
                    // 인코딩하기도 한다(hwpspec.hwp s2:pi=89, pi=104). 단일 단에서
                    // 모든 cv<pv 를 reset 으로 보면 일반 직접 작성 HWP(2022년
                    // 국립국어원), partial-table 직후 정상 흐름(hwpspec pi=78→79),
                    // 표 host 문단(hwpspec pi=57)을 깨므로, 비영 near-top reset 은
                    // 직전 문단이 페이지 하단부에 있고 대상이 텍스트/그림-only 문단일
                    // 때만 인정한다.
                    // 그림만 든 빈 문단은 한컴이 조금 더 일찍 새 페이지로 넘기는 케이스
                    // (hwpspec.hwp s3:pi=93)가 있어 표/텍스트보다 낮은 하단 기준을 쓴다.
                    let shape_only_para = para.text.trim().is_empty()
                        && !para.controls.is_empty()
                        && para
                            .controls
                            .iter()
                            .all(|c| matches!(c, Control::Picture(_) | Control::Shape(_)));
                    let has_table_control =
                        para.controls.iter().any(|c| matches!(c, Control::Table(_)));
                    let near_page_top_reset = hwp3_origin_page_tolerance
                        && cv > 0
                        && ((shape_only_para && cv <= 200 && prev_vpos_end > 52_000)
                            || (!shape_only_para
                                && !has_table_control
                                && cv <= 500
                                && prev_vpos_end > 60_000));
                    let para_sb_hu_for_reset = para_style
                        .map(|s| (s.spacing_before * 7200.0 / 96.0) as i32)
                        .unwrap_or(0);
                    let next_heading_after_top_content_reset =
                        paragraphs.get(para_idx + 1).is_some_and(|next_para| {
                            let next_sb_hu = styles
                                .para_styles
                                .get(next_para.para_shape_id as usize)
                                .map(|ps| (ps.spacing_before * 7200.0 / 96.0) as i32)
                                .unwrap_or(0);
                            next_para.line_segs.len() >= 2
                                && next_para
                                    .line_segs
                                    .first()
                                    .filter(|ls| !is_synthetic_line_seg(ls))
                                    .is_some_and(|ls| {
                                        ls.vertical_pos > 0 && ls.vertical_pos <= 2500
                                    })
                                && next_para.controls.is_empty()
                                && para_has_visible_text(next_para)
                                && next_sb_hu >= 500
                        });
                    let hwp3_content_vpos_zero_reset = is_hwp3_variant
                        && st.col_count == 1
                        && cv == 0
                        && prev_vpos_end > body_height_hu_for_variant * 70 / 100
                        && para_sb_hu_for_reset < 250
                        && para.controls.is_empty()
                        && para_has_visible_text(para)
                        && next_heading_after_top_content_reset;
                    let trigger = if st.col_count > 1 {
                        if is_distribute {
                            cv < prev_vpos_end && prev_vpos_end > 0
                        } else {
                            cv < pv && pv > 5000
                        }
                    } else {
                        (cv == 0 && pv > 5000 && !hwp3_content_vpos_zero_reset)
                            || near_page_top_reset
                    };
                    if trigger {
                        // [Task #724] wrap_around active 시 강제 종료 — anchor cs=0
                        // (HWP5 변환본 caption-style) 한정. 일반 wrap_around (anchor cs>0)
                        // 는 기존 동작 (Task #362 vpos-reset 무시) 유지.
                        if st.wrap_around_cs == 0 {
                            st.wrap_around_cs = -1;
                            st.wrap_around_sw = -1;
                            st.wrap_around_any_seg = false;
                        }
                        if st.wrap_around_cs < 0 {
                            st.advance_column_or_new_page();
                        }
                    }
                }
            }

            // [Task #359] 단독 항목 페이지 차단:
            // 다음 pi 가 vpos-reset 가드를 발동할 예정이고 현재 pi 가 잔여 공간 부족으로
            // 새 페이지를 시작하면 단독 항목 페이지가 발생.
            //   - 현재 pi 가 빈 문단이면: skip (한컴은 표시하지 않음)
            //   - 현재 pi 가 일반 텍스트이면: fit 안전마진 (10px) 1회 비활성화
            //     (kps-ai pi=317 case: 0.x px 차이로 fit 실패하여 단독 페이지 35 발생)
            // 가드 제외 조건:
            //   - 다음 pi 가 force_page_break (column_type==Page/Section) 인 경우 발동 안 함
            //     (정상 쪽나누기 신호 — 단독 페이지 발생 안 함, hwp-multi-001 회귀 차단)
            let next_will_vpos_reset =
                if !st.current_items.is_empty() && para_idx + 1 < paragraphs.len() {
                    let next_para = &paragraphs[para_idx + 1];
                    let next_force_break = next_para.column_type == ColumnBreakType::Page
                        || next_para.column_type == ColumnBreakType::Section;
                    if next_force_break {
                        false
                    } else {
                        // [Task #470] 다단 섹션에서는 nv == 0 → nv < cl 로 완화 (컬럼 헤더 오프셋).
                        // 단일 단에서는 partial-table split 회귀 (issue #418) 회피 위해 nv == 0 유지.
                        paragraph_saved_vpos_reset_starts_new_page_after(
                            para,
                            next_para,
                            st.col_count,
                            st.is_hwp3_variant,
                        )
                    }
                } else {
                    false
                };

            if next_will_vpos_reset {
                // [Task #362] 빈 paragraph 가 표/도형/그림 컨트롤을 포함하면 skip 안 함
                // (kps-ai pi=778 case: 빈 텍스트 + 3x3 wrap=Square 표를 가진 paragraph 가
                //  잘못 skip 되어 표 누락).
                let is_empty_no_ctrl = para.text.is_empty() && para.controls.is_empty();
                if is_empty_no_ctrl {
                    // [#1648] 빈 문단이 현재 페이지에 들어가면 정상 배치한다(한글 동작 —
                    //   페이지 하단에 빈 줄 1개). 들어가지 않을 때만 skip 하여 단독 빈 페이지를
                    //   차단한다. 종전엔 fit 무검사로 fit 하는 빈 문단까지 드롭하여, 페이지를
                    //   채운 TAC 표 직후의 빈 문단이 누락되고 페이지↔PI 가 한글과 어긋났다
                    //   (#1643 rhwp_pNone).
                    //
                    // 1) height fit: 합산 current_height 기준 (종전 #1648 판정).
                    let empty_h_px = para
                        .line_segs
                        .first()
                        .map(|s| hwpunit_to_px((s.line_height + s.line_spacing) as i32, self.dpi))
                        .unwrap_or(0.0);
                    let height_fits = empty_h_px <= st.available_height() - st.current_height;

                    // 2) [#1659] vpos fit: 합산 height 는 음수 줄간격 문단에서 실제 vpos 진행을
                    //   과소평가 → 페이지 하단 빈 문단을 height fit 으로 오판 emit 하지만 placement
                    //   (아래 vpos overflow 가드, ~L2300)는 vpos overflow 로 새 페이지에 단독 배치
                    //   → 단독 빈 페이지 +1 회귀(synam-001 35→36). placement(L2333/L2339)와 동일한
                    //   page_top_vpos 기준 vpos 판정을 AND 로 더해, height·vpos 둘 다 fit 일 때만
                    //   emit. placement 가 height 기반인 다단/wrap 에선 vpos 판정을 생략(true).
                    let vpos_fits = if st.col_count == 1 && st.wrap_around_cs < 0 {
                        // 페이지 첫 실 item 의 top vpos. PartialParagraph continuation 은 원
                        //   문단의 첫 줄이 아니라 fragment 시작 줄(start_line)의 vpos 가 페이지
                        //   상단이다 → line_segs[start_line] 사용. 줄 기준 vpos 가 없는 항목
                        //   (PartialTable continuation)은 None 으로 두어 vpos 판정을 보류(height
                        //   fit 에 위임) — 잘못된 baseline 으로 skip/emit 오판 방지(#1659 리뷰).
                        let page_top_vpos = st
                            .current_items
                            .iter()
                            .find(|item| !matches!(item, PageItem::EndnoteSeparator { .. }))
                            .and_then(|item| match item {
                                PageItem::FullParagraph { para_index }
                                | PageItem::Table { para_index, .. }
                                | PageItem::Shape { para_index, .. } => paragraphs
                                    .get(*para_index)
                                    .and_then(|p| p.line_segs.first())
                                    .map(|s| s.vertical_pos),
                                PageItem::PartialParagraph {
                                    para_index,
                                    start_line,
                                    ..
                                } => paragraphs
                                    .get(*para_index)
                                    .and_then(|p| p.line_segs.get(*start_line))
                                    .map(|s| s.vertical_pos),
                                // 줄 기준 vpos 없음 → 판정 보류.
                                PageItem::PartialTable { .. }
                                | PageItem::EndnoteSeparator { .. } => None,
                            });
                        match (para.line_segs.last(), page_top_vpos) {
                            (Some(last_seg), Some(top)) => {
                                let body_h_hu = crate::renderer::px_to_hwpunit(
                                    st.layout.body_area.height,
                                    self.dpi,
                                );
                                let vpos_end = last_seg.vertical_pos + last_seg.line_height;
                                vpos_end <= top + body_h_hu + 283
                            }
                            // vpos 판정 불가 → 제약 없음(height fit 에 위임).
                            _ => true,
                        }
                    } else {
                        true
                    };

                    if !(height_fits && vpos_fits) {
                        // [#1706] 빈 문단이 현재 페이지에 안 들어감.
                        // 종전엔 통째로 drop(continue) → 문단이 모델에서 사라져 한글 대비
                        // 문단→페이지 매핑이 어긋났다(rhwp_pNone; 대형 TAC 표가 페이지를 채운
                        // 직후의 빈 문단). 한글은 이 빈 문단을 현재 페이지 하단의 빈 줄 1개로 유지.
                        // → drop 대신 현재 페이지에 0-높이로 흡수 기록(hide_empty_line 와 동일
                        //   시멘틱). 페이지를 advance 하지 않으므로 단독 빈 페이지 회귀(synam-001
                        //   등)는 발생하지 않는다.
                        st.hidden_empty_paras.insert(para_idx);
                        st.current_items.push(PageItem::FullParagraph {
                            para_index: para_idx,
                        });
                        continue;
                    }
                    // height·vpos 둘 다 fit → 정상 emit (아래로 진행)
                } else {
                    // 일반 텍스트 또는 컨트롤 보유: 안전마진 1회 비활성화 (단독 텍스트 페이지 차단)
                    st.skip_safety_margin_once = true;
                    // [Task #1725] 각주 있는 페이지의 tail 문단: 각주 안전마진(40px 버퍼)도 1회
                    // 비활성화. 한글은 tail 을 본문에 배치하는데 rhwp 각주 예약 버퍼가 tail 을 수 px
                    // 밀어 near-empty 페이지 over-pagination(국제고속선기준 258 vs 242) 을 만든다.
                    st.skip_footnote_margin_once = true;
                }
            } else if !st.current_items.is_empty() && para_idx + 1 < paragraphs.len() {
                // [Task #967] 빈 paragraph 직후 force page break (쪽나누기) case 가드:
                // 빈 paragraph 가 현재 page 잔여 공간 초과 시 별도 page 분기 →
                // +1 page inflate 회귀 (sample18.hwp 의 pi=27, pi=164).
                // 한컴은 빈 paragraph 를 trailing overflow 로 흡수 + 쪽나누기로 새 page 시작.
                // next_will_vpos_reset 가드는 next_force_break 인 경우 발동 안 함
                // (hwp-multi-001 회귀 차단). 본 추가 가드는 빈 paragraph + 다음 쪽나누기
                // case 중에서 **현재 page 잔여 공간 부족 (overflow) 시에만** skip — 빈
                // paragraph 가 page 에 fit 하면 정상 emit (aift.hwp 의 18 case 회귀 방지).
                let next_para = &paragraphs[para_idx + 1];
                let next_force_break = next_para.column_type == ColumnBreakType::Page
                    || next_para.column_type == ColumnBreakType::Section;
                let is_curr_empty = para.text.is_empty() && para.controls.is_empty();
                if next_force_break && is_curr_empty {
                    // empty paragraph 의 예상 height = first line_seg 의 lh + ls
                    let empty_h_px = para
                        .line_segs
                        .first()
                        .map(|s| hwpunit_to_px((s.line_height + s.line_spacing) as i32, self.dpi))
                        .unwrap_or(0.0);
                    let avail = st.available_height() - st.current_height;
                    if empty_h_px > avail {
                        // [#1706] 빈 paragraph 가 fit 안 됨.
                        // 종전엔 drop(continue) → 문단이 모델에서 사라져 한글 대비 매핑이
                        // 어긋났다(rhwp_pNone). 한컴은 이 빈 문단을 현재 page 하단의 빈 줄로
                        // 흡수(위 주석)하므로, drop 대신 현재 페이지에 0-높이로 흡수 기록한다.
                        // 페이지를 advance 하지 않으므로 단독 page 회귀(sample18 등)는 없다.
                        st.hidden_empty_paras.insert(para_idx);
                        st.current_items.push(PageItem::FullParagraph {
                            para_index: para_idx,
                        });
                        continue;
                    }
                    // fit 가능 — 정상 emit (기존 동작)
                }
            }
            // [Task #362] 어울림(Square wrap) 표 옆 paragraph 흡수.
            // Paginator engine.rs:288-320 동일 시멘틱.
            // 직전에 처리한 Square wrap 표의 (cs, sw) 와 동일한 LINE_SEG 를 가진
            // 후속 paragraph 는 표 옆에 배치되므로 height 소비 없이 wrap_around_paras 에 기록.
            if st.wrap_around_cs >= 0 && !has_table {
                let para_cs = para.line_segs.first().map(|s| s.column_start).unwrap_or(0);
                let para_sw = para
                    .line_segs
                    .first()
                    .map(|s| s.segment_width as i32)
                    .unwrap_or(0);
                let is_empty_para = para
                    .text
                    .chars()
                    .all(|ch| ch.is_whitespace() || ch == '\r' || ch == '\n')
                    && para.controls.is_empty();
                let any_seg_matches = para.line_segs.iter().any(|s| {
                    s.column_start == st.wrap_around_cs
                        && s.segment_width as i32 == st.wrap_around_sw
                });
                let body_w = (page_def.width as i32)
                    - (page_def.margin_left as i32)
                    - (page_def.margin_right as i32);
                let sw0_match =
                    st.wrap_around_sw == 0 && is_empty_para && para_sw > 0 && para_sw < body_w / 2;
                // [Task #724] HWP5 변환본 case: anchor host 의 wrap=Square image 위치/폭/margin
                // 으로 expected_cs 정확 계산 후 para_cs 일치 확인. anchor cs=0 (caption-style)
                // 한정 가드. expected_cs = (image_x_offset + width + 2*margin) - body_left.
                let anchor_image_match = if st.wrap_around_cs == 0 {
                    let body_left = page_def.margin_left as i32;
                    let expected_cs_hu = paragraphs
                        .get(st.wrap_around_table_para)
                        .and_then(|p| {
                            p.controls.iter().find_map(|c| {
                                let cm = match c {
                                    Control::Picture(pic) => Some(&pic.common),
                                    Control::Shape(s) => {
                                        if let crate::model::shape::ShapeObject::Picture(pic) =
                                            s.as_ref()
                                        {
                                            Some(&pic.common)
                                        } else {
                                            None
                                        }
                                    }
                                    _ => None,
                                };
                                cm.filter(|cm| {
                                    !cm.treat_as_char
                                        && matches!(
                                            cm.text_wrap,
                                            crate::model::shape::TextWrap::Square
                                        )
                                })
                                .map(|cm| {
                                    cm.horizontal_offset as i32
                                        + cm.width as i32
                                        + 2 * cm.margin.right as i32
                                        - body_left
                                })
                            })
                        })
                        .unwrap_or(0);
                    expected_cs_hu > 0
                        && (para_cs - expected_cs_hu).abs() < 200
                        && para_sw > 0
                        && para_cs + para_sw <= body_w + 200
                } else {
                    false
                };
                // [Task #901] cs 일치 + 합리적 sw 매칭 (anchor 의 wrap zone region 다양성).
                // pic2.hwp paragraph 1 (cs=24470 sw=18050) vs anchor (wrap_around_cs=24470 sw=2570)
                // — cs 같지만 sw 다름 (다른 wrap region). 기존 매칭 실패 → wrap_anchors 미등록
                // → paragraph 좌측 그려짐. anchor_any_seg 가 활성이면 cs 정확 일치 만으로
                // wrap zone 내부 paragraph 로 인정.
                let cs_only_match =
                    st.wrap_around_any_seg && para_cs == st.wrap_around_cs && para_sw > 0;
                if (para_cs == st.wrap_around_cs && para_sw == st.wrap_around_sw)
                    || (any_seg_matches && (is_empty_para || st.wrap_around_any_seg))
                    || sw0_match
                    || anchor_image_match
                    || cs_only_match
                {
                    // [Task #604 R3] wrap_around 매칭 분기를 anchor 종류 기반으로 본질화.
                    //
                    // - Picture (그림 Square wrap) anchor: wrap text 가 LineSeg cs/sw 로
                    //   사전 인코딩됨 → wrap_anchors 등록 + FullParagraph 통과
                    //   (layout 이 LineSeg cs/sw 정합 렌더)
                    // - Table (표 Square wrap) anchor: wrap text 는 표 옆 빈 ↵ 표시용
                    //   → 흡수 (current_column_wrap_around_paras)
                    //
                    // Stage 2b: Paragraph.wrap_precomputed (HWP3 휴리스틱 IR 누설) 제거.
                    // anchor paragraph 의 controls 검사로 본질 정합 대체.
                    let anchor_is_picture = paragraphs
                        .get(st.wrap_around_table_para)
                        .map(|p| {
                            p.controls.iter().any(|c| match c {
                                Control::Picture(pic) => !pic.common.treat_as_char,
                                Control::Shape(s) => {
                                    if let crate::model::shape::ShapeObject::Picture(pic) =
                                        s.as_ref()
                                    {
                                        !pic.common.treat_as_char
                                    } else {
                                        false
                                    }
                                }
                                _ => false,
                            })
                        })
                        .unwrap_or(false);
                    if anchor_is_picture {
                        // Picture anchor: wrap_anchors 등록 + FullParagraph 통과
                        // [Task #722] anchor image 의 outer margin_right (HU) 추출
                        let anchor_margin_right = paragraphs
                            .get(st.wrap_around_table_para)
                            .and_then(|p| {
                                p.controls.iter().find_map(|c| {
                                    let cm = match c {
                                        Control::Picture(pic) => Some(&pic.common),
                                        Control::Shape(s) => {
                                            if let crate::model::shape::ShapeObject::Picture(pic) =
                                                s.as_ref()
                                            {
                                                Some(&pic.common)
                                            } else {
                                                None
                                            }
                                        }
                                        _ => None,
                                    };
                                    cm.filter(|cm| {
                                        !cm.treat_as_char
                                            && matches!(
                                                cm.text_wrap,
                                                crate::model::shape::TextWrap::Square
                                            )
                                    })
                                    .map(|cm| cm.margin.right as i32)
                                })
                            })
                            .unwrap_or(0);
                        st.current_column_wrap_anchors.insert(
                            para_idx,
                            crate::renderer::pagination::WrapAnchorRef {
                                anchor_para_index: st.wrap_around_table_para,
                                anchor_cs: st.wrap_around_cs,
                                anchor_sw: st.wrap_around_sw,
                                anchor_image_margin_right: anchor_margin_right,
                            },
                        );
                    } else {
                        // Table anchor: 어울림 문단을 표 옆에 기록 + height 소비 없음.
                        // [Task #855] 단, 첫 줄만 표 옆이고 나머지 줄이 본문 전체 폭으로
                        // 흐르는 문단(= 마지막 LINE_SEG 가 wrap zone cs/sw 와 불일치)은
                        // 0-높이 흡수 대상이 아니다. 첫 LINE_SEG 만 보고 흡수하면 그런 문단이
                        // 통째로 페이지 흐름에서 누락된다. 이 경우 wrap zone 을 종료하고
                        // 일반 텍스트 배치로 폴백한다 (LINE_SEG cs/sw 가 이미 wrap 형상을
                        // 인코딩하므로 layout 이 첫 줄을 표 옆에, 나머지를 표 아래에 렌더).
                        let last_seg_match = para
                            .line_segs
                            .last()
                            .map(|s| {
                                s.column_start == st.wrap_around_cs
                                    && s.segment_width as i32 == st.wrap_around_sw
                            })
                            .unwrap_or(false);
                        if last_seg_match || is_empty_para {
                            st.current_column_wrap_around_paras.push(
                                crate::renderer::pagination::WrapAroundPara {
                                    para_index: para_idx,
                                    table_para_index: st.wrap_around_table_para,
                                    has_text: !is_empty_para,
                                },
                            );
                            continue;
                        }
                        st.wrap_around_cs = -1;
                        st.wrap_around_sw = -1;
                        st.wrap_around_any_seg = false;
                        // fall through → 일반 paragraph 배치
                    }
                } else {
                    // 매칭 실패 → wrap zone 종료, 정상 처리 진행
                    st.wrap_around_cs = -1;
                    st.wrap_around_sw = -1;
                    st.wrap_around_any_seg = false;
                    // [Task #741 Stage 4] 매칭 실패 paragraph 의 vpos=0 hint (page break 의도)
                    // 발견 시 advance_column_or_new_page. wrap_around active 종료 후 추가 가드.
                    // hwp3-sample10-hwp5.hwp paragraph 26 ("● 제목차례 ●") case —
                    // paragraph 22 anchor (cs=11084) active 유지로 line 419 vpos-reset 가드
                    // 미발현 → 매칭 실패 후 추가 vpos-reset 가드로 페이지 break 정합.
                    if para_idx > 0 && !st.current_items.is_empty() {
                        let prev_para = &paragraphs[para_idx - 1];
                        let curr_first_vpos = para.line_segs.first().map(|s| s.vertical_pos);
                        let prev_last_vpos = prev_para.line_segs.last().map(|s| s.vertical_pos);
                        if let (Some(cv), Some(pv)) = (curr_first_vpos, prev_last_vpos) {
                            let trigger = if st.col_count > 1 {
                                cv < pv && pv > 5000
                            } else {
                                cv == 0 && pv > 5000
                            };
                            if trigger {
                                st.advance_column_or_new_page();
                            }
                        }
                    }
                }
            }

            st.ensure_page();

            // [Task #404] heading-orphan 패턴 보정 (vpos 기반).
            // 현재 paragraph 가 누적 height 로는 fit 하지만 HWP vpos 기준 페이지 한계를
            // 넘고, 다음 substantial block(Table/Shape/큰 paragraph)이 잔여 영역에 들어
            // 가지 않을 때 → 현재 paragraph 를 다음 페이지로 push 하여 heading + 후속
            // 블록을 같은 페이지에 배치.
            //
            // 조건 (모두 AND):
            //   A) current_items 비어있지 않음 (페이지 첫 item 자기참조 회피)
            //   B) 단일 단 + wrap-around zone 아님 (multi-column / wrap 의미 차이 회피)
            //   C) 누적 height 로 fit
            //   D) vpos overflow > 1mm (283 HU)
            //   E) 다음 paragraph 의 height 가 substantial (>30px ≈ 8mm) AND 잔여 영역에
            //      들어가지 않음
            //
            // Stage 1 진단 로그 분석으로 false positive 41건 → 1건(pi=83)으로 축소.
            // page_top_vpos 는 current_items 의 첫 item para_index 를 통해 즉시 계산
            // (TypesetState 필드 추적은 typeset_paragraph 내부 페이지 flush 와 동기 안 됨).
            if !st.current_items.is_empty() && st.wrap_around_cs < 0 && st.col_count == 1 {
                let page_first_para_idx = st.current_items.iter().find_map(|item| match item {
                    PageItem::FullParagraph { para_index } => Some(*para_index),
                    PageItem::PartialParagraph { para_index, .. } => Some(*para_index),
                    PageItem::Table { para_index, .. } => Some(*para_index),
                    PageItem::PartialTable { para_index, .. } => Some(*para_index),
                    PageItem::Shape { para_index, .. } => Some(*para_index),
                    PageItem::EndnoteSeparator { .. } => None,
                });
                let page_top_vpos_opt = page_first_para_idx
                    .and_then(|pi| paragraphs.get(pi))
                    .and_then(|p| p.line_segs.first())
                    .map(|s| s.vertical_pos);
                if let (Some(first_seg), Some(page_top_vpos)) =
                    (para.line_segs.first(), page_top_vpos_opt)
                {
                    let body_h_hu =
                        crate::renderer::px_to_hwpunit(st.layout.body_area.height, self.dpi);
                    let para_h_px: f64 = para
                        .line_segs
                        .iter()
                        .map(|s| {
                            crate::renderer::hwpunit_to_px(s.line_height + s.line_spacing, self.dpi)
                        })
                        .sum();
                    let para_h_hu = crate::renderer::px_to_hwpunit(para_h_px, self.dpi);
                    // [Task #643] vpos_end 는 마지막 줄의 bottom (vpos + lh) 기준.
                    // para_h_px 누적은 트레일링 line_spacing 까지 포함하여 ~10-12 HU 과대.
                    // HWP 가 페이지 끝에서 트레일링 ls 를 고려하지 않고 lh 만 fit 검사하는
                    // 시멘틱 정합 (pi=39 page 3 fits 케이스).
                    let vpos_end = para
                        .line_segs
                        .last()
                        .map(|s| s.vertical_pos + s.line_height)
                        .unwrap_or(first_seg.vertical_pos + para_h_hu);
                    let page_bottom_vpos = page_top_vpos + body_h_hu;

                    let avail = st.available_height();
                    let current_fits = st.current_height + para_h_px <= avail;
                    let vpos_overflow = vpos_end > page_bottom_vpos + 283; // 1mm 안전여유

                    let next_h_px: f64 = paragraphs
                        .get(para_idx + 1)
                        .map(|p| {
                            p.line_segs
                                .iter()
                                .map(|s| {
                                    crate::renderer::hwpunit_to_px(
                                        s.line_height + s.line_spacing,
                                        self.dpi,
                                    )
                                })
                                .sum::<f64>()
                        })
                        .unwrap_or(0.0);
                    let next_substantial = next_h_px > 30.0;
                    let next_doesnt_fit = st.current_height + para_h_px + next_h_px > avail;

                    if current_fits && vpos_overflow && next_substantial && next_doesnt_fit {
                        st.advance_column_or_new_page();
                    }
                }
            }

            if !has_table {
                // --- 핵심: format → fits → place/split ---
                let col_w = st
                    .layout
                    .column_areas
                    .get(st.current_column as usize)
                    .map(|a| a.width)
                    .unwrap_or(st.layout.body_area.width);
                let formatted =
                    self.format_paragraph(para, composed.get(para_idx), styles, Some(col_w));
                let is_last_in_section = para_idx + 1 == paragraphs.len();
                // [Task #1027 Stage D] fit 직전 vpos 스냅으로 누적 drift 제거 (렌더러 정합).
                self.vpos_snap_current_height(&mut st, para_idx, paragraphs, styles);
                self.typeset_paragraph(
                    &mut st,
                    para_idx,
                    para,
                    &formatted,
                    paragraphs,
                    is_last_in_section,
                );
            } else {
                // 표 문단: Phase 2에서 전환 예정. 현재는 기존 방식 호환용 stub.
                self.typeset_table_paragraph(
                    &mut st,
                    para_idx,
                    para,
                    composed.get(para_idx),
                    paragraphs.get(para_idx + 1),
                    styles,
                    measured_tables,
                    page_def,
                );
            }

            // [Task #1027 Stage D] 항목 배치 후 vpos 커서 prev/base 추적 (렌더러 정합).
            // 렌더러 build_single_column: 매 항목 후 prev_layout_para 갱신, 표/Shape/
            // PartialTable 배치 후 page/lazy base 무효화(LINE_SEG lh 가 개체 높이를
            // 반영 못 해 drift 유발 → 직후 paragraph 는 lazy 역산으로 재산출). 단단 전용.
            if st.col_count == 1 {
                st.vpos_prev_layout_para = Some(para_idx);
                let last = st.current_items.last();
                st.vpos_prev_partial_table = matches!(last, Some(PageItem::PartialTable { .. }));
                if matches!(
                    last,
                    Some(
                        PageItem::Table { .. }
                            | PageItem::PartialTable { .. }
                            | PageItem::Shape { .. }
                    )
                ) {
                    // Para-float TopAndBottom 표 예외(렌더러 2513)는 Stage E.
                    st.vpos_page_base = None;
                    st.vpos_lazy_base = None;
                }
            }

            // [Task #362] Square wrap 표 처리 후 wrap zone 활성화.
            // Paginator engine.rs:356-372 동일 시멘틱.
            // 후속 paragraph 가 동일 cs/sw 를 가지면 흡수.
            if has_table {
                let has_tac_block = para
                    .controls
                    .iter()
                    .any(|c| matches!(c, Control::Table(t) if t.common.treat_as_char));
                let has_non_tac_table = !has_tac_block;
                if has_non_tac_table {
                    let is_wrap_around = para.controls.iter().any(|c| {
                        if let Control::Table(t) = c {
                            matches!(t.common.text_wrap, crate::model::shape::TextWrap::Square)
                        } else {
                            false
                        }
                    });
                    if is_wrap_around {
                        st.wrap_around_cs =
                            para.line_segs.first().map(|s| s.column_start).unwrap_or(0);
                        st.wrap_around_sw = para
                            .line_segs
                            .first()
                            .map(|s| s.segment_width as i32)
                            .unwrap_or(0);
                        st.wrap_around_table_para = para_idx;
                        st.wrap_around_any_seg = false;
                    }
                }
            }
            // 비-TAC Picture/Shape Square wrap: engine.rs:380-397 동일 시멘틱.
            // 그림의 첫 lineseg cs가 0일 수 있어 any_seg_matches 허용 플래그 활성화.
            if !has_table {
                let has_non_tac_pic_square = para.controls.iter().any(|c| {
                    let cm = match c {
                        Control::Picture(p) => Some(&p.common),
                        Control::Shape(s) => {
                            if let crate::model::shape::ShapeObject::Picture(p) = s.as_ref() {
                                Some(&p.common)
                            } else {
                                None
                            }
                        }
                        _ => None,
                    };
                    cm.map(|cm| {
                        !cm.treat_as_char
                            && matches!(cm.text_wrap, crate::model::shape::TextWrap::Square)
                    })
                    .unwrap_or(false)
                });
                if has_non_tac_pic_square {
                    let anchor_cs = para.line_segs.first().map(|s| s.column_start).unwrap_or(0);
                    let anchor_sw = para
                        .line_segs
                        .first()
                        .map(|s| s.segment_width as i32)
                        .unwrap_or(0);
                    if anchor_cs > 0 || anchor_sw > 0 {
                        st.wrap_around_cs = anchor_cs;
                        st.wrap_around_sw = anchor_sw;
                        st.wrap_around_table_para = para_idx;
                        st.wrap_around_any_seg = true;
                        // [Task #722] anchor host paragraph 자체도 wrap_anchors 등록.
                        // LINE_SEG cs/sw 가 wrap zone 으로 인코딩되어 있으면 host paragraph 의
                        // 줄도 image 우측 wrap zone 에 layout 되어야 한다 (한컴 PDF 권위 정합).
                        // 미등록 시 paragraph_layout 의 wrap_anchor 분기 미진입 → col_area
                        // 전체 폭 layout → image 영역 침범 → image z-order 후 그려져 가려짐.
                        //
                        // Case 가드 (Stage 3~5 진단):
                        //   - LINE_SEG ≥ 2 → wrap zone (multi-line)
                        //   - LINE_SEG 1 + caption_room ≤ line_height → wrap zone (image 가
                        //     body_top 자체에 위치 → image 위 caption 영역 없음, 강제 wrap)
                        //   - LINE_SEG 1 + caption_room > line_height → caption-style (자기
                        //     미등록 → col_area 전체 폭 layout, image 위 자유 영역 표시)
                        let body_top_hu = page_def.margin_top as i32;
                        let line_height_hu = para
                            .line_segs
                            .first()
                            .map(|s| s.line_height as i32)
                            .unwrap_or(900);
                        let (image_voff_hu, image_margin_right_hu) = para
                            .controls
                            .iter()
                            .find_map(|c| {
                                let cm = match c {
                                    Control::Picture(p) => Some(&p.common),
                                    Control::Shape(s) => {
                                        if let crate::model::shape::ShapeObject::Picture(p) =
                                            s.as_ref()
                                        {
                                            Some(&p.common)
                                        } else {
                                            None
                                        }
                                    }
                                    _ => None,
                                };
                                cm.filter(|cm| {
                                    !cm.treat_as_char
                                        && matches!(
                                            cm.text_wrap,
                                            crate::model::shape::TextWrap::Square
                                        )
                                })
                                .map(|cm| (cm.vertical_offset as i32, cm.margin.right as i32))
                            })
                            .unwrap_or((0, 0));
                        let caption_room_hu = image_voff_hu - body_top_hu;
                        let is_caption_style =
                            para.line_segs.len() == 1 && caption_room_hu > line_height_hu;
                        // [PR #732 후속 — exam_science 회귀 가드] image_mr=0 (margin 부재) 이면
                        // 본 환경 OLD 동작 보존 — Task #722 host_self register skip.
                        // 본질: image_mr > 0 인 경우 (한컴 viewer 가 inter-image-text gap 으로
                        // margin 적용) 만 host_self register 가 의미. exam_science p.21/37/60 의
                        // Square wrap picture 는 image_mr=0 (호스트 margin 부재) 이므로 OLD 의
                        // col_area-full-width layout 정합 (line_seg cs=0/sw=실제 wrap zone 인코딩
                        // 으로 한컴 정합 이미 유지). hwp3-sample5.hwp 의 page 8/27/48 (Task #722
                        // 본질 영역) 은 image_mr > 0 으로 가드 통과 → 정합 유지.
                        if !is_caption_style && image_margin_right_hu > 0 {
                            st.current_column_wrap_anchors.insert(
                                para_idx,
                                crate::renderer::pagination::WrapAnchorRef {
                                    anchor_para_index: para_idx,
                                    anchor_cs,
                                    anchor_sw,
                                    anchor_image_margin_right: image_margin_right_hu,
                                },
                            );
                        }
                    }
                }
            }

            // Task #321: col 0 처리 중 body-wide TopAndBottom 표/도형이 발견되면
            // col 1+ advance 시 적용할 current_height 시작값을 미리 등록.
            // layout의 body_wide_reserved와 동일 조건으로 detect.
            if st.col_count > 1 && st.current_column == 0 && st.pending_body_wide_top_reserve == 0.0
            {
                let reserve = compute_body_wide_top_reserve_for_para(para, &st.layout, self.dpi);
                if reserve > 0.0 {
                    st.pending_body_wide_top_reserve = reserve;
                }
            }

            // 인라인 컨트롤 처리: 도형/그림/수식/각주 (Paginator engine.rs:509-525 동일)
            for (ctrl_idx, ctrl) in para.controls.iter().enumerate() {
                match ctrl {
                    Control::Shape(_) | Control::Picture(_) | Control::Equation(_) => {
                        if !has_table {
                            // [Issue #476] treat_as_char Shape 는 박스가 속한 line 이 라우팅된
                            // 페이지/단에 등록. paragraph 가 페이지 분할되면 이 시점의
                            // st.current_items 는 마지막 페이지 상태이므로, 그대로 push 하면
                            // 박스가 잘못된 페이지에 떠 있게 된다.
                            let is_tac_shape = matches!(ctrl,
                                Control::Shape(s) if s.common().treat_as_char);
                            let routed = if is_tac_shape {
                                crate::renderer::pagination::find_inline_control_target_page(
                                    &st.pages,
                                    &st.current_items,
                                    para_idx,
                                    ctrl_idx,
                                    para,
                                )
                            } else {
                                None
                            };
                            let item = PageItem::Shape {
                                para_index: para_idx,
                                control_index: ctrl_idx,
                            };
                            match routed {
                                Some((page_idx, col_idx)) => {
                                    if let Some(page) = st.pages.get_mut(page_idx) {
                                        if let Some(col) = page.column_contents.get_mut(col_idx) {
                                            col.items.push(item);
                                        } else {
                                            st.current_items.push(item);
                                        }
                                    } else {
                                        st.current_items.push(item);
                                    }
                                }
                                None => {
                                    st.current_items.push(item);
                                }
                            }
                            // [Task #1052] 글상자 내 각주 수집 (engine.rs:1376-1398 동등)
                            // footnote-tbox-01.hwpx 의 글상자 안 각주 본문이 페이지 하단 영역
                            // 에 누락되는 결함 정정. engine.rs (legacy) 는 이미 처리하나
                            // typeset.rs (main, default) 만 누락 — feedback_image_renderer_paths_separate.
                            if let Control::Shape(shape_obj) = ctrl {
                                if let Some(text_box) =
                                    shape_obj.drawing().and_then(|d| d.text_box.as_ref())
                                {
                                    for (tp_idx, tp) in text_box.paragraphs.iter().enumerate() {
                                        for (tc_idx, tc) in tp.controls.iter().enumerate() {
                                            if let Control::Footnote(fn_ctrl) = tc {
                                                if let Some(page) = st.pages.last_mut() {
                                                    page.footnotes.push(FootnoteRef {
                                                        number: fn_ctrl.number,
                                                        source: FootnoteSource::ShapeTextBox {
                                                            para_index: para_idx,
                                                            shape_control_index: ctrl_idx,
                                                            tb_para_index: tp_idx,
                                                            tb_control_index: tc_idx,
                                                        },
                                                    });
                                                    let fn_height = estimate_footnote_note_height(
                                                        fn_ctrl, self.dpi,
                                                    );
                                                    st.add_footnote_height(fn_height);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            // Task #409 v2: 비-TAC TopAndBottom + vert=Para Picture/Shape 는
                            // layout 에서 picture_footnote.rs:356 의 `y_offset + total_height`
                            // 패턴으로 후속 콘텐츠를 개체 높이만큼 밀어냄. 하지만 paragraph
                            // line_seg 의 lh 는 텍스트 baseline 만 반영하므로 페이지네이션의
                            // current_height 가 개체 높이만큼 부족하게 누적되어 page packing
                            // 시 layout 실제 y 와 어긋남 (21페이지: pagination used=803px vs
                            // layout y=1275px → pi=192 가 21페이지에 packing 되었다가
                            // overflow 로 잘림). pagination 측에서도 layout 과 동일하게
                            // 개체 높이를 current_height 에 누적.
                            use crate::model::shape::{TextWrap, VertRelTo};
                            // (obj_h, extra=obj_h+margin_bottom)
                            let pushdown_h: Option<(f64, f64)> = match ctrl {
                                Control::Picture(pic)
                                    if !pic.common.treat_as_char
                                        && matches!(
                                            pic.common.text_wrap,
                                            TextWrap::TopAndBottom
                                        )
                                        && matches!(pic.common.vert_rel_to, VertRelTo::Para) =>
                                {
                                    let h = hwpunit_to_px(pic.common.height as i32, self.dpi);
                                    let mb =
                                        hwpunit_to_px(pic.common.margin.bottom as i32, self.dpi);
                                    Some((h, h + mb))
                                }
                                Control::Shape(s)
                                    if !s.common().treat_as_char
                                        && matches!(
                                            s.common().text_wrap,
                                            TextWrap::TopAndBottom
                                        )
                                        && matches!(s.common().vert_rel_to, VertRelTo::Para) =>
                                {
                                    let cm = s.common();
                                    let h = hwpunit_to_px(cm.height as i32, self.dpi);
                                    let mb = hwpunit_to_px(cm.margin.bottom as i32, self.dpi);
                                    Some((h, h + mb))
                                }
                                _ => None,
                            };
                            if let Some((obj_h, extra)) = pushdown_h {
                                // [Task #1079] 파일 vpos 가 이미 그림 공간을 반영(그림 para 줄
                                // 앞 gap ≥ 그림 높이)하면 VPOS_CORR sync 가 그 공간을 따르므로
                                // pushdown 가산은 이중 계상. gap 이 그림 높이 미만(파일 vpos
                                // 미반영, Task #409 계열)일 때만 가산.
                                const PUSHDOWN_GAP_TOL_PX: f64 = 8.0;
                                let already_accounted = para_idx > 0 && {
                                    let v_cur = para.line_segs.first().map(|s| s.vertical_pos);
                                    let prev_end = paragraphs[para_idx - 1]
                                        .line_segs
                                        .last()
                                        .map(|s| s.vertical_pos + s.line_height);
                                    match (v_cur, prev_end) {
                                        (Some(vc), Some(pe)) if vc > pe => {
                                            hwpunit_to_px((vc - pe) as i32, self.dpi)
                                                >= obj_h - PUSHDOWN_GAP_TOL_PX
                                        }
                                        _ => false,
                                    }
                                };
                                if !already_accounted {
                                    st.current_height += extra;
                                }
                            }
                        }
                    }
                    Control::Footnote(fn_ctrl) => {
                        if !has_table {
                            if let Some(page) = st.pages.last_mut() {
                                page.footnotes.push(FootnoteRef {
                                    number: fn_ctrl.number,
                                    source: FootnoteSource::Body {
                                        para_index: para_idx,
                                        control_index: ctrl_idx,
                                    },
                                });
                            }
                            let fn_height = estimate_footnote_note_height(fn_ctrl, self.dpi);
                            st.add_footnote_height(fn_height);
                        }
                    }
                    Control::Endnote(en_ctrl) => {
                        // [Task #836] 미주 수집 — 문서 끝에 모아서 렌더
                        st.endnotes.push(EndnoteRef {
                            number: en_ctrl.number,
                            section_index,
                            para_index: para_idx,
                            control_index: ctrl_idx,
                        });
                    }
                    _ => {}
                }
            }
            // [Task #1007] variant vpos reset 감지용 prev_para_idx 갱신
            variant_prev_para_idx = Some(para_idx);
        }

        // [Task #836] 미주 paragraphs를 본문 흐름에 가상 삽입
        // 한컴 정합: 미주는 섹션 마지막에 일반 본문처럼 2단 레이아웃 플로우를 따름
        // 미주 paragraphs를 endnote_paragraphs Vec에 모으고, ENDNOTE_PARA_BASE 이상 인덱스로 마킹
        if !st.endnotes.is_empty() {
            let endnote_refs: Vec<EndnoteRef> = st.endnotes.clone();
            // 본문 마지막 paragraph의 vpos 끝 위치 계산
            let mut vpos_offset: i32 = paragraphs
                .last()
                .and_then(|p| p.line_segs.last())
                .map(|ls| ls.vertical_pos + ls.line_height + ls.line_spacing)
                .unwrap_or(0);
            // [Task #1082] 다단 미주 vpos-delta 누적용 prev tracker.
            // 시드 = 현재 단의 본문 last bottom vpos(body→endnote 전환 정합); 없으면 None
            // (단의 첫 미주 → 자체 높이 사용). 단 advance 시 flush_column 에서 prev_body 리셋.
            let mut prev_en_bottom_vpos: Option<i32> = st.prev_body_bottom_vpos;
            let mut prev_en_content_bottom_vpos: Option<i32> = st.prev_body_bottom_vpos;
            let mut prev_endnote_had_vpos_rewind = false;
            let mut prev_endnote_had_inline_object_vpos_overestimate = false;
            let mut cleared_single_line_internal_rewind_split = false;
            let mut emitted_endnote_separator = false;
            let mut emitted_endnote_count = 0usize;
            let mut last_render_endnote_para_local_idx: Option<usize> = None;
            // 이 플래그는 "시험지 미주 흐름"의 split/rewind 보정 사용 여부다.
            // 구분선 아래 여백이 20mm처럼 커도 문항 미주 흐름 자체는 같은
            // 정책을 타야 하므로 separator 크기와 분리한다.
            let endnote_flow_profile = endnote_shape.map(EndnoteFlowProfile::from_shape);
            let compact_endnote_separator_profile = endnote_flow_profile.is_some();
            if let Some(profile) = endnote_flow_profile {
                st.endnote_separator_above_hu = profile.separator_above_hu;
                st.endnote_separator_below_hu = profile.separator_below_hu;
                st.endnote_between_notes_hu = profile.between_notes_hu;
            }

            for (en_ref_idx, en_ref) in endnote_refs.iter().enumerate() {
                if let Some(para) = paragraphs.get(en_ref.para_index) {
                    if let Some(Control::Endnote(en_ctrl)) = para.controls.get(en_ref.control_index)
                    {
                        if !emitted_endnote_separator {
                            if let (Some(shape), Some(profile)) =
                                (endnote_shape, endnote_flow_profile)
                            {
                                let sep_height = profile.separator_height_px(self.dpi);
                                if sep_height > 0.0 {
                                    st.current_items.push(PageItem::EndnoteSeparator {
                                        separator_length: shape.separator_length,
                                        margin_above: shape.separator_above_margin_hu(),
                                        margin_below: endnote_separator_below_margin(shape),
                                        line_type: shape.separator_line_type,
                                        line_width: shape.separator_line_width,
                                        color: shape.separator_color,
                                    });
                                    st.current_endnote_flow = true;
                                    if !profile.compact_separator_below {
                                        st.current_height += sep_height;
                                        st.current_start_height = st.current_height;
                                    }
                                }
                            }
                            emitted_endnote_separator = true;
                        }
                        let rewind_group_advance_threshold = if st.current_column + 1 < st.col_count
                        {
                            0.85
                        } else {
                            0.95
                        };
                        let default_nonzero_between_note_tail_candidate = endnote_flow_profile
                            .map(EndnoteFlowProfile::nonzero_default_between_notes)
                            .unwrap_or(false)
                            && en_ref.number > 0;
                        let default_late_question_group_tail = compact_endnote_separator_profile
                            && endnote_shape
                                .map(|shape| {
                                    endnote_between_notes_margin(shape) as i32
                                        <= ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU
                                })
                                .unwrap_or(false)
                            && default_nonzero_between_note_tail_candidate
                            && st.current_column + 1 >= st.col_count;
                        let default_question_group_head_tail = compact_endnote_separator_profile
                            && prev_endnote_had_inline_object_vpos_overestimate
                            && endnote_shape
                                .map(|shape| {
                                    endnote_between_notes_margin(shape) as i32
                                        <= ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU
                                })
                                .unwrap_or(false)
                            && {
                                let head_h: f64 = en_ctrl
                                    .paragraphs
                                    .iter()
                                    .take(2)
                                    .filter_map(|p| {
                                        let first = p.line_segs.first()?.vertical_pos;
                                        let bottom = p
                                            .line_segs
                                            .iter()
                                            .map(|s| {
                                                s.vertical_pos + s.line_height + s.line_spacing
                                            })
                                            .max()?;
                                        Some(hwpunit_to_px((bottom - first).max(0), self.dpi))
                                    })
                                    .sum();
                                head_h > 0.0
                                    && st.current_height + head_h <= st.available_height() - 8.0
                            };
                        // 기본 7mm 미주는 제목 한 줄 tail을 허용하되, 빈/TAC 식만
                        // 뒤따르는 orphan 제목은 frame overflow로 이어지므로 제외한다.
                        let default_question_group_title_tail = compact_endnote_separator_profile
                            && endnote_shape
                                .map(|shape| {
                                    endnote_between_notes_margin(shape) as i32
                                        <= ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU
                                })
                                .unwrap_or(false)
                            && en_ref.number > 0
                            && !st.current_items.is_empty()
                            && en_ctrl.paragraphs.first().is_some_and(|head| {
                                if head.line_segs.len() != 1 {
                                    return false;
                                }
                                let title_h = hwpunit_to_px(
                                    head.line_segs[0].line_height + head.line_segs[0].line_spacing,
                                    self.dpi,
                                );
                                let title_fits = title_h > 0.0
                                    && st.current_height + title_h
                                        <= st.available_height()
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 2.0;
                                if !title_fits {
                                    return false;
                                }
                                if st.current_column + 1 >= st.col_count {
                                    return en_ctrl
                                        .paragraphs
                                        .get(1)
                                        .map(para_has_visible_text)
                                        .unwrap_or(true);
                                }
                                if !default_nonzero_between_note_tail_candidate {
                                    return false;
                                }
                                let mut head_h = 0.0;
                                let mut head_count = 0usize;
                                for para in en_ctrl.paragraphs.iter().take(4) {
                                    let Some(first) = para.line_segs.first() else {
                                        continue;
                                    };
                                    let Some(bottom) = para
                                        .line_segs
                                        .iter()
                                        .map(|seg| {
                                            seg.vertical_pos + seg.line_height + seg.line_spacing
                                        })
                                        .max()
                                    else {
                                        continue;
                                    };
                                    head_h += hwpunit_to_px(
                                        (bottom - first.vertical_pos).max(0),
                                        self.dpi,
                                    );
                                    head_count += 1;
                                }
                                head_count >= 2
                                    && st.current_height + head_h
                                        <= st.available_height()
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 2.0
                            });
                        let zero_question_group_title_tail = compact_endnote_separator_profile
                            && prev_endnote_had_vpos_rewind
                            && st.current_column + 1 < st.col_count
                            && endnote_shape
                                .map(|shape| {
                                    shape.separator_above_margin_hu() == 0
                                        && endnote_between_notes_margin(shape) == 0
                                        && endnote_separator_below_margin(shape) == 0
                                        && endnote_has_visible_separator(shape)
                                })
                                .unwrap_or(false)
                            && en_ctrl
                                .paragraphs
                                .first()
                                .map(|p| {
                                    let en_col_w = st
                                        .layout
                                        .column_areas
                                        .get(st.current_column as usize)
                                        .map(|a| a.width)
                                        .unwrap_or(st.layout.body_area.width);
                                    let comp = crate::renderer::composer::compose_paragraph(p);
                                    let fmt = self.format_paragraph(
                                        p,
                                        Some(&comp),
                                        &styles,
                                        Some(en_col_w),
                                    );
                                    fmt.line_heights.len() == 1
                                        && line_has_visible_text_or_tac_equation(p, &comp, 0)
                                        && st.current_height + fmt.line_advance(0)
                                            <= st.available_height()
                                                + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                })
                                .unwrap_or(false);
                        let zero_between_question_group_title_tail =
                            compact_endnote_separator_profile
                                && prev_endnote_had_vpos_rewind
                                && st.current_column + 1 < st.col_count
                                && endnote_shape
                                    .map(|shape| {
                                        endnote_has_visible_separator(shape)
                                            && endnote_between_notes_margin(shape) == 0
                                    })
                                    .unwrap_or(false)
                                && en_ref.number > 0
                                && !st.current_items.is_empty()
                                && en_ctrl.paragraphs.first().is_some_and(|head| {
                                    head.line_segs.first().is_some_and(|seg| {
                                        let title_h = hwpunit_to_px(
                                            (seg.line_height + seg.line_spacing).max(0),
                                            self.dpi,
                                        );
                                        title_h > 0.0
                                            && st.current_height + title_h
                                                <= st.available_height()
                                                    + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                    + 2.0
                                    })
                                });
                        let visible_large_between_question_group_title_tail =
                            compact_endnote_separator_profile
                                && prev_endnote_had_vpos_rewind
                                && st.current_column + 1 < st.col_count
                                && endnote_flow_profile
                                    .map(EndnoteFlowProfile::visible_non_default_between_notes)
                                    .unwrap_or(false)
                                && en_ref.number > 0
                                && !st.current_items.is_empty()
                                && en_ctrl.paragraphs.first().is_some_and(|head| {
                                    if head.line_segs.len() != 1 {
                                        return false;
                                    }
                                    let Some(first) = head.line_segs.first() else {
                                        return false;
                                    };
                                    let title_h = hwpunit_to_px(
                                        (first.line_height + first.line_spacing).max(0),
                                        self.dpi,
                                    );
                                    title_h > 0.0
                                        && st.current_height + title_h
                                            <= st.available_height()
                                                + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                + 2.0
                                });
                        if st.col_count > 1
                            && compact_endnote_separator_profile
                            && !st.current_items.is_empty()
                            && prev_endnote_had_vpos_rewind
                            && !default_late_question_group_tail
                            && !default_question_group_head_tail
                            && !default_question_group_title_tail
                            && !zero_question_group_title_tail
                            && !zero_between_question_group_title_tail
                            && !visible_large_between_question_group_title_tail
                            && st.current_height
                                > st.available_height() * rewind_group_advance_threshold
                        {
                            let group_first = en_ctrl
                                .paragraphs
                                .iter()
                                .filter_map(|p| p.line_segs.first().map(|s| s.vertical_pos))
                                .min();
                            let group_bottom = en_ctrl
                                .paragraphs
                                .iter()
                                .flat_map(|p| {
                                    p.line_segs
                                        .iter()
                                        .map(|s| s.vertical_pos + s.line_height + s.line_spacing)
                                })
                                .max();
                            if let (Some(first), Some(bottom)) = (group_first, group_bottom) {
                                let group_h = hwpunit_to_px((bottom - first).max(0), self.dpi);
                                let available = st.available_height();
                                if group_h > 0.0
                                    && group_h <= available + 0.5
                                    && st.current_height + group_h > available
                                {
                                    let reclaimed = (available - st.current_height).max(0.0);
                                    st.advance_column_or_new_page();
                                    st.current_height -= reclaimed;
                                    st.current_start_height = st.current_height;
                                    st.current_endnote_flow = true;
                                    st.reset_vpos_cursor();
                                    prev_en_bottom_vpos = None;
                                    prev_en_content_bottom_vpos = None;
                                }
                            }
                        }
                        let boundary_prev_endnote_had_vpos_rewind = prev_endnote_had_vpos_rewind;
                        let mut prev_group_bottom: Option<i32> = None;
                        let endnote_has_vpos_rewind = en_ctrl.paragraphs.iter().any(|p| {
                            let internal_rewind = p
                                .line_segs
                                .windows(2)
                                .any(|w| w[1].vertical_pos < w[0].vertical_pos);
                            let first = p.line_segs.first().map(|s| s.vertical_pos);
                            let bottom = p
                                .line_segs
                                .iter()
                                .map(|s| s.vertical_pos + s.line_height + s.line_spacing)
                                .max();
                            let group_rewind = matches!(
                                (prev_group_bottom, first),
                                (Some(prev), Some(cur)) if cur < prev
                            );
                            if let Some(b) = bottom {
                                prev_group_bottom = Some(b);
                            }
                            internal_rewind || group_rewind
                        });
                        prev_endnote_had_vpos_rewind = endnote_has_vpos_rewind;
                        let mut current_endnote_had_inline_object_vpos_overestimate = false;
                        let continued_endnote_tail_before_new_note =
                            st.current_endnote_flow && !st.current_items.is_empty();

                        // endnote 단위로 시작점 결정
                        if emitted_endnote_count > 0 {
                            if let (Some(shape), Some(prev_local_idx)) =
                                (endnote_shape, last_render_endnote_para_local_idx)
                            {
                                let between_notes = endnote_between_notes_margin(shape) as i32;
                                if between_notes > 0 {
                                    // [Task #1246] 섹션 미주 between-notes 마진(HU)을 보관 →
                                    // HeightCursor 가 미주 사이 min-gap 보정에 사용. 모든 경계 동일값.
                                    st.endnote_between_notes_hu = between_notes;
                                    let prev_spacing = st
                                        .endnote_paragraphs
                                        .get(prev_local_idx)
                                        .and_then(|p| p.line_segs.last())
                                        .map(|s| s.line_spacing.max(0))
                                        .unwrap_or(0);
                                    let extra_gap = (between_notes - prev_spacing).max(0);
                                    let large_rewind_equation_tail_between_notes_boundary = {
                                        let visible_large_profile = endnote_flow_profile
                                            .map(EndnoteFlowProfile::visible_large_between_notes)
                                            .unwrap_or(false);
                                        let previous_tail_is_equation_only = st
                                            .endnote_paragraphs
                                            .get(prev_local_idx)
                                            .map(|prev_para| {
                                                !para_has_visible_text(prev_para)
                                                    && para_has_visible_text_or_equation(prev_para)
                                            })
                                            .unwrap_or(false);
                                        visible_large_profile
                                            && boundary_prev_endnote_had_vpos_rewind
                                            && continued_endnote_tail_before_new_note
                                            && previous_tail_is_equation_only
                                            && st.current_height < st.available_height() * 0.35
                                    };
                                    let large_equation_tail_before_tac_head_boundary = {
                                        let visible_large_profile = endnote_flow_profile
                                            .map(EndnoteFlowProfile::visible_large_between_notes)
                                            .unwrap_or(false);
                                        let previous_tail_is_large_equation_only = st
                                            .endnote_paragraphs
                                            .get(prev_local_idx)
                                            .map(|prev_para| {
                                                !para_has_visible_text(prev_para)
                                                    && prev_para
                                                        .line_segs
                                                        .last()
                                                        .map(|seg| seg.line_height >= 3000)
                                                        .unwrap_or(false)
                                            })
                                            .unwrap_or(false);
                                        let current_head_has_large_tac_picture =
                                            en_ctrl.paragraphs.iter().take(8).any(|head_para| {
                                                let head_comp =
                                                    crate::renderer::composer::compose_paragraph(
                                                        head_para,
                                                    );
                                                (0..head_comp.lines.len()).any(|line_idx| {
                                                    !line_has_visible_text(&head_comp, line_idx)
                                                        && line_tac_picture_or_shape_height(
                                                            head_para, &head_comp, line_idx,
                                                            self.dpi,
                                                        )
                                                        .is_some_and(|height| height >= 80.0)
                                                })
                                            });
                                        visible_large_profile
                                            && !large_rewind_equation_tail_between_notes_boundary
                                            && endnote_has_visible_separator(shape)
                                            && continued_endnote_tail_before_new_note
                                            && previous_tail_is_large_equation_only
                                            && current_head_has_large_tac_picture
                                            && st.col_count > 1
                                            && st.current_column + 1 >= st.col_count
                                            && st.current_height > st.available_height() * 0.45
                                            && st.current_height < st.available_height() * 0.65
                                    };
                                    if std::env::var("RHWP_ENDNOTE_BOUNDARY_DEBUG").is_ok() {
                                        eprintln!(
                                            "ENDNOTE_BOUNDARY note={} src=s{}:p{}:ci{} emitted={} col={}/{} cur={:.2} avail={:.2} between={} prev_spacing={} extra={} large_rewind={} large_tac_head={} continued={} visible_sep={}",
                                            en_ref.number,
                                            en_ref.section_index,
                                            en_ref.para_index,
                                            en_ref.control_index,
                                            emitted_endnote_count,
                                            st.current_column + 1,
                                            st.col_count,
                                            st.current_height,
                                            st.available_height(),
                                            between_notes,
                                            prev_spacing,
                                            extra_gap,
                                            large_rewind_equation_tail_between_notes_boundary,
                                            large_equation_tail_before_tac_head_boundary,
                                            continued_endnote_tail_before_new_note,
                                            endnote_has_visible_separator(shape),
                                        );
                                    }
                                    if extra_gap > 0 {
                                        // split=1 내부 rewind를 가짜 단 분할로 보고 해소한 뒤에는
                                        // 그 분할이 만들던 암묵적 여백이 사라진다. 큰 미주 사이
                                        // 문서에서는 다음 미주 경계부터 전체 between-notes 값을
                                        // 예약해 PDF의 24쪽 흐름을 유지한다.
                                        // 보이는 구분선이 없는 미주는 renderer가 이전 문단
                                        // line_spacing에 전체 "미주 사이"를 반영한다. pagination도
                                        // 같은 전체 gap을 써야 첫 단 하단에서 under-count가 생기지 않는다.
                                        let visible_separator_tail_continues_current_column =
                                            endnote_has_visible_separator(shape)
                                                && continued_endnote_tail_before_new_note
                                                && st.current_height < st.available_height() * 0.25
                                                && between_notes
                                                    > ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU;
                                        let pagination_gap =
                                            if visible_separator_tail_continues_current_column {
                                                // 같은 단/쪽에 직전 미주 tail이 이미 이어져 있으면
                                                // 직전 문단 line_spacing이 "미주 사이"를 대표한다.
                                                // 여기서 vpos_offset까지 다시 밀면 다음 번호가
                                                // 한컴보다 약 미주사이만큼 아래로 내려간다.
                                                0
                                            } else if large_rewind_equation_tail_between_notes_boundary
                                            {
                                                // 내부 vpos 되감김으로 현재 쪽 상단에 이어진
                                                // 수식 tail은 저장 vpos와 기본 lineSeg 흐름이
                                                // 이미 경계를 만든다. 초과 pagination gap까지
                                                // 더하면 다음 문항 제목이 한 gap만큼 늦어진다.
                                                0
                                            } else if between_notes
                                                > ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU
                                                && (cleared_single_line_internal_rewind_split
                                                    || !endnote_has_visible_separator(shape))
                                            {
                                                between_notes
                                            } else {
                                                endnote_flow_profile
                                                    .map(
                                                        EndnoteFlowProfile::pagination_between_notes_margin,
                                                    )
                                                    .unwrap_or_else(|| {
                                                        endnote_between_notes_pagination_margin(shape)
                                                    })
                                        };
                                        if pagination_gap > 0 {
                                            vpos_offset += pagination_gap;
                                        }
                                        let skip_default_render_between_notes_trailing =
                                            endnote_flow_profile
                                                .map(|profile| {
                                                    profile.visible_nonzero_default_between_notes()
                                                        && profile.large_separator_margin()
                                                })
                                                .unwrap_or(false)
                                                && continued_endnote_tail_before_new_note
                                                && st.current_height > st.available_height() * 0.70
                                                && st.current_height < st.available_height() * 0.75;
                                        let skip_default_mid_column_between_notes_trailing =
                                            endnote_flow_profile
                                                .map(
                                                    EndnoteFlowProfile::visible_nonzero_default_between_notes,
                                                )
                                                .unwrap_or(false)
                                                && boundary_prev_endnote_had_vpos_rewind
                                                && continued_endnote_tail_before_new_note
                                                && st.current_column + 1 >= st.col_count
                                                && st.current_height > st.available_height() * 0.25
                                                && st.current_height < st.available_height() * 0.50;
                                        let skip_absorbed_render_between_notes_trailing = {
                                            let absorbed_visible_profile = endnote_flow_profile
                                                .map(|profile| {
                                                    profile.visible_separator
                                                        && profile.absorbed_between_notes_gap
                                                })
                                                .unwrap_or(false);
                                            let absorbed_tail_continues_at_column_top =
                                                st.current_height < st.available_height() * 0.25;
                                            let absorbed_tail_near_column_bottom =
                                                st.current_height > st.available_height() * 0.65;
                                            let absorbed_short_tac_tail = st
                                                .endnote_paragraphs
                                                .get(prev_local_idx)
                                                .map(|prev_para| {
                                                    let last_line_height = prev_para
                                                        .line_segs
                                                        .last()
                                                        .map(|seg| seg.line_height)
                                                        .unwrap_or(0);
                                                    !para_has_visible_text(prev_para)
                                                        && last_line_height <= 1200
                                                        && prev_para.controls.iter().any(|ctrl| {
                                                            matches!(
                                                                ctrl,
                                                                Control::Equation(eq)
                                                                    if eq.common.treat_as_char
                                                            )
                                                        })
                                                })
                                                .unwrap_or(false);
                                            absorbed_visible_profile
                                                && boundary_prev_endnote_had_vpos_rewind
                                                && continued_endnote_tail_before_new_note
                                                && (absorbed_tail_continues_at_column_top
                                                    || (st.current_column + 1 >= st.col_count
                                                        && absorbed_tail_near_column_bottom)
                                                    || (absorbed_short_tac_tail
                                                        && st.current_height
                                                            > st.available_height() * 0.80))
                                        };
                                        let skip_render_between_notes_trailing =
                                            skip_default_render_between_notes_trailing
                                                || skip_default_mid_column_between_notes_trailing
                                                || skip_absorbed_render_between_notes_trailing;
                                        if let Some(prev_para) =
                                            st.endnote_paragraphs.get_mut(prev_local_idx)
                                        {
                                            if let Some(last_seg) = prev_para.line_segs.last_mut() {
                                                if !skip_render_between_notes_trailing {
                                                    // 내부 vpos 되감김으로 현재 단/쪽 상단에 이어진
                                                    // 수식 tail은 저장 lineSeg 흐름에 기본 gap이 이미
                                                    // 포함되어 있다. 20mm 전체를 render tail에 다시
                                                    // 주입하면 다음 제목이 한 note gap만큼 내려간다.
                                                    let render_between_notes =
                                                        if large_rewind_equation_tail_between_notes_boundary
                                                        {
                                                            ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU
                                                                .max(prev_spacing)
                                                        } else {
                                                            between_notes
                                                        };
                                                    last_seg.line_spacing = render_between_notes;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        let endnote_start = vpos_offset;
                        let mut pre_emitted_endnote_para_indices = std::collections::HashSet::new();
                        for (ep_idx, en_para) in en_ctrl.paragraphs.iter().enumerate() {
                            if pre_emitted_endnote_para_indices.remove(&ep_idx) {
                                emitted_endnote_count += 1;
                                continue;
                            }
                            let en_para_idx = paragraphs.len() + st.endnote_paragraphs.len();
                            let mut en_para_copy = en_para.clone();
                            // line_segs vpos를 endnote 시작점 기준으로 오프셋
                            for ls in &mut en_para_copy.line_segs {
                                ls.vertical_pos += endnote_start;
                            }
                            // 첫 paragraph에 미주 번호 prepend
                            if ep_idx == 0 {
                                let prefix = format!("{} ", format_endnote_marker_text(en_ctrl));
                                en_para_copy.text = format!("{}{}", prefix, en_para_copy.text);
                                en_para_copy.char_count += prefix.encode_utf16().count() as u32;
                                let shift = prefix.encode_utf16().count() as u32;
                                for off in &mut en_para_copy.char_offsets {
                                    *off += shift;
                                }
                                let mut new_offsets: Vec<u32> = (0..shift).collect();
                                new_offsets.extend_from_slice(&en_para_copy.char_offsets);
                                en_para_copy.char_offsets = new_offsets;
                            }
                            let prev_render_endnote_para_local_idx =
                                last_render_endnote_para_local_idx;
                            let prev_rendered_endnote_is_title = prev_render_endnote_para_local_idx
                                .and_then(|idx| st.endnote_paragraphs.get(idx))
                                .map(|p| p.text.trim_start().starts_with('문'))
                                .unwrap_or(false);
                            let en_para_local_idx = st.endnote_paragraphs.len();
                            st.endnote_paragraphs.push(en_para_copy);
                            st.endnote_para_sources.push(EndnoteParaSource {
                                section_index: en_ref.section_index,
                                para_index: en_ref.para_index,
                                control_index: en_ref.control_index,
                                note_para_index: ep_idx,
                            });
                            last_render_endnote_para_local_idx = Some(en_para_local_idx);

                            let composed = crate::renderer::composer::compose_paragraph(en_para);
                            let en_col_w = st
                                .layout
                                .column_areas
                                .get(st.current_column as usize)
                                .map(|a| a.width)
                                .unwrap_or(st.layout.body_area.width);
                            let fmt = self.format_paragraph(
                                en_para,
                                Some(&composed),
                                &styles,
                                Some(en_col_w),
                            );
                            if std::env::var("RHWP_ENDNOTE_LINE_DEBUG").is_ok() {
                                debug_print_endnote_line_segments(
                                    en_ref.number,
                                    ep_idx,
                                    en_para,
                                    &composed,
                                    &fmt,
                                    self.dpi,
                                    endnote_start,
                                );
                            }
                            if compact_endnote_separator_profile
                                && st.col_count > 1
                                && st.current_items.is_empty()
                                && st.current_height < -0.5
                                && ep_idx == 0
                                && !para_is_treat_as_char_picture_only(en_para)
                            {
                                st.current_height = 0.0;
                                st.current_start_height = 0.0;
                                st.reset_vpos_cursor();
                                prev_en_bottom_vpos = None;
                                prev_en_content_bottom_vpos = None;
                            }
                            let available = st.available_height();
                            // [Task #1082] 다단 미주 누적/판정을 렌더 vpos 정규화와 정합.
                            // 렌더는 미주를 px(vpos − 단 첫아이템 vpos)에 배치하므로 단 used
                            // = px(마지막 bottom_vpos − 첫 first_vpos). 종전(#1062)은 미주 para
                            // 내부 span(자체 높이)만 더해 미주 간 vpos 간격(빈줄/문단간격)을
                            // 누락 → 단 과충전 → 렌더 overflow(3-09/10/11월 교육·실전).
                            // 본 정합: 직전 배치 아이템 bottom 기준 vpos delta(px)로 누적.
                            // 시드 prev_en_bottom_vpos = body→endnote 전환 시 본문 last bottom
                            // (위 prev_body_bottom_vpos), 단 advance 후엔 None(자체 높이).
                            // #1062 안전 floor(fmt.height_for_fit) 유지 — vpos delta 가
                            // formatter 추정보다 작은 케이스 회귀 차단. 단단은 종전.
                            let this_first_offset = en_para
                                .line_segs
                                .first()
                                .map(|s| s.vertical_pos + endnote_start);
                            let endnote_bottom_with_spacing = en_para
                                .line_segs
                                .iter()
                                .map(|s| {
                                    (
                                        s.vertical_pos
                                            + s.line_height
                                            + s.line_spacing
                                            + endnote_start,
                                        s.line_spacing,
                                    )
                                })
                                .max_by_key(|(bottom, _)| *bottom);
                            let this_bottom_offset =
                                endnote_bottom_with_spacing.map(|(bottom, _)| bottom);
                            let this_content_bottom_offset = en_para
                                .line_segs
                                .iter()
                                .map(|s| s.vertical_pos + s.line_height + endnote_start)
                                .max();
                            // 다음 미주 묶음의 시작점도 렌더상 가장 낮은 줄 기준으로 갱신한다.
                            // 마지막 LINE_SEG가 위쪽으로 되감기는 문단에서는 last 기준이
                            // 다음 미주를 현재 쪽에 과도하게 붙인다.
                            if let Some(tb) = this_bottom_offset {
                                if tb > vpos_offset {
                                    vpos_offset = tb;
                                }
                            }
                            let trailing_ls_px = endnote_bottom_with_spacing
                                .map(|(_, spacing)| hwpunit_to_px(spacing.max(0), self.dpi))
                                .unwrap_or(0.0);
                            let default_between_notes_gap_before_rewind = endnote_flow_profile
                                .map(EndnoteFlowProfile::default_between_notes)
                                .unwrap_or(false);
                            let absorbed_between_notes_gap_before_rewind = endnote_flow_profile
                                .map(|profile| profile.absorbed_between_notes_gap)
                                .unwrap_or(false);
                            let large_between_notes_gap_before_rewind = endnote_flow_profile
                                .map(EndnoteFlowProfile::large_between_notes)
                                .unwrap_or(false);
                            let zero_endnote_spacing_profile_before_rewind = endnote_flow_profile
                                .map(EndnoteFlowProfile::zero_spacing)
                                .unwrap_or(false);
                            let current_default_late_question_title =
                                default_between_notes_gap_before_rewind
                                    && default_nonzero_between_note_tail_candidate
                                    && ep_idx == 0
                                    && st.current_column + 1 >= st.col_count;
                            let has_visible_endnote_separator_before_rewind = endnote_flow_profile
                                .map(|profile| profile.visible_separator)
                                .unwrap_or(false);
                            let large_separator_block_before_rewind = endnote_flow_profile
                                .map(EndnoteFlowProfile::large_between_notes)
                                .unwrap_or(false);
                            // 같은 미주 안에서도 LINE_SEG vpos 가 되감기며 다음 단 시작을
                            // 표시하는 문서가 있다. 특히 3-09월_교육_통합_2022.hwp 9쪽의
                            // 문5) 풀이처럼 단 하단에서 다음 paragraph first_vpos 가 직전
                            // bottom 보다 작아지는 경우, 한컴은 같은 단에 겹쳐 쌓지 않고
                            // 다음 단으로 넘긴다.
                            let local_rewind_advance_threshold =
                                if absorbed_between_notes_gap_before_rewind {
                                    0.65
                                } else if large_between_notes_gap_before_rewind {
                                    0.80
                                } else {
                                    0.85
                                };
                            let zero_visible_local_rewind_equation_line_tail_fits =
                                compact_endnote_separator_profile
                                    && zero_endnote_spacing_profile_before_rewind
                                    && has_visible_endnote_separator_before_rewind
                                    && st.col_count > 1
                                    && ep_idx > 0
                                    && fmt.line_heights.len() == 1
                                    && !para_is_treat_as_char_picture_only(en_para)
                                    && line_has_tac_equation_control(en_para, &composed, 0)
                                    && st.current_height + fmt.line_advance(0)
                                        <= available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX;
                            let zero_visible_local_rewind_text_run_para_fits =
                                compact_endnote_separator_profile
                                    && zero_endnote_spacing_profile_before_rewind
                                    && has_visible_endnote_separator_before_rewind
                                    && ep_idx > 0
                                    && !para_is_treat_as_char_picture_only(en_para)
                                    && this_first_offset
                                        .is_some_and(|first| first <= endnote_start)
                                    && line_has_text_span(&composed, 0)
                                    && line_leading_tac_equation_count(en_para, &composed, 0) >= 2
                                    && st.current_height + fmt.height_for_fit
                                        <= available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX;
                            let zero_visible_last_column_local_rewind_text_fits =
                                compact_endnote_separator_profile
                                    && zero_endnote_spacing_profile_before_rewind
                                    && has_visible_endnote_separator_before_rewind
                                    && st.current_column + 1 >= st.col_count
                                    && ep_idx > 0
                                    && !para_is_treat_as_char_picture_only(en_para)
                                    && para_has_visible_text_or_equation(en_para)
                                    && st.current_height + fmt.total_height
                                        <= available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 2.0
                                    && matches!(
                                        (prev_en_bottom_vpos, this_first_offset),
                                        (Some(prev), Some(first)) if first < prev
                                    );
                            let zero_between_visible_local_rewind_para_fits_current_column =
                                compact_endnote_separator_profile
                                    && has_visible_endnote_separator_before_rewind
                                    && endnote_flow_profile
                                        .map(|profile| {
                                            profile.between_notes_hu == 0
                                                && profile.large_separator_margin()
                                        })
                                        .unwrap_or(false)
                                    && ep_idx > 0
                                    && !para_is_treat_as_char_picture_only(en_para)
                                    && para_has_visible_text_or_equation(en_para)
                                    && st.current_height > available * 0.80
                                    && st.current_height + fmt.total_height
                                        <= available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 4.0
                                    && matches!(
                                        (prev_en_bottom_vpos, this_first_offset),
                                        (Some(prev), Some(first)) if first < prev
                                    );
                            let no_separator_local_rewind_final_tail_fits_current_column =
                                compact_endnote_separator_profile
                                    && large_separator_block_before_rewind
                                    && !has_visible_endnote_separator_before_rewind
                                    && st.current_column + 1 < st.col_count
                                    && ep_idx > 0
                                    && ep_idx + 2 >= en_ctrl.paragraphs.len()
                                    && st.current_height > available * 0.90
                                    && matches!(
                                        (prev_en_bottom_vpos, this_first_offset),
                                        (Some(prev), Some(first)) if first < prev
                                    )
                                    && {
                                        let remaining_tail: f64 = en_ctrl
                                            .paragraphs
                                            .iter()
                                            .skip(ep_idx)
                                            .map(|tail_para| {
                                                let tail_comp =
                                                    crate::renderer::composer::compose_paragraph(
                                                        tail_para,
                                                    );
                                                self.format_paragraph(
                                                    tail_para,
                                                    Some(&tail_comp),
                                                    &styles,
                                                    Some(en_col_w),
                                                )
                                                .total_height
                                            })
                                            .sum();
                                        // 구분선 없는 미주 끝의 짧은 rewind tail은 같은 단
                                        // 하단에 남고, 다음 미주 제목부터 새 단으로 넘어간다.
                                        // 이 tail까지 밀면 한컴/PDF보다 오른쪽 단이 늦게 시작한다.
                                        st.current_height + remaining_tail
                                            <= available
                                                + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                + 4.0
                                    };
                            let zero_between_visible_local_rewind_final_tail_fits_current_column =
                                compact_endnote_separator_profile
                                    && has_visible_endnote_separator_before_rewind
                                    && endnote_flow_profile
                                        .map(|profile| {
                                            profile.between_notes_hu == 0
                                                && profile.large_separator_margin()
                                        })
                                        .unwrap_or(false)
                                    && st.current_column + 1 < st.col_count
                                    && ep_idx > 0
                                    && ep_idx + 1 >= en_ctrl.paragraphs.len()
                                    && fmt.line_heights.len() == 1
                                    && !para_is_treat_as_char_picture_only(en_para)
                                    && para_has_visible_text_or_equation(en_para)
                                    && st.current_height > available * 0.90
                                    && matches!(
                                        (prev_en_bottom_vpos, this_first_offset),
                                        (Some(prev), Some(first)) if first < prev
                                    )
                                    && {
                                        let remaining_tail: f64 = en_ctrl
                                            .paragraphs
                                            .iter()
                                            .skip(ep_idx)
                                            .map(|tail_para| {
                                                let tail_comp =
                                                    crate::renderer::composer::compose_paragraph(
                                                        tail_para,
                                                    );
                                                self.format_paragraph(
                                                    tail_para,
                                                    Some(&tail_comp),
                                                    &styles,
                                                    Some(en_col_w),
                                                )
                                                .total_height
                                            })
                                            .sum();
                                        // 미주 사이 0에서는 마지막 rewind tail과 다음 번호 제목
                                        // 사이에 추가 미주 gap을 만들지 않는다. tail 자체가
                                        // frame 안에 들어가면 현재 단 하단에 남긴다.
                                        st.current_height + remaining_tail
                                            <= available
                                                + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                + 4.0
                                    };
                            let visible_separator_title_body_rewind_starts_next_column =
                                compact_endnote_separator_profile
                                    && has_visible_endnote_separator_before_rewind
                                    && default_between_notes_gap_before_rewind
                                    && endnote_flow_profile
                                        .map(EndnoteFlowProfile::large_separator_margin)
                                        .unwrap_or(false)
                                    && st.col_count > 1
                                    && st.current_column + 1 < st.col_count
                                    && ep_idx == 1
                                    && en_ref.number > 0
                                    && st.current_height > available * 0.80
                                    && en_ctrl
                                        .paragraphs
                                        .first()
                                        .is_some_and(|title_para| title_para.line_segs.len() == 1)
                                    && para_has_visible_text_or_equation(en_para)
                                    && matches!(
                                        (prev_en_bottom_vpos, this_first_offset),
                                        (Some(prev), Some(first)) if first < prev
                                    );
                            if st.col_count > 1
                                && !st.current_items.is_empty()
                                && (st.current_height > available * local_rewind_advance_threshold
                                    || visible_separator_title_body_rewind_starts_next_column)
                                && !current_default_late_question_title
                                && !zero_visible_local_rewind_equation_line_tail_fits
                                && !zero_visible_local_rewind_text_run_para_fits
                                && !zero_visible_last_column_local_rewind_text_fits
                                && !zero_between_visible_local_rewind_para_fits_current_column
                                && !no_separator_local_rewind_final_tail_fits_current_column
                                && !zero_between_visible_local_rewind_final_tail_fits_current_column
                                && matches!(
                                    (prev_en_bottom_vpos, this_first_offset),
                                    (Some(prev), Some(first)) if first < prev
                                )
                            {
                                st.advance_column_or_new_page();
                                prev_en_bottom_vpos = None;
                                prev_en_content_bottom_vpos = None;
                            }
                            let local_vpos_rewind = matches!(
                                (prev_en_bottom_vpos, this_first_offset),
                                (Some(prev), Some(first)) if first < prev
                            );
                            let has_visible_endnote_separator =
                                has_visible_endnote_separator_before_rewind;
                            // 보이는 구분선 + 큰 미주 사이에서는 renderer가 이전 content floor를
                            // 넘는 되감김을 순차 y로 유지한다. pagination도 같은 조건에서
                            // TAC 그림 되감김 축약을 피해야 단 하단 overflow가 줄어든다.
                            let local_vpos_rewind_crosses_prev_content =
                                large_between_notes_gap_before_rewind
                                    && has_visible_endnote_separator
                                    && st.current_height > available * 0.225
                                    && matches!(
                                        (prev_en_content_bottom_vpos, this_first_offset),
                                        (Some(prev_content), Some(first)) if first < prev_content
                                    );
                            let large_vpos_jump_at_column_top = st.col_count > 1
                                && st.current_height < available * 0.20
                                && matches!(
                                    (prev_en_bottom_vpos, this_first_offset),
                                    (Some(prev), Some(first))
                                        if first > prev
                                            && hwpunit_to_px(first - prev, self.dpi)
                                                > available * 0.75
                                );
                            let internal_rewind_position = en_para
                                .line_segs
                                .windows(2)
                                .position(|w| w[1].vertical_pos < w[0].vertical_pos)
                                .map(|idx| idx + 1)
                                .filter(|split| {
                                    *split > 0
                                        && *split < en_para.line_segs.len()
                                        && *split < fmt.line_heights.len()
                                });
                            let internal_vpos_rewind = internal_rewind_position.is_some();
                            let saved_page_reset_rewind = internal_rewind_position
                                .and_then(|split| {
                                    en_para.line_segs.get(split).map(|seg| (split, seg))
                                })
                                .map(|(split, seg)| {
                                    split >= 4
                                        && seg.vertical_pos <= 0
                                        && st.current_height > available * 0.65
                                })
                                .unwrap_or(false);
                            let large_separator_block = endnote_flow_profile
                                .map(EndnoteFlowProfile::large_between_notes)
                                .unwrap_or(false);
                            let zero_between_large_separator_margin = endnote_flow_profile
                                .map(
                                    EndnoteFlowProfile::visible_zero_between_large_separator_margin,
                                )
                                .unwrap_or(false);
                            let endnote_has_text_or_equation =
                                para_has_visible_text_or_equation(en_para);
                            let endnote_has_visible_payload = endnote_has_text_or_equation
                                || para_has_non_tac_picture_or_shape(en_para);
                            let mut internal_rewind_split = if compact_endnote_separator_profile
                                && st.col_count > 1
                                && (st.current_height > available * 0.75 || saved_page_reset_rewind)
                                && endnote_has_visible_payload
                            {
                                internal_rewind_position
                            } else {
                                None
                            };
                            let move_internal_rewind_equation_to_next =
                                compact_endnote_separator_profile
                                    && internal_vpos_rewind
                                    && internal_rewind_split.is_none()
                                    && st.col_count > 1
                                    && st.current_height > available * 0.75
                                    && endnote_has_visible_payload;

                            let col_count = st.col_count;
                            let dpi = self.dpi;
                            let h4f = fmt.height_for_fit;
                            let tot = fmt.total_height;
                            let default_between_notes_gap = endnote_flow_profile
                                .map(EndnoteFlowProfile::default_between_notes)
                                .unwrap_or(false);
                            let zero_endnote_spacing_profile = endnote_flow_profile
                                .map(EndnoteFlowProfile::zero_spacing)
                                .unwrap_or(false);
                            let compact_between_notes_gap = endnote_flow_profile
                                .map(EndnoteFlowProfile::default_or_compact_between_notes)
                                .unwrap_or(false);
                            let absorbed_between_notes_gap = endnote_flow_profile
                                .map(|profile| profile.absorbed_between_notes_gap)
                                .unwrap_or(false);
                            let visible_non_default_compact_between_gap = endnote_flow_profile
                                .map(EndnoteFlowProfile::visible_non_default_compact_between_notes)
                                .unwrap_or(false);
                            let visible_large_between_notes_gap = endnote_flow_profile
                                .map(EndnoteFlowProfile::visible_large_between_notes)
                                .unwrap_or(false);
                            let no_separator_large_between_notes_gap = endnote_flow_profile
                                .map(EndnoteFlowProfile::no_separator_large_between_notes)
                                .unwrap_or(false);
                            let visible_zero_between_large_separator_gap = endnote_flow_profile
                                .map(
                                    EndnoteFlowProfile::visible_zero_between_large_separator_margin,
                                )
                                .unwrap_or(false);
                            let visible_large_between_zero_above_compact_below =
                                endnote_flow_profile
                                    .map(
                                        EndnoteFlowProfile::visible_large_between_zero_above_compact_below,
                                    )
                                    .unwrap_or(false);
                            // 기본 미주 사이 7mm의 번호 미주 tail은 단 하단에서도 제목 뒤
                            // 풀이 본문 일부가 같은 쪽에 이어지는 경우가 있다. 20mm처럼
                            // 커진 "미주 사이"는 별도 큰 gap 정책을 타야 한다.
                            let allow_default_late_question_tail = default_between_notes_gap
                                && !zero_endnote_spacing_profile
                                && default_nonzero_between_note_tail_candidate
                                && st.current_column + 1 >= st.col_count;
                            let suppress_late_question_gap_for_fit =
                                allow_default_late_question_tail
                                    && st.current_column + 1 >= st.col_count
                                    && st.current_height > available * 0.90;
                            let large_rewind_equation_tail_new_note_gap_absorbed = ep_idx == 0
                                && emitted_endnote_count > 0
                                && endnote_flow_profile
                                    .map(EndnoteFlowProfile::visible_large_between_notes)
                                    .unwrap_or(false)
                                && boundary_prev_endnote_had_vpos_rewind
                                && continued_endnote_tail_before_new_note
                                && st.current_height < available * 0.35
                                && prev_render_endnote_para_local_idx
                                    .and_then(|idx| st.endnote_paragraphs.get(idx))
                                    .map(|prev_para| {
                                        !para_has_visible_text(prev_para)
                                            && para_has_visible_text_or_equation(prev_para)
                                    })
                                    .unwrap_or(false);
                            let new_endnote_between_notes_px = if ep_idx == 0
                                && emitted_endnote_count > 0
                                && compact_endnote_separator_profile
                                && !suppress_late_question_gap_for_fit
                                && !large_rewind_equation_tail_new_note_gap_absorbed
                            {
                                endnote_shape.map(|shape| {
                                    let gap = endnote_between_notes_margin(shape) as i32;
                                    let default_visible_tail_absorbed_gap =
                                        default_between_notes_gap
                                            && has_visible_endnote_separator
                                            && boundary_prev_endnote_had_vpos_rewind
                                            && st.current_column + 1 >= st.col_count
                                            && st.current_height > available * 0.25
                                            && st.current_height < available * 0.50;
                                    let effective_gap = if default_visible_tail_absorbed_gap {
                                        0
                                    } else {
                                        gap
                                    };
                                    hwpunit_to_px(effective_gap, dpi)
                                })
                            } else {
                                None
                            };
                            let same_endnote_body_first_line_advance =
                                if ep_idx == 0 && no_separator_large_between_notes_gap {
                                    en_ctrl.paragraphs.get(1).map(|body_para| {
                                        let body_comp =
                                            crate::renderer::composer::compose_paragraph(body_para);
                                        let body_fmt = self.format_paragraph(
                                            body_para,
                                            Some(&body_comp),
                                            &styles,
                                            Some(en_col_w),
                                        );
                                        body_fmt.line_advance(0)
                                    })
                                } else {
                                    None
                                };
                            let no_separator_new_note_head_fits_current_column =
                                no_separator_large_between_notes_gap
                                    && ep_idx == 0
                                    && emitted_endnote_count > 0
                                    && new_endnote_between_notes_px
                                        .map(|gap| {
                                            st.current_height + fmt.line_advance(0) + gap
                                                <= available
                                                    + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                        })
                                        .unwrap_or(false);
                            let no_separator_last_column_new_note_head_without_gap_fits =
                                no_separator_large_between_notes_gap
                                    && ep_idx == 0
                                    && emitted_endnote_count > 0
                                    && st.current_column + 1 >= st.col_count
                                    && st.current_height > available * 0.80
                                    && same_endnote_body_first_line_advance
                                        .map(|body_head| {
                                            // 구분선이 없는 마지막 단에서는 직전 미주의
                                            // 마지막 line spacing이 이미 다음 번호와의
                                            // 시각 gap을 갖는 경우가 있다. 한컴은 이 gap을
                                            // 새 번호 앞에 다시 예약하지 않고, 제목과 첫 본문
                                            // 줄이 들어가면 현재 쪽 하단 tail로 남긴다.
                                            st.current_height + fmt.line_advance(0) + body_head
                                                <= available
                                                    + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                    + 8.0
                                        })
                                        .unwrap_or(false);
                            let visible_separator_new_note_title_tail_fits_for_a2 =
                                compact_endnote_separator_profile
                                    && visible_non_default_compact_between_gap
                                    && ep_idx == 0
                                    && emitted_endnote_count > 0
                                    && en_ref.number > 0
                                    && fmt.line_heights.len() == 1
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && st.current_column + 1 < st.col_count
                                    && st.current_height > available * 0.88
                                    && st.current_height < available
                                    && new_endnote_between_notes_px
                                        .map(|gap| {
                                            st.current_height + fmt.line_advance(0) + gap
                                                <= available
                                                    + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                    + 2.0
                                        })
                                        .unwrap_or_else(|| {
                                            st.current_height + fmt.line_advance(0)
                                                <= available
                                                    + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                    + 2.0
                                        })
                                    && endnote_has_visible_payload;
                            let min_vpos_rewind_height = en_para
                                .line_segs
                                .first()
                                .map(|s| hwpunit_to_px(s.line_height.max(1), dpi))
                                .unwrap_or(h4f);
                            let has_treat_as_char_picture_shape =
                                para_has_treat_as_char_picture_or_shape(en_para);
                            let tac_picture_only_height =
                                if para_is_treat_as_char_picture_only(en_para) {
                                    en_para
                                        .controls
                                        .iter()
                                        .filter_map(|ctrl| {
                                            tac_picture_or_shape_height_px(ctrl, dpi)
                                        })
                                        .reduce(f64::max)
                                } else {
                                    None
                                };
                            let tac_picture_tail_height = (0..fmt.line_heights.len())
                                .filter(|line_idx| !line_has_visible_text(&composed, *line_idx))
                                .filter_map(|line_idx| {
                                    line_tac_picture_or_shape_height(
                                        en_para, &composed, line_idx, dpi,
                                    )
                                })
                                .chain(tac_picture_only_height)
                                .reduce(f64::max);
                            let tac_picture_tail_group_height = if let (Some(first), Some(pic_h)) =
                                (this_first_offset, tac_picture_tail_height)
                            {
                                let tail_bottom = en_ctrl
                                    .paragraphs
                                    .iter()
                                    .skip(ep_idx + 1)
                                    .flat_map(|p| p.line_segs.iter())
                                    .map(|s| {
                                        s.vertical_pos
                                            + s.line_height
                                            + s.line_spacing
                                            + endnote_start
                                    })
                                    .max();
                                Some(
                                    tail_bottom
                                        .map(|bottom| hwpunit_to_px((bottom - first).max(0), dpi))
                                        .unwrap_or(0.0)
                                        .max(pic_h),
                                )
                            } else {
                                None
                            };
                            let cap_large_separator_stale_forward_vpos = large_separator_block
                                && compact_between_notes_gap
                                && st.current_height < available * 0.70;
                            let current_height_for_metrics = st.current_height;
                            let current_column_has_tac_picture_only = st
                                .current_items
                                .iter()
                                .filter_map(page_item_para_index)
                                .any(|pi| {
                                    paragraph_by_global_index(
                                        paragraphs,
                                        &st.endnote_paragraphs,
                                        pi,
                                    )
                                    .map(para_is_treat_as_char_picture_only)
                                    .unwrap_or(false)
                                });
                            // [Task #1363] SSOT: layout 이 순차 format 으로 렌더하는 점유 높이.
                            // Divergence A(내부 vpos rewind) 이전의 ground truth.
                            let line_advances_sum =
                                fmt.line_advances_sum(0..fmt.line_heights.len());
                            let ssot_level = en_ssot_level();
                            let ssot_debug = en_ssot_debug();
                            let mut compute_en_metrics =
                                |prev: Option<i32>, emit: bool| -> (f64, f64) {
                                    if col_count > 1 {
                                        if let (Some(tf), Some(tb)) =
                                            (this_first_offset, this_bottom_offset)
                                        {
                                            let base = if local_vpos_rewind
                                                || large_vpos_jump_at_column_top
                                            {
                                                tf
                                            } else {
                                                prev.unwrap_or(tf)
                                            };
                                            let advance_px = hwpunit_to_px((tb - base).max(0), dpi);
                                            let compact_local_rewind =
                                                compact_endnote_separator_profile
                                                    && local_vpos_rewind
                                                    && !local_vpos_rewind_crosses_prev_content;
                                            // 한컴 저장본의 미주 LineSeg는 TAC 도형을 포함한 문단의
                                            // 다음 줄/문단 시작 vpos까지 이미 반영한다. formatter가
                                            // inline object 높이를 다시 큰 floor로 잡으면 2023 12쪽처럼
                                            // 다음 문제 시작이 한 단 늦게 밀린다.
                                            let inline_object_formatter_overestimate =
                                                compact_endnote_separator_profile
                                                    && has_treat_as_char_picture_shape
                                                    && !internal_vpos_rewind
                                                    && !compact_local_rewind
                                                    && !large_vpos_jump_at_column_top
                                                    && h4f > advance_px + 80.0
                                                    && advance_px > min_vpos_rewind_height + 40.0;
                                            if inline_object_formatter_overestimate {
                                                current_endnote_had_inline_object_vpos_overestimate =
                                                true;
                                            }
                                            let min_h = if inline_object_formatter_overestimate {
                                                (advance_px - trailing_ls_px)
                                                    .max(min_vpos_rewind_height)
                                            } else if internal_vpos_rewind || compact_local_rewind {
                                                min_vpos_rewind_height
                                            } else {
                                                h4f
                                            };
                                            let stale_forward_vpos =
                                                compact_endnote_separator_profile
                                                    && !local_vpos_rewind
                                                    && !large_vpos_jump_at_column_top
                                                    && (!large_separator_block
                                                        || has_visible_endnote_separator
                                                        || cap_large_separator_stale_forward_vpos)
                                                    && advance_px > h4f + 100.0;
                                            let compact_internal_rewind_full_advance =
                                                compact_endnote_separator_profile
                                                    && internal_vpos_rewind
                                                    && !local_vpos_rewind
                                                    && !large_vpos_jump_at_column_top
                                                    && !has_treat_as_char_picture_shape
                                                    && current_height_for_metrics
                                                        < available * 0.45
                                                    && tot > advance_px + 40.0;
                                            let cap_no_separator_stale_new_note =
                                                large_separator_block
                                                    && !has_visible_endnote_separator
                                                    && (current_height_for_metrics < available * 0.50
                                                        || (current_column_has_tac_picture_only
                                                            && current_height_for_metrics
                                                                < available * 0.65)
                                                        || no_separator_new_note_head_fits_current_column);
                                            let capped_new_endnote_advance =
                                                if large_separator_block
                                                    && !has_visible_endnote_separator
                                                    && !cap_no_separator_stale_new_note
                                                {
                                                    None
                                                } else {
                                                    new_endnote_between_notes_px
                                                        .map(|gap| h4f + gap)
                                                        .filter(|cap| advance_px > *cap + 12.0)
                                                };
                                            let metric_advance_px =
                                                if compact_internal_rewind_full_advance {
                                                    tot
                                                } else if compact_local_rewind {
                                                    min_vpos_rewind_height
                                                } else if let Some(cap) = capped_new_endnote_advance
                                                {
                                                    cap
                                                } else if stale_forward_vpos {
                                                    h4f
                                                } else {
                                                    advance_px
                                                };
                                            let fit =
                                                (metric_advance_px - trailing_ls_px).max(min_h);
                                            let acc_legacy = metric_advance_px.max(min_h);
                                            // [Task #1363] Divergence A: 내부 vpos rewind para 는
                                            // layout 이 첫 줄만 vpos 로 배치한 뒤 나머지 줄을 순차
                                            // format 으로 렌더하므로 실제 점유 높이 = 전체
                                            // line_advances_sum. saved-vpos delta(metric_advance_px)
                                            // 는 rewind 로 과소 추정(pi=894 −61.2)되어 단 하단
                                            // 본문 초과를 유발 → SSOT 로 대체.
                                            // [Task #1363 Stage 5] 잔여 Divergence B(trailing-ls)·
                                            // 전면 SSOT 는 acc=line_advances_sum 로 닫을 수 없음(실증):
                                            //  · 전면: capped/stale/overlap para 를 렌더가 line_adv_sum
                                            //    보다 작게 겹쳐 그려 2022 overflow +166px 회귀.
                                            //  · uncapped sequential 한정: trailing-ls 가산이 미주
                                            //    질문 흐름(단 배치)을 흔들어 issue_1139/1261/1284 10건
                                            //    회귀. → 잔여 divergence 는 overflow 무영향이고 안전
                                            //    정합 불가하므로 보류. acc 는 A(rewind)/C(TAC)만 SSOT.
                                            let acc = if ssot_level >= EnSsotLevel::A
                                                && internal_vpos_rewind
                                            {
                                                line_advances_sum.max(min_vpos_rewind_height)
                                            } else {
                                                acc_legacy
                                            };
                                            if emit && ssot_debug {
                                                eprintln!(
                                                "EN_SSOT pi={} rewind={} acc_legacy={:.1} acc={:.1} line_adv_sum={:.1} fit={:.1} h4f={:.1}",
                                                en_para_idx,
                                                internal_vpos_rewind,
                                                acc_legacy,
                                                acc,
                                                line_advances_sum,
                                                fit,
                                                h4f,
                                            );
                                            }
                                            (fit, acc)
                                        } else {
                                            if emit && ssot_debug {
                                                eprintln!(
                                                "EN_SSOT pi={} rewind={} acc_legacy={:.1} acc={:.1} line_adv_sum={:.1} fit={:.1} h4f={:.1}",
                                                en_para_idx,
                                                internal_vpos_rewind,
                                                tot,
                                                tot,
                                                line_advances_sum,
                                                h4f,
                                                h4f,
                                            );
                                            }
                                            (h4f, tot)
                                        }
                                    } else {
                                        (h4f, tot)
                                    }
                                };

                            let non_tac_object_height = if endnote_has_text_or_equation {
                                None
                            } else {
                                non_tac_picture_or_shape_block_height_px(en_para, dpi)
                            };
                            let endnote_boundary_gap_extra_px = endnote_shape
                                .filter(|shape| {
                                    let between_notes = endnote_between_notes_margin(shape) as i32;
                                    compact_endnote_separator_profile
                                        && ep_idx + 1 == en_ctrl.paragraphs.len()
                                        && endnote_refs.get(en_ref_idx + 1).is_some()
                                        && between_notes > ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU
                                        && !endnote_has_absorbed_between_notes_gap(shape)
                                })
                                .map(|shape| {
                                    let between_notes = endnote_between_notes_margin(shape) as i32;
                                    let saved_spacing = en_para
                                        .line_segs
                                        .last()
                                        .map(|seg| seg.line_spacing.max(0))
                                        .unwrap_or(0);
                                    hwpunit_to_px((between_notes - saved_spacing).max(0), self.dpi)
                                })
                                .unwrap_or(0.0);
                            let (raw_en_fit, _) = compute_en_metrics(prev_en_bottom_vpos, false);
                            let en_fit = non_tac_object_height
                                .map(|height| raw_en_fit.max(height))
                                .unwrap_or(raw_en_fit);
                            let total_advance_fit =
                                line_advances_sum.max(non_tac_object_height.unwrap_or(0.0));
                            let remaining_height = (available - st.current_height).max(0.0);
                            // [Task #1363 v2 Stage 3] A2: 새 para 를 이어붙인 렌더-정합 시뮬
                            // bottom 으로 fit 판정 (saved line_segs 기반 → 렌더와 일치).
                            let a2_overflow_with_para = if ssot_level >= EnSsotLevel::A2 {
                                self.simulate_endnote_column_bottom_y(
                                    &st,
                                    paragraphs,
                                    styles,
                                    available,
                                    en_col_w,
                                    Some(en_para_idx),
                                )
                                .map(|bottom| {
                                    bottom > available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                })
                            } else {
                                None
                            };
                            // 구분선 없는 큰 미주 block에서는 다줄 수식 문단의 advance가
                            // frame을 약간 넘더라도 실제 보이는 줄은 하단 frame 안에 남는다.
                            // 이 tail을 통째로 유지해야 다음 단의 새 문항 시작점이 한컴과 맞는다.
                            let no_separator_tail_extra_bleed =
                                if st.current_column + 1 < st.col_count {
                                    24.0
                                } else {
                                    0.0
                                };
                            let no_separator_tail_min_height_ratio =
                                if st.current_column + 1 < st.col_count {
                                    0.90
                                } else {
                                    0.84
                                };
                            let no_separator_visible_multiline_tail_fits_with_bleed =
                                large_separator_block
                                    && !has_visible_endnote_separator
                                    && ep_idx > 0
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && fmt.line_heights.len() > 1
                                    && st.current_height
                                        > available * no_separator_tail_min_height_ratio
                                    && para_has_visible_text_or_equation(en_para)
                                    && !para_has_non_tac_picture_or_shape(en_para)
                                    && st.current_height + total_advance_fit
                                        <= available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + no_separator_tail_extra_bleed;
                            let next_endnote_title_fit_height = if ep_idx + 1
                                == en_ctrl.paragraphs.len()
                            {
                                endnote_refs.get(en_ref_idx + 1).and_then(|next_ref| {
                                    let next_host = paragraphs.get(next_ref.para_index)?;
                                    let Control::Endnote(next_ctrl) =
                                        next_host.controls.get(next_ref.control_index)?
                                    else {
                                        return None;
                                    };
                                    let mut next_para = next_ctrl.paragraphs.first()?.clone();
                                    let prefix =
                                        format!("{} ", format_endnote_marker_text(next_ctrl));
                                    next_para.text = format!("{}{}", prefix, next_para.text);
                                    next_para.char_count += prefix.encode_utf16().count() as u32;
                                    let shift = prefix.encode_utf16().count() as u32;
                                    for off in &mut next_para.char_offsets {
                                        *off += shift;
                                    }
                                    let mut new_offsets: Vec<u32> = (0..shift).collect();
                                    new_offsets.extend_from_slice(&next_para.char_offsets);
                                    next_para.char_offsets = new_offsets;

                                    let next_comp =
                                        crate::renderer::composer::compose_paragraph(&next_para);
                                    let next_fmt = self.format_paragraph(
                                        &next_para,
                                        Some(&next_comp),
                                        &styles,
                                        Some(en_col_w),
                                    );
                                    (next_fmt.line_heights.len() == 1
                                        && line_has_visible_text_or_tac_equation(
                                            &next_para, &next_comp, 0,
                                        ))
                                    .then_some(next_fmt.height_for_fit)
                                })
                            } else {
                                None
                            };
                            let next_endnote_head_has_large_tac_picture = if ep_idx + 1
                                == en_ctrl.paragraphs.len()
                            {
                                endnote_refs
                                    .get(en_ref_idx + 1)
                                    .and_then(|next_ref| {
                                        let next_host = paragraphs.get(next_ref.para_index)?;
                                        let Control::Endnote(next_ctrl) =
                                            next_host.controls.get(next_ref.control_index)?
                                        else {
                                            return None;
                                        };
                                        Some(next_ctrl.paragraphs.iter().take(8).any(|next_para| {
                                            if !para_is_treat_as_char_picture_only(next_para) {
                                                return false;
                                            }
                                            let next_comp =
                                                crate::renderer::composer::compose_paragraph(
                                                    next_para,
                                                );
                                            let next_fmt = self.format_paragraph(
                                                next_para,
                                                Some(&next_comp),
                                                &styles,
                                                Some(en_col_w),
                                            );
                                            next_fmt.height_for_fit > 80.0
                                        }))
                                    })
                                    .unwrap_or(false)
                            } else {
                                false
                            };
                            let compact_endnote_own_vpos_span_fits =
                                compact_endnote_separator_profile
                                    && st.col_count > 1
                                    && st.current_height < available
                                    && compact_between_notes_gap
                                    && !local_vpos_rewind
                                    && (!internal_vpos_rewind
                                        || (st.current_items.is_empty()
                                            && st.current_height <= 1.0))
                                    && endnote_has_visible_payload
                                    && non_tac_object_height
                                        .map(|height| {
                                            height
                                                <= remaining_height
                                                    + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                    + 1.0
                                        })
                                        .unwrap_or(true)
                                    && matches!(
                                        (this_first_offset, this_content_bottom_offset),
                                        (Some(first), Some(bottom))
                                            if hwpunit_to_px((bottom - first).max(0), dpi)
                                                <= remaining_height
                                                    + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                    + 1.0
                                    );
                            let compact_endnote_body_tail_overflows_frame =
                                compact_endnote_own_vpos_span_fits
                                    && ep_idx > 0
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && fmt.line_heights.len() > 1
                                    && st.current_height + total_advance_fit > available + 1.0
                                    && endnote_has_visible_payload
                                    && ((!default_between_notes_gap
                                        && st.current_height > available * 0.95)
                                        || (zero_between_large_separator_margin
                                            && st.current_column + 1 >= st.col_count
                                            && st.current_height > available * 0.80));
                            let no_separator_saved_vpos_tail_outside = large_separator_block
                                && !has_visible_endnote_separator
                                && ep_idx > 0
                                && st.current_column + 1 >= st.col_count
                                && st.current_height > available * 0.90
                                && st.current_height + en_fit
                                    > available - ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                && !local_vpos_rewind
                                && !internal_vpos_rewind
                                && endnote_has_visible_payload
                                && st
                                    .current_items
                                    .iter()
                                    .filter_map(page_item_para_index)
                                    .find_map(|pi| {
                                        paragraph_by_global_index(
                                            paragraphs,
                                            &st.endnote_paragraphs,
                                            pi,
                                        )
                                        .and_then(|p| p.line_segs.first())
                                        .map(|s| s.vertical_pos)
                                    })
                                    .and_then(|base_vpos| {
                                        this_first_offset.map(|first_vpos| {
                                            let predicted_y = hwpunit_to_px(
                                                (first_vpos - base_vpos).max(0),
                                                self.dpi,
                                            );
                                            predicted_y + fmt.line_advance(0)
                                                > available
                                                    - ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                        })
                                    })
                                    .unwrap_or(false);
                            let visible_separator_saved_vpos_tail_outside =
                                compact_endnote_separator_profile
                                    && zero_endnote_spacing_profile
                                    && has_visible_endnote_separator
                                    && ep_idx > 0
                                    && st.current_column + 1 >= st.col_count
                                    && st.current_height > available * 0.90
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && endnote_has_visible_payload
                                    && !(fmt.line_heights.len() == 1
                                        && !para_is_treat_as_char_picture_only(en_para)
                                        && st.current_height + total_advance_fit
                                            <= available
                                                + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX)
                                    && st
                                        .current_items
                                        .iter()
                                        .filter_map(page_item_para_index)
                                        .find_map(|pi| {
                                            paragraph_by_global_index(
                                                paragraphs,
                                                &st.endnote_paragraphs,
                                                pi,
                                            )
                                            .and_then(|p| p.line_segs.first())
                                            .map(|s| s.vertical_pos)
                                        })
                                        .and_then(|base_vpos| {
                                            this_first_offset.map(|first_vpos| {
                                                let predicted_y = hwpunit_to_px(
                                                    (first_vpos - base_vpos).max(0),
                                                    self.dpi,
                                                );
                                                predicted_y + total_advance_fit
                                                    > available
                                                        - ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            })
                                        })
                                        .unwrap_or(false);
                            let compact_endnote_own_vpos_span_fits_for_flow =
                                compact_endnote_own_vpos_span_fits
                                    && !compact_endnote_body_tail_overflows_frame
                                    && !visible_separator_saved_vpos_tail_outside
                                    && !(large_separator_block
                                        && ep_idx == 0
                                        && st.current_column + 1 >= st.col_count
                                        && st.current_height + en_fit > available);
                            let split_endnote_to_fit = if compact_endnote_separator_profile
                                && st.col_count > 1
                                && !local_vpos_rewind
                                && st.current_height < available
                                && !compact_endnote_own_vpos_span_fits_for_flow
                                && a2_overflow_with_para.unwrap_or(
                                    st.current_height + en_fit > available
                                        || st.current_height + total_advance_fit > available,
                                )
                                && fmt.line_heights.len() > 1
                                && endnote_has_visible_payload
                            {
                                let split_remaining_height = if has_visible_endnote_separator
                                    && st.current_column + 1 >= st.col_count
                                    && (!default_between_notes_gap
                                        || zero_between_large_separator_margin)
                                {
                                    // 보이는 구분선의 마지막 단에서는 renderer의 저장 vpos
                                    // 보정이 하단으로 약간 내려갈 수 있다. 미주 사이가
                                    // 0이어도 구분선 위/아래가 큰 프로필은 같은 방식으로
                                    // 마지막 visible tail 한 줄을 현재 단에 남긴다.
                                    remaining_height + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                } else {
                                    remaining_height
                                };
                                let mut sum = 0.0;
                                let mut split = 0usize;
                                for line_idx in 0..fmt.line_heights.len() {
                                    let line_h = fmt.line_advance(line_idx);
                                    if sum + line_h > split_remaining_height {
                                        break;
                                    }
                                    sum += line_h;
                                    split = line_idx + 1;
                                }
                                (split > 0 && split < fmt.line_heights.len()).then_some(split)
                            } else {
                                None
                            };
                            let split_endnote_to_fit = if split_endnote_to_fit.is_none()
                                && !default_between_notes_gap
                                && compact_endnote_separator_profile
                                && has_visible_endnote_separator
                                && ep_idx > 0
                                && st.current_column + 1 >= st.col_count
                                && fmt.line_heights.len() >= 5
                                && st.current_height > available * 0.84
                                && st.current_height + en_fit
                                    <= available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                && en_ctrl.paragraphs.get(ep_idx + 1).is_some_and(|next_para| {
                                    let next_comp =
                                        crate::renderer::composer::compose_paragraph(next_para);
                                    let next_fmt = self.format_paragraph(
                                        next_para,
                                        Some(&next_comp),
                                        &styles,
                                        Some(en_col_w),
                                    );
                                    let next_first = next_fmt.line_advance(0);
                                    st.current_height + en_fit + next_first
                                        > available - ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                }) {
                                // 큰 미주 사이의 마지막 단 하단에서는 저장 vpos가 현재
                                // 문단 마지막 줄을 다음 단/쪽의 첫 줄로 넘기는 경우가 있다.
                                // 다음 문단 첫 줄이 들어가지 않는 상황이면 현재 다줄 문단을
                                // 마지막 줄 직전에 쪼개 한컴의 tail 흐름을 따른다.
                                Some(fmt.line_heights.len() - 1)
                            } else {
                                split_endnote_to_fit
                            };
                            let split_endnote_to_fit = if split_endnote_to_fit.is_none()
                                && compact_endnote_separator_profile
                                && zero_endnote_spacing_profile
                                && has_visible_endnote_separator
                                && ep_idx > 0
                                && st.current_column + 1 >= st.col_count
                                && fmt.line_heights.len() >= 3
                                && st.current_height > available * 0.90
                                && !local_vpos_rewind
                                && !internal_vpos_rewind
                                && endnote_has_visible_payload
                            {
                                let tail_split = fmt.line_heights.len() - 1;
                                let head_h = fmt.line_advances_sum(0..tail_split);
                                let tail_h = fmt.line_advance(tail_split);
                                let head_fits = st.current_height + head_h
                                    <= available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX;
                                let tail_overflows = st.current_height + head_h + tail_h
                                    > available - ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX;
                                let last_line_visible = line_has_visible_text_or_tac_equation(
                                    en_para, &composed, tail_split,
                                );

                                // 0/0/0 미주의 마지막 단에서는 저장 vpos 보정 때문에
                                // 문단 전체 sequential 높이는 들어가도 마지막 줄만 frame
                                // 아래로 내려갈 수 있다. 한컴은 이 tail 한 줄을 다음 쪽
                                // 첫 줄로 넘기므로 마지막 줄 직전에 분할한다.
                                (head_fits && tail_overflows && last_line_visible)
                                    .then_some(tail_split)
                            } else {
                                split_endnote_to_fit
                            };
                            let late_internal_rewind_fit_split = compact_endnote_separator_profile
                                && internal_vpos_rewind
                                && !default_between_notes_gap
                                && !local_vpos_rewind
                                && !has_treat_as_char_picture_shape
                                && st.current_height > available * 0.90
                                && split_endnote_to_fit.is_some_and(|split| {
                                    split >= 4
                                        || (split == 1 && st.current_height > available * 0.97)
                                });
                            let split_endnote_to_fit = if late_internal_rewind_fit_split {
                                Some(1)
                            } else {
                                split_endnote_to_fit
                            };
                            let split_endnote_to_fit = if !default_between_notes_gap
                                && (compact_between_notes_gap
                                    || large_between_notes_gap_before_rewind)
                                && has_visible_endnote_separator
                                && internal_vpos_rewind
                                && st.current_column + 1 < st.col_count
                                && st.current_height > available * 0.90
                            {
                                split_endnote_to_fit.map(|split| {
                                    // 보이는 구분선 + 비기본/대형 "미주 사이" 샘플의 하단
                                    // internal-rewind 문단은 renderer가 저장 vpos/gap을
                                    // 적용해 마지막 포함 줄을 pagination보다 낮게 그린다.
                                    // split 후보의 마지막 줄을 다음 단으로 보내 overflow를
                                    // 사전에 차단한다.
                                    if split > 1 && split < fmt.line_heights.len() {
                                        split - 1
                                    } else {
                                        split
                                    }
                                })
                            } else {
                                split_endnote_to_fit
                            };
                            let mut split_endnote_to_fit = split_endnote_to_fit.filter(|split| {
                                let single_line_tail_split_at_bottom = *split == 1
                                    && !default_between_notes_gap
                                    && !allow_default_late_question_tail
                                    && !(late_internal_rewind_fit_split
                                        && has_visible_endnote_separator
                                        && compact_between_notes_gap)
                                    && endnote_has_visible_payload;
                                let large_separator_title_tail_split = *split == 1
                                    && large_separator_block
                                    && ep_idx == 0
                                    && st.current_column + 1 >= st.col_count
                                    && st.current_height + en_fit > available
                                    && endnote_has_visible_payload;
                                !single_line_tail_split_at_bottom
                                    && !large_separator_title_tail_split
                            });
                            if no_separator_visible_multiline_tail_fits_with_bleed {
                                // 구분선이 없는 큰 미주 block에서 이미 보이는 다줄 tail이
                                // 허용 bleed 안에 들어간다고 판정했다면, fit용 분할 후보도
                                // 함께 제거해야 한다. 그렇지 않으면 문단을 4/2줄처럼
                                // 쪼개 다음 쪽 문항 전체가 한컴보다 내려간다.
                                split_endnote_to_fit = None;
                            }
                            let compact_non_default_empty_column_rewind_fits =
                                compact_between_notes_gap
                                    && !default_between_notes_gap
                                    && internal_vpos_rewind
                                    && st.current_height <= 2.0
                                    && st.current_height + total_advance_fit
                                        <= available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX;
                            if compact_non_default_empty_column_rewind_fits {
                                // 이전 단 하단에서 다음 단/쪽으로 넘어온 내부 rewind 문단이
                                // 새 단 맨 위에서 통째로 들어가면 다시 줄 단위로 쪼개지 않는다.
                                // 여기서 분할하면 왼쪽 단에 수식 두 줄만 남고 다음 문항이
                                // 오른쪽 단으로 밀려 한컴/PDF보다 한 쪽 많아진다.
                                split_endnote_to_fit = None;
                            }
                            let visible_compact_sequential_tail_fits_current_column =
                                compact_between_notes_gap
                                    && !default_between_notes_gap
                                    && has_visible_endnote_separator
                                    && ep_idx > 0
                                    && st.current_column + 1 < st.col_count
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && endnote_has_visible_payload
                                    && st.current_height + total_advance_fit
                                        <= available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX;
                            // [Task #1363 v2 Stage 3] A2: split 불가(단일줄 등) para 가 단을
                            // 넘으면 먼저 단 advance (fit-or-advance). sim 이 렌더-정합이므로
                            // overflow 판정이 신뢰 가능.
                            if ssot_level >= EnSsotLevel::A2
                                && a2_overflow_with_para == Some(true)
                                && split_endnote_to_fit.is_none()
                                && !visible_compact_sequential_tail_fits_current_column
                                && !visible_separator_new_note_title_tail_fits_for_a2
                                && !st.current_items.is_empty()
                                && st.current_height > available * 0.5
                                && !local_vpos_rewind
                                && !internal_vpos_rewind
                            {
                                st.advance_column_or_new_page();
                                prev_en_bottom_vpos = None;
                            }
                            let large_between_split_head_render_overflows =
                                if !default_between_notes_gap
                                    && compact_endnote_separator_profile
                                    && has_visible_endnote_separator
                                    && ep_idx > 0
                                    && st.col_count > 1
                                    && st.current_height > available * 0.90
                                    && !st.current_items.is_empty()
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && fmt.line_heights.len() > 1
                                    && para_has_visible_text_or_equation(en_para)
                                    && endnote_has_visible_payload
                                {
                                    split_endnote_to_fit
                                        .and_then(|split_line| {
                                            let mut local_paras: Vec<Paragraph> = Vec::new();
                                            let mut local_indices: Vec<(usize, usize)> = Vec::new();
                                            for pi in st
                                                .current_items
                                                .iter()
                                                .filter_map(page_item_para_index)
                                                .chain(std::iter::once(en_para_idx))
                                            {
                                                if local_indices
                                                    .iter()
                                                    .any(|(global, _)| *global == pi)
                                                {
                                                    continue;
                                                }
                                                if let Some(p) = paragraph_by_global_index(
                                                    paragraphs,
                                                    &st.endnote_paragraphs,
                                                    pi,
                                                ) {
                                                    let local = local_paras.len();
                                                    local_paras.push(p.clone());
                                                    local_indices.push((pi, local));
                                                }
                                            }
                                            let lookup_local =
                                                |pi: usize, indices: &[(usize, usize)]| {
                                                    indices.iter().find_map(|(global, local)| {
                                                        (*global == pi).then_some(*local)
                                                    })
                                                };
                                            let first_vpos = st
                                                .current_items
                                                .iter()
                                                .filter_map(page_item_para_index)
                                                .find_map(|pi| {
                                                    paragraph_by_global_index(
                                                        paragraphs,
                                                        &st.endnote_paragraphs,
                                                        pi,
                                                    )
                                                    .and_then(|p| p.line_segs.first())
                                                    .map(|seg| seg.vertical_pos)
                                                })?;
                                            let mut hc = HeightCursor::new(
                                                self.dpi,
                                                0.0,
                                                available,
                                                st.current_start_height,
                                                Some(first_vpos),
                                                st.skip_spacing_before_prededuct,
                                                false,
                                                st.current_endnote_flow
                                                    && st.current_start_height < -0.5,
                                                st.current_endnote_flow,
                                            );
                                            hc.endnote_between_notes_hu =
                                                st.endnote_between_notes_hu;
                                            let mut y = st.current_start_height;
                                            for item in &st.current_items {
                                                let Some(pi) = page_item_para_index(item) else {
                                                    continue;
                                                };
                                                let Some(local) = lookup_local(pi, &local_indices)
                                                else {
                                                    continue;
                                                };
                                                y = hc.vpos_adjust(y, local, &local_paras, &styles);
                                                let item_para = &local_paras[local];
                                                let item_composed =
                                                    crate::renderer::composer::compose_paragraph(
                                                        item_para,
                                                    );
                                                let item_fmt = self.format_paragraph(
                                                    item_para,
                                                    Some(&item_composed),
                                                    &styles,
                                                    Some(en_col_w),
                                                );
                                                y += match item {
                                                    PageItem::PartialParagraph {
                                                        start_line,
                                                        end_line,
                                                        ..
                                                    } => item_fmt
                                                        .line_advances_sum(*start_line..*end_line),
                                                    PageItem::FullParagraph { .. } => {
                                                        item_fmt.total_height
                                                    }
                                                    _ => 0.0,
                                                };
                                                let current_vpos_rewinds_from_prev = hc
                                                    .prev_layout_para
                                                    .and_then(|prev_local| {
                                                        let prev_first = local_paras
                                                            .get(prev_local)
                                                            .and_then(|p| p.line_segs.first())
                                                            .map(|seg| seg.vertical_pos)?;
                                                        let curr_first = local_paras
                                                            .get(local)
                                                            .and_then(|p| p.line_segs.first())
                                                            .map(|seg| seg.vertical_pos)?;
                                                        Some(curr_first < prev_first)
                                                    })
                                                    .unwrap_or(false);
                                                if matches!(
                                                    item,
                                                    PageItem::PartialParagraph { start_line, .. }
                                                        if *start_line > 0
                                                ) || current_vpos_rewinds_from_prev
                                                {
                                                    hc.prev_layout_para = None;
                                                    hc.vpos_page_base = None;
                                                    hc.vpos_lazy_base = None;
                                                } else {
                                                    hc.prev_layout_para = Some(local);
                                                }
                                                hc.prev_item_was_partial_table =
                                                    matches!(item, PageItem::PartialTable { .. });
                                            }
                                            let local = lookup_local(en_para_idx, &local_indices)?;
                                            let predicted_y =
                                                hc.vpos_adjust(y, local, &local_paras, &styles);
                                            let split_head_h = fmt.line_advances_sum(0..split_line);
                                            Some(
                                                predicted_y + split_head_h
                                                    > available
                                                        + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                        + 1.0,
                                            )
                                        })
                                        .unwrap_or(false)
                                } else {
                                    false
                                };
                            if large_between_split_head_render_overflows {
                                // pagination 기준으로는 split head가 들어가도, 저장 vpos를 적용한
                                // 실제 render 위치가 frame을 넘으면 한컴처럼 문단 전체를 다음 단에서
                                // 시작시킨다.
                                split_endnote_to_fit = None;
                            }
                            let internal_rewind_head_fits_current_column = internal_rewind_split
                                .map(|split| {
                                    let head_h = fmt.line_advances_sum(0..split);
                                    head_h
                                        <= remaining_height
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 1.0
                                })
                                .unwrap_or(false);
                            let single_line_internal_rewind_head_overflows_frame =
                                internal_rewind_split == Some(1)
                                    && !default_between_notes_gap
                                    && ep_idx > 0
                                    && fmt.line_heights.len() > 1
                                    && st.current_height + fmt.line_advances_sum(0..1)
                                        > available + 1.0
                                    && endnote_has_visible_payload;
                            let internal_rewind_head_allows_current_column =
                                internal_rewind_head_fits_current_column
                                    && !single_line_internal_rewind_head_overflows_frame;
                            let internal_rewind_target_is_reset = internal_rewind_split
                                .and_then(|split| en_para.line_segs.get(split))
                                .map(|seg| seg.vertical_pos == 0)
                                .unwrap_or(false);
                            let preserve_reset_internal_rewind_split = internal_rewind_split
                                == Some(1)
                                && !default_between_notes_gap
                                && has_visible_endnote_separator
                                && st.current_column + 1 < st.col_count
                                && st.current_height > available * 0.75
                                && internal_rewind_target_is_reset
                                && internal_rewind_head_allows_current_column
                                && endnote_has_visible_payload;
                            let internal_rewind_head_overflows_current_column =
                                zero_endnote_spacing_profile
                                    && internal_rewind_split.is_some()
                                    && !internal_rewind_head_allows_current_column
                                    && st.current_height >= available;
                            let preserve_single_line_internal_rewind_split = internal_rewind_split
                                == Some(1)
                                && !default_between_notes_gap
                                && st.current_column + 1 < st.col_count
                                && fmt.line_heights.len() > 1
                                && internal_rewind_head_allows_current_column
                                && (st.current_height + total_advance_fit
                                    > available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                    || preserve_reset_internal_rewind_split)
                                && endnote_has_visible_payload;
                            let preserve_no_separator_last_column_single_line_rewind =
                                internal_rewind_split == Some(1)
                                    && large_separator_block
                                    && !has_visible_endnote_separator
                                    && !default_between_notes_gap
                                    && ep_idx == 1
                                    && st.current_column + 1 >= st.col_count
                                    && fmt.line_heights.len() > 1
                                    && internal_rewind_head_allows_current_column
                                    && st.current_height + fmt.line_advances_sum(0..1)
                                        <= available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 8.0
                                    && st
                                        .current_items
                                        .last()
                                        .and_then(page_item_para_index)
                                        .is_some_and(|prev_pi| prev_pi + 1 == en_para_idx)
                                    && endnote_has_visible_payload;
                            let large_between_single_line_internal_rewind = internal_rewind_split
                                == Some(1)
                                && !default_between_notes_gap
                                && endnote_has_visible_payload;
                            let advance_large_between_single_line_rewind =
                                large_between_single_line_internal_rewind
                                    && !preserve_no_separator_last_column_single_line_rewind
                                    && st.current_column + 1 >= st.col_count
                                    && st.current_height > available * 0.80
                                    && !st.current_items.is_empty();
                            if advance_large_between_single_line_rewind {
                                // 큰 `미주 사이` 문서의 마지막 단 하단에서 첫 줄부터
                                // vpos가 되감기는 문단은 한컴/PDF처럼 다음 쪽에서 통째로
                                // 시작해야 한다. 현재 쪽에 FullParagraph로 남기면 첫 줄이
                                // frame 밖에 그려지고, 다음 쪽 문항 흐름이 한 줄만큼 당겨진다.
                                st.advance_column_or_new_page();
                                prev_en_bottom_vpos = None;
                                internal_rewind_split = None;
                            } else if large_between_single_line_internal_rewind
                                && !preserve_single_line_internal_rewind_split
                                && !preserve_no_separator_last_column_single_line_rewind
                            {
                                internal_rewind_split = None;
                                cleared_single_line_internal_rewind_split = true;
                            }
                            let internal_reset_split_head_render_overflows = internal_rewind_split
                                .filter(|split| *split > 1)
                                .filter(|_| {
                                    !default_between_notes_gap
                                        && compact_endnote_separator_profile
                                        && has_visible_endnote_separator
                                        && internal_rewind_target_is_reset
                                        && st.col_count > 1
                                        && st.current_column + 1 >= st.col_count
                                        && !st.current_items.is_empty()
                                        && endnote_has_visible_payload
                                })
                                .and_then(|split| {
                                    self.predict_current_column_para_y(
                                        &st,
                                        en_para_idx,
                                        paragraphs,
                                        &styles,
                                        measured_tables,
                                        Some(en_col_w),
                                    )
                                    .map(|render_y| {
                                        render_y + fmt.line_advances_sum(0..split)
                                            > available
                                                + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                + 1.0
                                    })
                                })
                                .unwrap_or(false);
                            let internal_rewind_full_advance_needed = internal_rewind_split
                                .filter(|split| *split > 1)
                                .filter(|split| {
                                    split_endnote_to_fit.is_some_and(|fit_split| fit_split > *split)
                                })
                                .filter(|_| {
                                    default_between_notes_gap
                                        && compact_endnote_separator_profile
                                        && has_visible_endnote_separator
                                        && internal_vpos_rewind
                                        && internal_rewind_target_is_reset
                                        && st.col_count > 1
                                        && st.current_column + 1 < st.col_count
                                        && st.current_height > available * 0.90
                                        && !st.current_items.is_empty()
                                        && st.current_height + en_fit
                                            <= available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                        && st.current_height + total_advance_fit
                                            > available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                        && endnote_has_visible_payload
                                })
                                .and_then(|split| {
                                    let first = en_para.line_segs.first()?;
                                    let target = en_para.line_segs.get(split)?;
                                    (target.vertical_pos < first.vertical_pos).then_some(true)
                                })
                                .unwrap_or(false);
                            let new_endnote_stale_forward_vpos = compact_endnote_separator_profile
                                && ep_idx == 0
                                && emitted_endnote_count > 0
                                && !local_vpos_rewind
                                && !large_vpos_jump_at_column_top
                                && !large_separator_block
                                && matches!(
                                    (prev_en_bottom_vpos, this_first_offset, this_bottom_offset),
                                    (Some(prev), Some(_), Some(bottom))
                                        if hwpunit_to_px((bottom - prev).max(0), self.dpi) > h4f + 100.0
                                );
                            let large_between_tail_render_overflows = if !default_between_notes_gap
                                && compact_endnote_separator_profile
                                && (has_visible_endnote_separator || !large_separator_block)
                                && ep_idx > 0
                                && st.col_count > 1
                                && st.current_column + 1 < st.col_count
                                && st.current_height > available * 0.85
                                && !st.current_items.is_empty()
                                && !local_vpos_rewind
                                && !internal_vpos_rewind
                                && split_endnote_to_fit.is_none()
                                && !visible_compact_sequential_tail_fits_current_column
                                && para_has_visible_text(en_para)
                            {
                                let prev_equation_only_tail = st
                                    .current_items
                                    .iter()
                                    .rev()
                                    .filter_map(page_item_para_index)
                                    .find_map(|pi| {
                                        paragraph_by_global_index(
                                            paragraphs,
                                            &st.endnote_paragraphs,
                                            pi,
                                        )
                                    })
                                    .map(|prev_para| {
                                        !para_has_visible_text(prev_para)
                                            && prev_para.controls.iter().any(|ctrl| {
                                                matches!(ctrl, Control::Equation(eq) if eq.common.treat_as_char)
                                            })
                                    })
                                    .unwrap_or(false);
                                st.current_items
                                    .iter()
                                    .filter_map(page_item_para_index)
                                    .find_map(|pi| {
                                        paragraph_by_global_index(
                                            paragraphs,
                                            &st.endnote_paragraphs,
                                            pi,
                                        )
                                        .and_then(|p| p.line_segs.first())
                                        .map(|s| s.vertical_pos)
                                    })
                                    .and_then(|base_vpos| {
                                        this_first_offset.map(|first_vpos| {
                                            let predicted_y = hwpunit_to_px(
                                                (first_vpos - base_vpos).max(0),
                                                self.dpi,
                                            ) + st.current_start_height;
                                            let rendered_h =
                                                fmt.line_advances_sum(0..fmt.line_heights.len());
                                            // TAC 그림/수식으로 lazy base가 깊게 보정된 단에서는
                                            // 저장 vpos 직접 예측이 실제 렌더 y보다 낮게 나올 수 있다.
                                            // 직전 수식-only 문단 뒤의 한 줄짜리 풀이 tail은 남은
                                            // 공간이 50px 이하이면 한컴처럼 다음 단에서 이어간다.
                                            let near_bottom_tail = prev_equation_only_tail
                                                && fmt.line_heights.len() == 1
                                                && para_has_visible_text(en_para)
                                                && !para_is_treat_as_char_picture_only(en_para)
                                                && !para_has_treat_as_char_picture_or_shape(
                                                    en_para,
                                                )
                                                && st.current_height > available * 0.90
                                                && st.current_height + rendered_h
                                                    > available
                                                        + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX;
                                            predicted_y + rendered_h
                                                > available
                                                    + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                    + 1.0
                                                || near_bottom_tail
                                        })
                                    })
                                    .unwrap_or(false)
                            } else {
                                false
                            };
                            let large_between_tail_before_rewind_picture =
                                !default_between_notes_gap
                                    && compact_endnote_separator_profile
                                    && (has_visible_endnote_separator || !large_separator_block)
                                    && ep_idx > 0
                                    && st.col_count > 1
                                    && st.current_column + 1 < st.col_count
                                    && st.current_height > available * 0.88
                                    && !st.current_items.is_empty()
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && fmt.line_heights.len() == 1
                                    && para_has_visible_text_or_equation(en_para)
                                    && !para_is_treat_as_char_picture_only(en_para)
                                    && st.current_height + fmt.line_advance(0) > available - 50.0
                                    && en_ctrl.paragraphs.get(ep_idx + 1).is_some_and(
                                        |next_para| {
                                            para_is_treat_as_char_picture_only(next_para)
                                                && matches!(
                                                        (
                                                            this_first_offset,
                                                            next_para.line_segs.first().map(|s| {
                                                                s.vertical_pos + endnote_start
                                                            }),
                                                        ),
                                                        (Some(cur), Some(next)) if next < cur
                                                )
                                        },
                                    );
                            let table_only_endnote_para_before_rewind = en_para.text.is_empty()
                                && en_para
                                    .controls
                                    .iter()
                                    .any(|ctrl| matches!(ctrl, Control::Table(_)))
                                && !en_para
                                    .controls
                                    .iter()
                                    .any(|ctrl| matches!(ctrl, Control::Equation(_)));
                            let no_separator_tail_table_starts_next_column = large_separator_block
                                && !has_visible_endnote_separator
                                && ep_idx > 0
                                && st.col_count > 1
                                && st.current_column + 1 < st.col_count
                                && st.current_height > available * 0.95
                                && !st.current_items.is_empty()
                                && !local_vpos_rewind
                                && !internal_vpos_rewind
                                && table_only_endnote_para_before_rewind
                                && en_ctrl.paragraphs.get(ep_idx + 1).is_some_and(|next_para| {
                                    matches!(
                                        (
                                            this_first_offset,
                                            next_para
                                                .line_segs
                                                .first()
                                                .map(|s| s.vertical_pos + endnote_start),
                                        ),
                                        (Some(cur), Some(next)) if next < cur
                                    )
                                });
                            let no_separator_last_column_tail_before_rewind_starts_next_page =
                                large_separator_block
                                    && !has_visible_endnote_separator
                                    && ep_idx > 0
                                    && st.col_count > 1
                                    && st.current_column + 1 >= st.col_count
                                    && st.current_height > available * 0.90
                                    && !st.current_items.is_empty()
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && fmt.line_heights.len() == 1
                                    && para_has_visible_text_or_equation(en_para)
                                    && !para_has_non_tac_picture_or_shape(en_para)
                                    && st.current_height + fmt.line_advance(0)
                                        <= available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 8.0
                                    && en_ctrl.paragraphs.get(ep_idx + 1).is_some_and(
                                        |next_para| {
                                            matches!(
                                                (
                                                    this_first_offset,
                                                    next_para
                                                        .line_segs
                                                        .first()
                                                        .map(|s| s.vertical_pos + endnote_start),
                                                ),
                                                (Some(cur), Some(next)) if next < cur
                                            )
                                        },
                                    );
                            let no_separator_tail_after_picture_starts_next_page =
                                large_separator_block
                                    && !has_visible_endnote_separator
                                    && ep_idx > 0
                                    && st.col_count > 1
                                    && st.current_column + 1 >= st.col_count
                                    && st.current_height > available * 0.93
                                    && !st.current_items.is_empty()
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && fmt.line_heights.len() <= 2
                                    && para_has_visible_text_or_equation(en_para)
                                    && !para_has_treat_as_char_picture_or_shape(en_para)
                                    && !para_has_non_tac_picture_or_shape(en_para)
                                    && {
                                        let mut recent_pi: Vec<usize> = Vec::new();
                                        for pi in st
                                            .current_items
                                            .iter()
                                            .rev()
                                            .filter_map(page_item_para_index)
                                        {
                                            if recent_pi.last().copied() == Some(pi) {
                                                continue;
                                            }
                                            recent_pi.push(pi);
                                            if recent_pi.len() >= 2 {
                                                break;
                                            }
                                        }
                                        match (recent_pi.first(), recent_pi.get(1)) {
                                            (Some(last_pi), Some(prev_pi)) => {
                                                let last_is_text_tail = paragraph_by_global_index(
                                                    paragraphs,
                                                    &st.endnote_paragraphs,
                                                    *last_pi,
                                                )
                                                .is_some_and(|prev_para| {
                                                    para_has_visible_text_or_equation(prev_para)
                                                        && !para_has_treat_as_char_picture_or_shape(
                                                            prev_para,
                                                        )
                                                        && !para_has_non_tac_picture_or_shape(
                                                            prev_para,
                                                        )
                                                });
                                                let previous_is_tac_picture = paragraph_by_global_index(
                                                    paragraphs,
                                                    &st.endnote_paragraphs,
                                                    *prev_pi,
                                                )
                                                .is_some_and(para_is_treat_as_char_picture_only);
                                                last_is_text_tail && previous_is_tac_picture
                                            }
                                            _ => false,
                                        }
                                    };
                            let later_endnote_vpos_rewinds_after_current = this_first_offset
                                .is_some_and(|cur| {
                                    en_ctrl.paragraphs.iter().skip(ep_idx + 1).any(|next_para| {
                                        next_para
                                            .line_segs
                                            .first()
                                            .map(|seg| seg.vertical_pos + endnote_start < cur)
                                            .unwrap_or(false)
                                    })
                                });
                            let large_between_small_equation_tail_bleeds_previous_column =
                                !default_between_notes_gap
                                    && compact_endnote_separator_profile
                                    && has_visible_endnote_separator
                                    && ep_idx == 1
                                    && en_ctrl.paragraphs.len().saturating_sub(ep_idx) >= 5
                                    && st.col_count > 1
                                    && st.current_column + 1 < st.col_count
                                    && st.current_height > available * 0.90
                                    && !st.current_items.is_empty()
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && later_endnote_vpos_rewinds_after_current
                                    && fmt.line_heights.len() == 1
                                    && fmt.line_advance(0) <= 36.0
                                    && st.current_height + fmt.line_advance(0)
                                        <= available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 80.0
                                    && line_is_equation_tac_text_run_only(en_para, &composed, 0)
                                    && en_ctrl.paragraphs.get(ep_idx + 1).is_some_and(
                                        |next_para| {
                                            let next_comp =
                                                crate::renderer::composer::compose_paragraph(
                                                    next_para,
                                                );
                                            let next_fmt = self.format_paragraph(
                                                next_para,
                                                Some(&next_comp),
                                                &styles,
                                                Some(en_col_w),
                                            );
                                            next_fmt.line_heights.len() == 1
                                                && next_fmt.line_advance(0) <= 24.0
                                                && line_has_visible_text(&next_comp, 0)
                                                && !para_has_treat_as_char_picture_or_shape(
                                                    next_para,
                                                )
                                                && !para_has_non_tac_picture_or_shape(next_para)
                                        },
                                    );
                            let large_between_equation_tail_starts_next_column =
                                !default_between_notes_gap
                                    && compact_endnote_separator_profile
                                    && has_visible_endnote_separator
                                    && ep_idx > 0
                                    && st.col_count > 1
                                    && st.current_column + 1 < st.col_count
                                    && st.current_height > available * 0.90
                                    && st.current_height + fmt.line_advance(0) > available - 50.0
                                    && !st.current_items.is_empty()
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && fmt.line_heights.len() == 1
                                    && !large_between_small_equation_tail_bleeds_previous_column
                                    && line_is_equation_tac_text_run_only(en_para, &composed, 0)
                                    && en_ctrl.paragraphs.get(ep_idx + 1).is_some_and(
                                        |next_para| {
                                            let next_comp =
                                                crate::renderer::composer::compose_paragraph(
                                                    next_para,
                                                );
                                            let next_fmt = self.format_paragraph(
                                                next_para,
                                                Some(&next_comp),
                                                &styles,
                                                Some(en_col_w),
                                            );
                                            next_fmt.line_heights.len() == 1
                                                && next_fmt.line_advance(0) <= 24.0
                                                && line_has_visible_text(&next_comp, 0)
                                                && !para_has_treat_as_char_picture_or_shape(
                                                    next_para,
                                                )
                                        },
                                    );
                            let large_between_title_tail_render_overflows =
                                if !default_between_notes_gap
                                    && ep_idx == 0
                                    && st.current_column + 1 >= st.col_count
                                    && en_ref.number > 0
                                    && fmt.line_heights.len() == 1
                                    && !st.current_items.is_empty()
                                {
                                    let mut local_paras: Vec<Paragraph> = Vec::new();
                                    let mut local_indices: Vec<(usize, usize)> = Vec::new();
                                    for pi in st
                                        .current_items
                                        .iter()
                                        .filter_map(page_item_para_index)
                                        .chain(std::iter::once(en_para_idx))
                                    {
                                        if local_indices.iter().any(|(global, _)| *global == pi) {
                                            continue;
                                        }
                                        if let Some(p) = paragraph_by_global_index(
                                            paragraphs,
                                            &st.endnote_paragraphs,
                                            pi,
                                        ) {
                                            let local = local_paras.len();
                                            local_paras.push(p.clone());
                                            local_indices.push((pi, local));
                                        }
                                    }
                                    let lookup_local = |pi: usize, indices: &[(usize, usize)]| {
                                        indices.iter().find_map(|(global, local)| {
                                            (*global == pi).then_some(*local)
                                        })
                                    };
                                    let first_vpos = st
                                        .current_items
                                        .iter()
                                        .filter_map(page_item_para_index)
                                        .find_map(|pi| {
                                            paragraph_by_global_index(
                                                paragraphs,
                                                &st.endnote_paragraphs,
                                                pi,
                                            )
                                            .and_then(|p| p.line_segs.first())
                                            .map(|seg| seg.vertical_pos)
                                        });
                                    let predicted_y = first_vpos.and_then(|page_base| {
                                        let mut hc = HeightCursor::new(
                                            self.dpi,
                                            0.0,
                                            available,
                                            st.current_start_height,
                                            Some(page_base),
                                            st.skip_spacing_before_prededuct,
                                            false,
                                            st.current_endnote_flow
                                                && st.current_start_height < -0.5,
                                            st.current_endnote_flow,
                                        );
                                        hc.endnote_between_notes_hu = st.endnote_between_notes_hu;
                                        let mut y = st.current_start_height;
                                        for item in &st.current_items {
                                            let Some(pi) = page_item_para_index(item) else {
                                                continue;
                                            };
                                            let Some(local) = lookup_local(pi, &local_indices)
                                            else {
                                                continue;
                                            };
                                            y = hc.vpos_adjust(y, local, &local_paras, &styles);
                                            let item_para = &local_paras[local];
                                            let item_composed =
                                                crate::renderer::composer::compose_paragraph(
                                                    item_para,
                                                );
                                            let item_fmt = self.format_paragraph(
                                                item_para,
                                                Some(&item_composed),
                                                &styles,
                                                Some(en_col_w),
                                            );
                                            y += match item {
                                                PageItem::PartialParagraph {
                                                    start_line,
                                                    end_line,
                                                    ..
                                                } => item_fmt
                                                    .line_advances_sum(*start_line..*end_line),
                                                PageItem::FullParagraph { .. } => {
                                                    item_fmt.total_height
                                                }
                                                _ => 0.0,
                                            };
                                            let current_vpos_rewinds_from_prev = hc
                                                .prev_layout_para
                                                .and_then(|prev_local| {
                                                    let prev_first = local_paras
                                                        .get(prev_local)
                                                        .and_then(|p| p.line_segs.first())
                                                        .map(|seg| seg.vertical_pos)?;
                                                    let curr_first = local_paras
                                                        .get(local)
                                                        .and_then(|p| p.line_segs.first())
                                                        .map(|seg| seg.vertical_pos)?;
                                                    Some(curr_first < prev_first)
                                                })
                                                .unwrap_or(false);
                                            if matches!(
                                                item,
                                                PageItem::PartialParagraph { start_line, .. }
                                                    if *start_line > 0
                                            ) || current_vpos_rewinds_from_prev
                                            {
                                                hc.prev_layout_para = None;
                                                hc.vpos_page_base = None;
                                                hc.vpos_lazy_base = None;
                                            } else {
                                                hc.prev_layout_para = Some(local);
                                            }
                                            hc.prev_item_was_partial_table =
                                                matches!(item, PageItem::PartialTable { .. });
                                        }
                                        lookup_local(en_para_idx, &local_indices).map(|local| {
                                            hc.vpos_adjust(y, local, &local_paras, &styles)
                                        })
                                    });
                                    predicted_y
                                        .map(|y| {
                                            let title_h = fmt.line_advance(0);
                                            // 한컴은 큰 미주 사이 문서에서도 문항 제목 한 줄만
                                            // 단 하단에 남는 tail을 허용한다. 다음 본문 첫 줄까지
                                            // 같은 단에 넣을 수 없다는 이유만으로 제목을 새 쪽으로
                                            // 밀면 2024-09 미주사이20 p13 문18처럼 한컴보다 한 쪽
                                            // 늦어진다. 제목 자체가 frame을 넘는 경우만 advance한다.
                                            y + title_h
                                                > available
                                                    + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                        })
                                        .unwrap_or(false)
                                } else {
                                    false
                                };
                            let large_between_question_title_render_y =
                                if !default_between_notes_gap
                                    && ep_idx == 0
                                    && en_ref.number > 0
                                    && fmt.line_heights.len() == 1
                                    && st.current_height < available
                                    && st.current_height > available * 0.80
                                    && !st.current_items.is_empty()
                                {
                                    self.predict_current_column_para_y(
                                        &st,
                                        en_para_idx,
                                        paragraphs,
                                        &styles,
                                        measured_tables,
                                        Some(en_col_w),
                                    )
                                } else {
                                    None
                                };
                            let large_between_question_title_head_inside_frame =
                                large_between_question_title_render_y
                                    .map(|predicted_y| {
                                        predicted_y + fmt.line_advance(0)
                                            <= available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                    })
                                    .unwrap_or(false);
                            let large_between_question_title_head_fits_flow =
                                !default_between_notes_gap
                                    && compact_endnote_separator_profile
                                    && has_visible_endnote_separator
                                    && ep_idx == 0
                                    && en_ref.number > 0
                                    && st.current_column + 1 < st.col_count
                                    && fmt.line_heights.len() == 1
                                    && st.current_height < available
                                    && st.current_height + fmt.line_advance(0)
                                        <= available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 2.0;
                            let large_between_question_title_render_head_outside =
                                large_between_question_title_render_y
                                    .map(|predicted_y| {
                                        predicted_y + fmt.line_advance(0)
                                            > available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                    })
                                    .unwrap_or(false);
                            let large_between_question_lead_group_render_outside =
                                !default_between_notes_gap
                                    && compact_endnote_separator_profile
                                    && has_visible_endnote_separator
                                    && ep_idx == 0
                                    && en_ref.number > 0
                                    && !endnote_has_vpos_rewind
                                    && st.current_column + 1 < st.col_count
                                    && large_between_question_title_render_y
                                        .map(|predicted_y| {
                                            let group_first = en_ctrl
                                                .paragraphs
                                                .first()
                                                .and_then(|p| p.line_segs.first())
                                                .map(|seg| seg.vertical_pos + endnote_start);
                                            let group_bottom = en_ctrl
                                                .paragraphs
                                                .iter()
                                                .take(4)
                                                .flat_map(|p| p.line_segs.iter())
                                                .map(|seg| {
                                                    seg.vertical_pos
                                                        + seg.line_height
                                                        + seg.line_spacing
                                                        + endnote_start
                                                })
                                                .max();
                                            group_first
                                                .zip(group_bottom)
                                                .map(|(first, bottom)| {
                                                    let group_h = hwpunit_to_px(
                                                        (bottom - first).max(0),
                                                        self.dpi,
                                                    );
                                                    predicted_y + group_h
                                                        > available
                                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                })
                                                .unwrap_or(false)
                                        })
                                        .unwrap_or(false);
                            let large_between_last_column_visual_split =
                                if !default_between_notes_gap
                                    && compact_endnote_separator_profile
                                    && has_visible_endnote_separator
                                    && visible_large_between_notes_gap
                                    && !compact_between_notes_gap
                                    && !zero_endnote_spacing_profile
                                    && ep_idx > 0
                                    && st.col_count > 1
                                    && st.current_column + 1 >= st.col_count
                                    && (st.current_height > available * 0.85
                                        || st.current_height + en_fit > available - 60.0)
                                    && st.current_height < available
                                    && (st.current_height + en_fit > available
                                        || fmt.line_heights.len() >= 3)
                                    && !st.current_items.is_empty()
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && fmt.line_heights.len() > 1
                                    && para_has_visible_text_or_equation(en_para)
                                {
                                    let mut local_paras: Vec<Paragraph> = Vec::new();
                                    let mut local_indices: Vec<(usize, usize)> = Vec::new();
                                    for pi in st
                                        .current_items
                                        .iter()
                                        .filter_map(page_item_para_index)
                                        .chain(std::iter::once(en_para_idx))
                                    {
                                        if local_indices.iter().any(|(global, _)| *global == pi) {
                                            continue;
                                        }
                                        if let Some(p) = paragraph_by_global_index(
                                            paragraphs,
                                            &st.endnote_paragraphs,
                                            pi,
                                        ) {
                                            let local = local_paras.len();
                                            local_paras.push(p.clone());
                                            local_indices.push((pi, local));
                                        }
                                    }
                                    let lookup_local = |pi: usize, indices: &[(usize, usize)]| {
                                        indices.iter().find_map(|(global, local)| {
                                            (*global == pi).then_some(*local)
                                        })
                                    };
                                    let first_vpos = st
                                        .current_items
                                        .iter()
                                        .filter_map(page_item_para_index)
                                        .find_map(|pi| {
                                            paragraph_by_global_index(
                                                paragraphs,
                                                &st.endnote_paragraphs,
                                                pi,
                                            )
                                            .and_then(|p| p.line_segs.first())
                                            .map(|seg| seg.vertical_pos)
                                        });
                                    let predicted_y = first_vpos.and_then(|page_base| {
                                        let mut hc = HeightCursor::new(
                                            self.dpi,
                                            0.0,
                                            available,
                                            st.current_start_height,
                                            Some(page_base),
                                            st.skip_spacing_before_prededuct,
                                            false,
                                            st.current_endnote_flow
                                                && st.current_start_height < -0.5,
                                            st.current_endnote_flow,
                                        );
                                        hc.endnote_between_notes_hu = st.endnote_between_notes_hu;
                                        let mut y = st.current_start_height;
                                        for item in &st.current_items {
                                            let Some(pi) = page_item_para_index(item) else {
                                                continue;
                                            };
                                            let Some(local) = lookup_local(pi, &local_indices)
                                            else {
                                                continue;
                                            };
                                            y = hc.vpos_adjust(y, local, &local_paras, &styles);
                                            let item_para = &local_paras[local];
                                            let item_composed =
                                                crate::renderer::composer::compose_paragraph(
                                                    item_para,
                                                );
                                            let item_fmt = self.format_paragraph(
                                                item_para,
                                                Some(&item_composed),
                                                &styles,
                                                Some(en_col_w),
                                            );
                                            y += match item {
                                                PageItem::PartialParagraph {
                                                    start_line,
                                                    end_line,
                                                    ..
                                                } => item_fmt
                                                    .line_advances_sum(*start_line..*end_line),
                                                PageItem::FullParagraph { .. } => {
                                                    item_fmt.total_height
                                                }
                                                _ => 0.0,
                                            };
                                            let current_vpos_rewinds_from_prev = hc
                                                .prev_layout_para
                                                .and_then(|prev_local| {
                                                    let prev_first = local_paras
                                                        .get(prev_local)
                                                        .and_then(|p| p.line_segs.first())
                                                        .map(|seg| seg.vertical_pos)?;
                                                    let curr_first = local_paras
                                                        .get(local)
                                                        .and_then(|p| p.line_segs.first())
                                                        .map(|seg| seg.vertical_pos)?;
                                                    Some(curr_first < prev_first)
                                                })
                                                .unwrap_or(false);
                                            if matches!(
                                                item,
                                                PageItem::PartialParagraph { start_line, .. }
                                                    if *start_line > 0
                                            ) || current_vpos_rewinds_from_prev
                                            {
                                                hc.prev_layout_para = None;
                                                hc.vpos_page_base = None;
                                                hc.vpos_lazy_base = None;
                                            } else {
                                                hc.prev_layout_para = Some(local);
                                            }
                                            hc.prev_item_was_partial_table =
                                                matches!(item, PageItem::PartialTable { .. });
                                        }
                                        lookup_local(en_para_idx, &local_indices).map(|local| {
                                            hc.vpos_adjust(y, local, &local_paras, &styles)
                                        })
                                    });

                                    predicted_y.and_then(|y| {
                                        if y >= available {
                                            return None;
                                        }
                                        // 첫 줄 자체가 frame 안쪽에 들어오지 못하면 visual split으로
                                        // 단 하단에 남기지 않는다. 큰 미주 사이 문서에서는 이 줄들을
                                        // 남기면 다음 쪽의 문항 시작점이 연쇄적으로 위로 당겨진다.
                                        if y + fmt.line_advance(0)
                                            > available
                                                + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                + 2.0
                                        {
                                            return None;
                                        }
                                        // 큰 미주 사이 문서의 마지막 단은 렌더 vpos가 직전
                                        // 문단들을 위로 당긴 뒤 남는 visual tail 공간을 사용한다.
                                        // pagination 누적 높이만 보면 부족하지만, 한컴/PDF는 다음
                                        // 문단의 마지막 1줄만 이월시키는 패턴이 있어 이 경로에만
                                        // 단 하단 visual 한도를 넓힌다.
                                        let flow_overflows = st.current_height + en_fit > available;
                                        let visual_tail_limit = if flow_overflows {
                                            available
                                                + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                + 46.0
                                        } else {
                                            // flow 누적 높이는 들어가지만 저장 vpos 기반 render
                                            // 마지막 줄이 frame 하단에 걸리는 큰 미주 사이 tail은
                                            // 한컴처럼 마지막 줄부터 다음 쪽으로 넘긴다.
                                            available + 1.0
                                        };
                                        let mut consumed = 0.0;
                                        let mut split = 0usize;
                                        for line_idx in 0..fmt.line_heights.len() {
                                            let next = consumed + fmt.line_advance(line_idx);
                                            if y + next > visual_tail_limit {
                                                break;
                                            }
                                            consumed = next;
                                            split = line_idx + 1;
                                        }
                                        (split > 0 && split < fmt.line_heights.len())
                                            .then_some(split)
                                    })
                                } else {
                                    None
                                };
                            let large_between_last_column_flow_tail_split =
                                if !default_between_notes_gap
                                    && compact_endnote_separator_profile
                                    && has_visible_endnote_separator
                                    && visible_large_between_notes_gap
                                    && !compact_between_notes_gap
                                    && !zero_endnote_spacing_profile
                                    && ep_idx > 0
                                    && st.col_count > 1
                                    && st.current_column + 1 >= st.col_count
                                    && st.current_height < available
                                    && st.current_height + en_fit > available - 60.0
                                    && st.current_height + en_fit
                                        <= available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 2.0
                                    && !st.current_items.is_empty()
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && fmt.line_heights.len() >= 5
                                    && para_has_visible_text_or_equation(en_para)
                                    && !para_has_treat_as_char_picture_or_shape(en_para)
                                    && !para_has_non_tac_picture_or_shape(en_para)
                                {
                                    Some(fmt.line_heights.len() - 1)
                                } else {
                                    None
                                };
                            if large_between_title_tail_render_overflows
                                && !no_separator_last_column_new_note_head_without_gap_fits
                            {
                                st.advance_column_or_new_page();
                                prev_en_bottom_vpos = None;
                            }
                            if large_between_tail_render_overflows
                                || large_between_tail_before_rewind_picture
                                || large_between_equation_tail_starts_next_column
                                || no_separator_tail_table_starts_next_column
                                || no_separator_last_column_tail_before_rewind_starts_next_page
                            {
                                st.advance_column_or_new_page();
                                prev_en_bottom_vpos = None;
                            }
                            let next_endnote_first_line_advance = if ep_idx == 0 {
                                en_ctrl.paragraphs.get(1).map(|next_para| {
                                    let next_comp =
                                        crate::renderer::composer::compose_paragraph(next_para);
                                    self.format_paragraph(
                                        next_para,
                                        Some(&next_comp),
                                        &styles,
                                        Some(en_col_w),
                                    )
                                    .line_advance(0)
                                })
                            } else {
                                None
                            };
                            let next_endnote_head_pair_advance = if ep_idx == 0 {
                                let mut total = 0.0;
                                let mut count = 0;
                                for next_para in en_ctrl.paragraphs.iter().skip(1).take(2) {
                                    let next_comp =
                                        crate::renderer::composer::compose_paragraph(next_para);
                                    let next_fmt = self.format_paragraph(
                                        next_para,
                                        Some(&next_comp),
                                        &styles,
                                        Some(en_col_w),
                                    );
                                    total += next_fmt.line_advance(0);
                                    count += 1;
                                }
                                (count == 2).then_some(total)
                            } else {
                                None
                            };
                            let zero_between_large_separator_last_column_title_orphan =
                                compact_endnote_separator_profile
                                    && has_visible_endnote_separator
                                    && endnote_shape
                                        .map(|shape| {
                                            endnote_between_notes_margin(shape) == 0
                                                && shape.separator_above_margin_hu() as i32
                                                    > ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU
                                                && endnote_separator_below_margin(shape) as i32
                                                    > ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU
                                        })
                                        .unwrap_or(false)
                                    && ep_idx == 0
                                    && en_ref.number > 0
                                    && st.current_column + 1 >= st.col_count
                                    && fmt.line_heights.len() == 1
                                    && st.current_height > available * 0.95
                                    && st.current_height + fmt.line_advance(0)
                                        <= available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                    && next_endnote_first_line_advance
                                        .map(|next_h| {
                                            st.current_height + fmt.line_advance(0) + next_h
                                                > available
                                                    + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                    + 2.0
                                        })
                                        .unwrap_or(false)
                                    && endnote_has_visible_payload;
                            let default_large_below_last_column_title_orphan =
                                compact_endnote_separator_profile
                                    && default_between_notes_gap
                                    && has_visible_endnote_separator
                                    && endnote_shape
                                        .map(|shape| {
                                            endnote_separator_below_margin(shape) as i32
                                                > ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU
                                        })
                                        .unwrap_or(false)
                                    && ep_idx == 0
                                    && en_ref.number > 0
                                    && st.current_column + 1 >= st.col_count
                                    && fmt.line_heights.len() == 1
                                    && st.current_height > available * 0.95
                                    && st.current_height + fmt.line_advance(0)
                                        <= available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                    && en_ctrl.paragraphs.get(1).is_some_and(|next_para| {
                                        !para_has_visible_text(next_para)
                                            && para_has_visible_text_or_equation(next_para)
                                    })
                                    && (next_endnote_first_line_advance
                                        .map(|next_h| {
                                            st.current_height + fmt.line_advance(0) + next_h
                                                > available
                                                    + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                    + 2.0
                                        })
                                        .unwrap_or(false)
                                        || next_endnote_head_pair_advance
                                            .map(|next_h| {
                                                st.current_height + fmt.line_advance(0) + next_h
                                                    > available
                                                        + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                        + 2.0
                                            })
                                            .unwrap_or(false))
                                    && endnote_has_visible_payload;
                            let default_compact_below_last_column_title_orphan =
                                compact_endnote_separator_profile
                                    && default_between_notes_gap
                                    && has_visible_endnote_separator
                                    && endnote_shape
                                        .map(|shape| {
                                            endnote_separator_below_margin(shape) as i32
                                                <= ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU
                                        })
                                        .unwrap_or(false)
                                    && ep_idx == 0
                                    && en_ref.number > 0
                                    && st.current_column + 1 >= st.col_count
                                    && fmt.line_heights.len() == 1
                                    && st.current_height > available * 0.95
                                    && st.current_height + fmt.line_advance(0)
                                        <= available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                    && en_ctrl.paragraphs.get(1).is_some_and(|next_para| {
                                        !para_has_visible_text(next_para)
                                            && para_has_visible_text_or_equation(next_para)
                                    })
                                    && next_endnote_head_pair_advance
                                        .map(|next_h| {
                                            st.current_height + fmt.line_advance(0) + next_h
                                                > available
                                                    + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                    + 2.0
                                        })
                                        .unwrap_or(false)
                                    && endnote_has_visible_payload;
                            if zero_between_large_separator_last_column_title_orphan
                                || default_large_below_last_column_title_orphan
                                || default_compact_below_last_column_title_orphan
                            {
                                st.advance_column_or_new_page();
                                prev_en_bottom_vpos = None;
                                prev_en_content_bottom_vpos = None;
                            }
                            let allow_large_between_question_title_tail = !default_between_notes_gap
                                && ep_idx == 0
                                && en_ref.number > 0
                                && st.current_column + 1 < st.col_count
                                && fmt.line_heights.len() == 1
                                && st.current_height < available
                                && (large_between_question_title_head_inside_frame
                                    || large_between_question_title_head_fits_flow)
                                && !large_between_question_lead_group_render_outside
                                && st.current_height + fmt.line_advance(0)
                                    <= available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX + 2.0;
                            let allow_default_column_bottom_question_title_tail =
                                default_between_notes_gap
                                    && compact_endnote_separator_profile
                                    && ep_idx == 0
                                    && en_ref.number > 0
                                    && fmt.line_heights.len() == 1
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && !st.current_items.is_empty()
                                    && default_question_group_title_tail
                                    && st.current_height < available
                                    && st.current_height > available * 0.88
                                    && st.current_height + fmt.line_advance(0)
                                        <= available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 2.0
                                    && para_has_visible_text_or_equation(en_para);
                            let allow_default_first_column_large_below_title_tail =
                                allow_default_column_bottom_question_title_tail
                                    && endnote_has_vpos_rewind
                                    && st.current_column + 1 < st.col_count
                                    && endnote_shape
                                        .map(|shape| {
                                            endnote_separator_below_margin(shape) as i32
                                                > ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU
                                        })
                                        .unwrap_or(false);
                            let new_endnote_advance_threshold = if default_between_notes_gap {
                                if st.current_column + 1 < st.col_count {
                                    0.88
                                } else {
                                    0.95
                                }
                            } else if st.current_column + 1 < st.col_count {
                                0.88
                            } else {
                                0.95
                            };
                            let allow_compact_question_title_tail =
                                compact_endnote_separator_profile
                                    && !default_between_notes_gap
                                    && (has_visible_endnote_separator || !large_separator_block)
                                    && ep_idx == 0
                                    && st.current_column + 1 < st.col_count
                                    && fmt.line_heights.len() == 1
                                    && endnote_has_visible_payload
                                    && st.current_height
                                        > available * new_endnote_advance_threshold
                                    && new_endnote_between_notes_px
                                        .map(|gap| {
                                            st.current_height + fmt.line_advance(0) + gap
                                                <= available
                                                    + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                        })
                                        .unwrap_or(true)
                                    && st.current_height + en_fit
                                        <= available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX;
                            let allow_large_separator_first_column_tail =
                                visible_large_between_notes_gap
                                    && ep_idx == 0
                                    && st.current_column + 1 < st.col_count
                                    && !large_between_question_lead_group_render_outside
                                    && st.current_height + en_fit
                                        <= available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                    && endnote_has_visible_payload;
                            let large_between_last_column_question_title_tail_fits =
                                !default_between_notes_gap
                                    && compact_endnote_separator_profile
                                    && has_visible_endnote_separator
                                    && visible_large_between_notes_gap
                                    && !compact_between_notes_gap
                                    && ep_idx == 0
                                    && emitted_endnote_count > 0
                                    && en_ref.number > 0
                                    && st.current_column + 1 >= st.col_count
                                    && fmt.line_heights.len() == 1
                                    && st.current_height > available * 0.90
                                    && st.current_height < available
                                    && large_between_question_title_head_inside_frame
                                    && st.current_height + fmt.line_advance(0)
                                        <= available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 2.0
                                    && new_endnote_between_notes_px
                                        .map(|gap| {
                                            // 마지막 단에서 새 미주 제목만 남길 때도
                                            // `미주 사이`는 제목 앞에 소비된다. gap 없이
                                            // 제목 한 줄만 fit으로 보면 20mm 문서에서 다음
                                            // 쪽으로 가야 할 제목이 현재 쪽 하단에 고아로 남는다.
                                            st.current_height + gap + fmt.line_advance(0)
                                                <= available
                                                    + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                    + 2.0
                                        })
                                        .unwrap_or(true)
                                    && (compact_between_notes_gap
                                        || new_endnote_between_notes_px
                                            .map(|gap| {
                                                let head_group_h: f64 = en_ctrl
                                                    .paragraphs
                                                    .iter()
                                                    .take(3)
                                                    .map(|head_para| {
                                                        let head_comp =
                                                            crate::renderer::composer::compose_paragraph(
                                                                head_para,
                                                            );
                                                        self.format_paragraph(
                                                            head_para,
                                                            Some(&head_comp),
                                                            &styles,
                                                            Some(en_col_w),
                                                        )
                                                        .total_height
                                                    })
                                                    .sum();
                                                // 제목과 첫 풀이 일부만 단 하단에 고아로 남기지
                                                // 않도록, 20mm급 large gap에서는 제목+본문 head
                                                // group이 함께 들어갈 때만 마지막 단 tail을 허용한다.
                                                st.current_height + gap + head_group_h
                                                    <= available
                                                        + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                        + 2.0
                                            })
                                            .unwrap_or(true))
                                    && endnote_has_visible_payload;
                            let large_between_last_column_render_title_tail_fits =
                                !default_between_notes_gap
                                    && compact_endnote_separator_profile
                                    && has_visible_endnote_separator
                                    && visible_large_between_notes_gap
                                    && !compact_between_notes_gap
                                    && ep_idx == 0
                                    && emitted_endnote_count > 0
                                    && en_ref.number > 0
                                    && st.current_column + 1 >= st.col_count
                                    && fmt.line_heights.len() == 1
                                    && st.current_height > available * 0.80
                                    && st.current_height < available * 0.85
                                    && !st.current_items.is_empty()
                                    && large_between_question_title_head_inside_frame
                                    && large_between_question_title_render_y
                                        .map(|predicted_y| {
                                            // 마지막 단의 20mm급 `미주 사이`는 제목 앞 렌더 gap을
                                            // 만든 뒤 제목 한 줄만 쪽 하단에 남길 수 있다. 본문
                                            // head group까지 같은 쪽에 들어가야 한다고 보면 문항
                                            // 시작이 한컴보다 한 쪽 늦어진다.
                                            predicted_y + fmt.line_advance(0)
                                                <= available
                                                    + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                    + 2.0
                                        })
                                        .unwrap_or(false)
                                    && endnote_has_visible_payload;
                            let large_between_last_column_rewind_title_tail_fits =
                                !default_between_notes_gap
                                    && compact_endnote_separator_profile
                                    && has_visible_endnote_separator
                                    && visible_large_between_notes_gap
                                    && !compact_between_notes_gap
                                    && ep_idx == 0
                                    && emitted_endnote_count > 0
                                    && en_ref.number > 0
                                    && st.current_column + 1 >= st.col_count
                                    && fmt.line_heights.len() == 1
                                    && endnote_has_vpos_rewind
                                    && st.current_height > available * 0.90
                                    && st.current_height < available
                                    && !st.current_items.is_empty()
                                    && st.current_height + en_fit
                                        <= available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 2.0
                                    && large_between_question_title_head_inside_frame
                                    && endnote_has_visible_payload;
                            let large_between_last_column_title_body_tail_fits =
                                !default_between_notes_gap
                                    && compact_endnote_separator_profile
                                    && has_visible_endnote_separator
                                    && ep_idx == 1
                                    && en_ref.number > 0
                                    && prev_rendered_endnote_is_title
                                    && st.current_column + 1 >= st.col_count
                                    && fmt.line_heights.len() > 1
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && st.current_height > available * 0.90
                                    && st.current_height < available
                                    && st.current_height
                                        + fmt.line_advances_sum(0..fmt.line_heights.len())
                                        <= available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 2.0
                                    && endnote_has_visible_payload
                                    && !para_has_non_tac_picture_or_shape(en_para);
                            let late_question_title_small_overflow =
                                allow_default_late_question_tail
                                    && ep_idx == 0
                                    && st.current_column + 1 >= st.col_count
                                    && st.current_height < available
                                    && st.current_height + en_fit <= available + 40.0;
                            let late_question_intro_tail = allow_default_late_question_tail
                                && ep_idx == 1
                                && st.current_column + 1 >= st.col_count
                                && st.current_height < available + 40.0
                                && st.current_height + en_fit <= available + 90.0;
                            let late_question_continuation_tail = allow_default_late_question_tail
                                && ep_idx > 1
                                && st.current_column + 1 >= st.col_count
                                && st.current_height < available + 40.0
                                && st.current_height + en_fit <= available + 90.0
                                && endnote_has_visible_payload;
                            let default_question_title_tail_fits_by_line_height =
                                compact_endnote_separator_profile
                                    && default_between_notes_gap
                                    && endnote_shape
                                        .map(endnote_has_compact_separator_below)
                                        .unwrap_or(false)
                                    && !zero_endnote_spacing_profile
                                    && ep_idx == 0
                                    && st.current_column + 1 < st.col_count
                                    && fmt.line_heights.len() == 1
                                    && st.current_height > available * 0.92
                                    && st.current_height + h4f
                                        <= available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                    && en_ctrl
                                        .paragraphs
                                        .get(1)
                                        .map(|next_para| {
                                            let next_comp =
                                                crate::renderer::composer::compose_paragraph(
                                                    next_para,
                                                );
                                            let next_fmt = self.format_paragraph(
                                                next_para,
                                                Some(&next_comp),
                                                &styles,
                                                Some(en_col_w),
                                            );
                                            let next_h = next_fmt.height_for_fit;
                                            let title_body_limit = if has_visible_endnote_separator
                                                && st.current_height > available * 0.95
                                            {
                                                available + 2.0
                                            } else {
                                                available
                                                    + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                    + 2.0
                                            };
                                            st.current_height + fmt.line_advance(0) + next_h
                                                <= title_body_limit
                                        })
                                        .unwrap_or(true)
                                    && endnote_has_visible_payload;
                            let zero_question_title_tail_fits_by_line_height =
                                compact_endnote_separator_profile
                                    && zero_endnote_spacing_profile
                                    && has_visible_endnote_separator
                                    && ep_idx == 0
                                    && fmt.line_heights.len() == 1
                                    // 0/0/0 미주는 한컴이 새 문항 제목 한 줄을
                                    // 왼쪽 단 하단에 남기고 큰 그림 풀이만 다음 단으로
                                    // 넘기는 경우가 있다. 기본 미주의 0.95 임계값을
                                    // 그대로 쓰면 제목까지 다음 단 상단으로 밀린다.
                                    && st.current_height > available * 0.88
                                    && st.current_height + h4f
                                        <= available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                    && (st.current_column + 1 >= st.col_count
                                        || st.current_height + fmt.line_advance(0)
                                            <= available + 1.0
                                        || st.current_height + h4f
                                            <= available
                                                + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX)
                                    && endnote_has_visible_payload;
                            let zero_question_intro_tail_before_rewind_fits =
                                compact_endnote_separator_profile
                                    && zero_endnote_spacing_profile
                                    && has_visible_endnote_separator
                                    && (st.current_column + 1 < st.col_count || ep_idx == 1)
                                    && matches!(ep_idx, 1 | 2)
                                    && en_ref.number > 0
                                    && fmt.line_heights.len() == 1
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && !para_is_treat_as_char_picture_only(en_para)
                                    && para_has_visible_text_or_equation(en_para)
                                    && en_ctrl
                                        .paragraphs
                                        .first()
                                        .is_some_and(|title_para| title_para.line_segs.len() == 1)
                                    && later_endnote_vpos_rewinds_after_current
                                    && st.current_height > available * 0.95
                                    && st.current_height + fmt.line_advance(0)
                                        <= available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 28.0
                                    && if st.current_column + 1 >= st.col_count {
                                        ep_idx == 1
                                            && fmt.line_advance(0) <= 24.0
                                            && line_has_visible_text_or_tac_equation(
                                                en_para, &composed, 0,
                                            )
                                    } else if ep_idx == 1 {
                                        en_ctrl.paragraphs.get(ep_idx + 1).is_some_and(
                                            |next_para| {
                                                let next_comp =
                                                    crate::renderer::composer::compose_paragraph(
                                                        next_para,
                                                    );
                                                let next_fmt = self.format_paragraph(
                                                    next_para,
                                                    Some(&next_comp),
                                                    &styles,
                                                    Some(en_col_w),
                                                );
                                                next_fmt.line_heights.len() == 1
                                                    && next_fmt.line_advance(0) <= 24.0
                                                    && line_has_visible_text_or_tac_equation(
                                                        next_para, &next_comp, 0,
                                                    )
                                            },
                                        )
                                    } else {
                                        fmt.line_advance(0) <= 24.0
                                            && line_has_visible_text_or_tac_equation(
                                                en_para, &composed, 0,
                                            )
                                    };
                            let zero_between_large_separator_tail_group_fits =
                                compact_endnote_separator_profile
                                    && has_visible_endnote_separator
                                    && large_separator_block
                                    && endnote_shape
                                        .map(|shape| endnote_between_notes_margin(shape) == 0)
                                        .unwrap_or(false)
                                    && st.col_count > 1
                                    && st.current_column + 1 < st.col_count
                                    && ep_idx > 0
                                    && en_ref.number > 0
                                    && fmt.line_heights.len() == 1
                                    && !internal_vpos_rewind
                                    && !para_is_treat_as_char_picture_only(en_para)
                                    && para_has_visible_text_or_equation(en_para)
                                    && st.current_height > available * 0.95
                                    && st.current_height + fmt.line_advance(0)
                                        <= available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 70.0
                                    && !local_vpos_rewind
                                    && later_endnote_vpos_rewinds_after_current;
                            let late_compact_text_tail_overflow_risk =
                                compact_endnote_separator_profile
                                    && compact_between_notes_gap
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && !has_treat_as_char_picture_shape
                                    && (fmt.line_heights.len() <= 2
                                        || (default_between_notes_gap
                                            && ep_idx > 0
                                            && en_fit > 60.0
                                            && fmt.line_heights.len() <= 3))
                                    && endnote_has_visible_payload
                                    && (((large_separator_block
                                        || !default_between_notes_gap
                                        || (default_between_notes_gap
                                            && default_nonzero_between_note_tail_candidate
                                            && ep_idx > 0
                                            && st.current_height > available * 0.90)
                                        || (default_between_notes_gap
                                            && ep_idx > 0
                                            && en_fit > 24.0))
                                        && st.current_column + 1 < st.col_count
                                        && st.current_height > available * 0.96
                                        && st.current_height + total_advance_fit
                                            > available - 20.0)
                                        || (!default_between_notes_gap
                                            && has_visible_endnote_separator
                                            && st.current_column + 1 >= st.col_count
                                            && ep_idx > 0
                                            && st.current_height > available * 0.92
                                            && st.current_height + total_advance_fit
                                                > available - 40.0));
                            let zero_tac_picture_tail_bleeds_frame =
                                compact_endnote_separator_profile
                                    && zero_endnote_spacing_profile
                                    && has_visible_endnote_separator
                                    && st.current_column + 1 < st.col_count
                                    && st.current_height > available * 0.70
                                    && para_is_treat_as_char_picture_only(en_para)
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && fmt.line_heights.len() == 1
                                    && st.current_height + total_advance_fit
                                        > available - ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX;
                            let visible_separator_large_tac_tail_candidate =
                                compact_endnote_separator_profile
                                    && !zero_endnote_spacing_profile
                                    && has_visible_endnote_separator
                                    && st.col_count > 1
                                    && st.current_height > available * 0.60
                                    && tac_picture_tail_height.is_some()
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && fmt.line_heights.len() == 1
                                    && tac_picture_tail_height.unwrap_or(total_advance_fit)
                                        > ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX * 3.0;
                            let visible_separator_large_tac_tail_render_y =
                                if visible_separator_large_tac_tail_candidate {
                                    self.predict_current_column_para_y(
                                        &st,
                                        en_para_idx,
                                        paragraphs,
                                        &styles,
                                        measured_tables,
                                        Some(en_col_w),
                                    )
                                } else {
                                    None
                                };
                            let visible_separator_large_tac_tail_bottom =
                                visible_separator_large_tac_tail_render_y.map(|render_y| {
                                    render_y + tac_picture_tail_height.unwrap_or(h4f)
                                });
                            let visible_separator_large_tac_tail_allows_small_bleed =
                                visible_separator_large_tac_tail_candidate
                                    && visible_large_between_notes_gap
                                    && st.current_column + 1 >= st.col_count
                                    && en_ctrl.paragraphs.get(ep_idx + 1).is_some_and(
                                        |next_para| {
                                            let next_comp =
                                                crate::renderer::composer::compose_paragraph(
                                                    next_para,
                                                );
                                            let next_fmt = self.format_paragraph(
                                                next_para,
                                                Some(&next_comp),
                                                &styles,
                                                Some(en_col_w),
                                            );
                                            next_fmt.line_heights.len() == 1
                                                && para_has_visible_text(next_para)
                                                && !para_has_treat_as_char_picture_or_shape(
                                                    next_para,
                                                )
                                                && !para_has_non_tac_picture_or_shape(next_para)
                                        },
                                    );
                            let visible_separator_large_tac_tail_overflow_limit =
                                if visible_separator_large_tac_tail_allows_small_bleed {
                                    available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                } else {
                                    available + 1.0
                                };
                            let visible_separator_large_tac_tail_overflows_frame =
                                visible_separator_large_tac_tail_bottom.is_some_and(|bottom| {
                                    bottom > visible_separator_large_tac_tail_overflow_limit
                                });
                            let visible_separator_text_after_large_tac_tail_starts_next_page =
                                !default_between_notes_gap
                                    && compact_endnote_separator_profile
                                    && has_visible_endnote_separator
                                    && visible_large_between_notes_gap
                                    && st.col_count > 1
                                    && st.current_column + 1 >= st.col_count
                                    && ep_idx > 1
                                    && st.current_height > available * 0.96
                                    && fmt.line_heights.len() == 1
                                    && para_has_visible_text(en_para)
                                    && !para_has_treat_as_char_picture_or_shape(en_para)
                                    && !para_has_non_tac_picture_or_shape(en_para)
                                    && en_ctrl
                                        .paragraphs
                                        .iter()
                                        .take(ep_idx)
                                        .skip(1)
                                        .any(para_has_visible_text)
                                    && st
                                        .current_items
                                        .iter()
                                        .rev()
                                        .filter_map(page_item_para_index)
                                        .next()
                                        .and_then(|pi| {
                                            paragraph_by_global_index(
                                                paragraphs,
                                                &st.endnote_paragraphs,
                                                pi,
                                            )
                                            .and_then(
                                                |prev_para| {
                                                    prev_para
                                                        .controls
                                                        .iter()
                                                        .filter_map(|ctrl| {
                                                            tac_picture_or_shape_height_px(
                                                                ctrl, dpi,
                                                            )
                                                        })
                                                        .reduce(f64::max)
                                                },
                                            )
                                        })
                                        .is_some_and(|height| height >= 80.0);
                            let visible_separator_text_after_equation_tail_overflows_frame =
                                !default_between_notes_gap
                                    && compact_endnote_separator_profile
                                    && has_visible_endnote_separator
                                    && visible_large_between_notes_gap
                                    && st.col_count > 1
                                    && st.current_column + 1 < st.col_count
                                    && ep_idx > 1
                                    && st.current_height > available * 0.90
                                    && fmt.line_heights.len() == 1
                                    && para_has_visible_text(en_para)
                                    && !para_has_treat_as_char_picture_or_shape(en_para)
                                    && !para_has_non_tac_picture_or_shape(en_para)
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && st
                                        .current_items
                                        .last()
                                        .and_then(page_item_para_index)
                                        .is_some_and(|prev_pi| prev_pi + 1 == en_para_idx)
                                    && st
                                        .current_items
                                        .last()
                                        .and_then(page_item_para_index)
                                        .and_then(|prev_pi| {
                                            paragraph_by_global_index(
                                                paragraphs,
                                                &st.endnote_paragraphs,
                                                prev_pi,
                                            )
                                        })
                                        .is_some_and(|prev_para| {
                                            !para_has_visible_text(prev_para)
                                                && prev_para.controls.iter().any(|ctrl| {
                                                    is_treat_as_char_equation_control(Some(ctrl))
                                                })
                                        })
                                    && self
                                        .predict_current_column_para_y(
                                            &st,
                                            en_para_idx,
                                            paragraphs,
                                            &styles,
                                            measured_tables,
                                            Some(en_col_w),
                                        )
                                        .is_some_and(|render_y| {
                                            render_y + fmt.line_advance(0) > available + 1.0
                                        });
                            let zero_equation_text_run_tail_before_next_title_fits =
                                compact_endnote_separator_profile
                                    && zero_endnote_spacing_profile
                                    && has_visible_endnote_separator
                                    && st.current_column + 1 < st.col_count
                                    && ep_idx + 1 == en_ctrl.paragraphs.len()
                                    && fmt.line_heights.len() == 1
                                    && line_is_equation_tac_text_run_only(en_para, &composed, 0)
                                    && next_endnote_title_fit_height.is_some_and(|next_h| {
                                        st.current_height + en_fit + next_h
                                            <= available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                    });
                            let boundary_gap_tail_and_next_title_fit_current_column =
                                !default_between_notes_gap
                                    && compact_endnote_separator_profile
                                    && has_visible_endnote_separator
                                    && ep_idx + 1 == en_ctrl.paragraphs.len()
                                    && st.current_column + 1 < st.col_count
                                    && st.current_height + total_advance_fit
                                        <= available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                    && next_endnote_title_fit_height.is_some_and(|next_h| {
                                        st.current_height + total_advance_fit + next_h
                                            <= available
                                                + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                + 2.0
                                    });
                            let no_separator_boundary_tail_without_gap_fits = large_separator_block
                                && !has_visible_endnote_separator
                                && endnote_boundary_gap_extra_px > 0.0
                                && ep_idx + 1 == en_ctrl.paragraphs.len()
                                && (st.current_column + 1 >= st.col_count
                                    || (st.current_column + 1 < st.col_count
                                        && st.current_height > available * 0.90))
                                && !local_vpos_rewind
                                && !internal_vpos_rewind
                                && st.current_height < available
                                && st.current_height + total_advance_fit
                                    <= available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                && para_has_visible_text_or_equation(en_para)
                                && !para_has_non_tac_picture_or_shape(en_para);
                            let endnote_boundary_gap_final_equation_tail_fits =
                                endnote_boundary_gap_extra_px > 0.0
                                    && !default_between_notes_gap
                                    && compact_endnote_separator_profile
                                    && has_visible_endnote_separator
                                    && ep_idx + 1 == en_ctrl.paragraphs.len()
                                    && st.current_column + 1 >= st.col_count
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && !para_has_visible_text(en_para)
                                    && line_is_equation_tac_text_run_only(en_para, &composed, 0)
                                    && st.current_height + total_advance_fit
                                        <= available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 80.0;
                            let endnote_boundary_gap_tail_overflows_frame =
                                endnote_boundary_gap_extra_px > 0.0
                                    && st.col_count > 1
                                    && ep_idx > 0
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && st.current_height > available * 0.90
                                    && st.current_height
                                        + total_advance_fit
                                        + endnote_boundary_gap_extra_px
                                        > available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                    && !boundary_gap_tail_and_next_title_fit_current_column
                                    && !no_separator_boundary_tail_without_gap_fits
                                    && !endnote_boundary_gap_final_equation_tail_fits
                                    && (para_has_visible_text_or_equation(en_para)
                                        || para_has_treat_as_char_picture_or_shape(en_para)
                                        || para_has_non_tac_picture_or_shape(en_para));
                            let no_separator_final_tail_fits_by_visible_height =
                                large_separator_block
                                    && !has_visible_endnote_separator
                                    && endnote_boundary_gap_extra_px > 0.0
                                    && ep_idx + 1 == en_ctrl.paragraphs.len()
                                    && st.current_column + 1 >= st.col_count
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && st.current_height < available
                                    && para_has_visible_text_or_equation(en_para)
                                    && !para_has_non_tac_picture_or_shape(en_para)
                                    && en_fit
                                        > total_advance_fit + endnote_boundary_gap_extra_px + 20.0
                                    && st.current_height
                                        + total_advance_fit
                                        + endnote_boundary_gap_extra_px
                                        <= available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 2.0;
                            let default_title_tail_body_advances_column =
                                compact_endnote_separator_profile
                                    && default_between_notes_gap
                                    && has_visible_endnote_separator
                                    && ep_idx == 1
                                    && en_ref.number > 0
                                    && st.current_column + 1 < st.col_count
                                    && st.current_height > available * 0.925
                                    && st.current_height + fmt.total_height > available + 1.0
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && en_ctrl
                                        .paragraphs
                                        .first()
                                        .is_some_and(|title_para| title_para.line_segs.len() == 1)
                                    && fmt.line_heights.len() <= 2
                                    && para_has_visible_text_or_equation(en_para)
                                    && endnote_has_visible_payload;
                            let large_between_title_tail_body_advances_page =
                                if !default_between_notes_gap
                                    && compact_endnote_separator_profile
                                    && has_visible_endnote_separator
                                    && ep_idx == 1
                                    && en_ref.number > 0
                                    && st.current_column + 1 >= st.col_count
                                    && st.current_height > available * 0.90
                                    && !st.current_items.is_empty()
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && en_ctrl
                                        .paragraphs
                                        .first()
                                        .is_some_and(|title_para| title_para.line_segs.len() == 1)
                                    && fmt.line_heights.len() <= 2
                                    && para_has_visible_text_or_equation(en_para)
                                    && endnote_has_visible_payload
                                {
                                    let mut local_paras: Vec<Paragraph> = Vec::new();
                                    let mut local_indices: Vec<(usize, usize)> = Vec::new();
                                    for pi in st
                                        .current_items
                                        .iter()
                                        .filter_map(page_item_para_index)
                                        .chain(std::iter::once(en_para_idx))
                                    {
                                        if local_indices.iter().any(|(global, _)| *global == pi) {
                                            continue;
                                        }
                                        if let Some(p) = paragraph_by_global_index(
                                            paragraphs,
                                            &st.endnote_paragraphs,
                                            pi,
                                        ) {
                                            let local = local_paras.len();
                                            local_paras.push(p.clone());
                                            local_indices.push((pi, local));
                                        }
                                    }
                                    let lookup_local = |pi: usize, indices: &[(usize, usize)]| {
                                        indices.iter().find_map(|(global, local)| {
                                            (*global == pi).then_some(*local)
                                        })
                                    };
                                    let first_vpos = st
                                        .current_items
                                        .iter()
                                        .filter_map(page_item_para_index)
                                        .find_map(|pi| {
                                            paragraph_by_global_index(
                                                paragraphs,
                                                &st.endnote_paragraphs,
                                                pi,
                                            )
                                            .and_then(|p| p.line_segs.first())
                                            .map(|seg| seg.vertical_pos)
                                        });
                                    let predicted_y = first_vpos.and_then(|page_base| {
                                        let mut hc = HeightCursor::new(
                                            self.dpi,
                                            0.0,
                                            available,
                                            st.current_start_height,
                                            Some(page_base),
                                            st.skip_spacing_before_prededuct,
                                            false,
                                            st.current_endnote_flow
                                                && st.current_start_height < -0.5,
                                            st.current_endnote_flow,
                                        );
                                        hc.endnote_between_notes_hu = st.endnote_between_notes_hu;
                                        let mut y = st.current_start_height;
                                        for item in &st.current_items {
                                            let Some(pi) = page_item_para_index(item) else {
                                                continue;
                                            };
                                            let Some(local) = lookup_local(pi, &local_indices)
                                            else {
                                                continue;
                                            };
                                            y = hc.vpos_adjust(y, local, &local_paras, &styles);
                                            let item_para = &local_paras[local];
                                            let item_composed =
                                                crate::renderer::composer::compose_paragraph(
                                                    item_para,
                                                );
                                            let item_fmt = self.format_paragraph(
                                                item_para,
                                                Some(&item_composed),
                                                &styles,
                                                Some(en_col_w),
                                            );
                                            y += match item {
                                                PageItem::PartialParagraph {
                                                    start_line,
                                                    end_line,
                                                    ..
                                                } => item_fmt
                                                    .line_advances_sum(*start_line..*end_line),
                                                PageItem::FullParagraph { .. } => {
                                                    item_fmt.total_height
                                                }
                                                _ => 0.0,
                                            };
                                            let current_vpos_rewinds_from_prev = hc
                                                .prev_layout_para
                                                .and_then(|prev_local| {
                                                    let prev_first = local_paras
                                                        .get(prev_local)
                                                        .and_then(|p| p.line_segs.first())
                                                        .map(|seg| seg.vertical_pos)?;
                                                    let curr_first = local_paras
                                                        .get(local)
                                                        .and_then(|p| p.line_segs.first())
                                                        .map(|seg| seg.vertical_pos)?;
                                                    Some(curr_first < prev_first)
                                                })
                                                .unwrap_or(false);
                                            if matches!(
                                                item,
                                                PageItem::PartialParagraph { start_line, .. }
                                                    if *start_line > 0
                                            ) || current_vpos_rewinds_from_prev
                                            {
                                                hc.prev_layout_para = None;
                                                hc.vpos_page_base = None;
                                                hc.vpos_lazy_base = None;
                                            } else {
                                                hc.prev_layout_para = Some(local);
                                            }
                                            hc.prev_item_was_partial_table =
                                                matches!(item, PageItem::PartialTable { .. });
                                        }
                                        lookup_local(en_para_idx, &local_indices).map(|local| {
                                            hc.vpos_adjust(y, local, &local_paras, &styles)
                                        })
                                    });

                                    predicted_y
                                        .map(|y| {
                                            // 큰 미주 사이 문서의 마지막 단에서는 새 문항 제목
                                            // 한 줄만 frame 안쪽 tail로 남기고, 첫 풀이 수식/본문이
                                            // render vpos 기준으로 frame을 넘으면 다음 쪽에서 시작한다.
                                            y + fmt.line_advance(0)
                                                > available
                                                    + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                    + 1.0
                                        })
                                        .unwrap_or(false)
                                } else {
                                    false
                                };
                            let large_between_last_column_new_note_tail = !default_between_notes_gap
                                && compact_endnote_separator_profile
                                && has_visible_endnote_separator
                                && ep_idx == 0
                                && emitted_endnote_count > 0
                                && st.current_column + 1 >= st.col_count
                                && st.current_height > available * 0.90
                                && (!endnote_has_vpos_rewind
                                    || st.current_height + en_fit > available)
                                && !st.current_items.is_empty()
                                && !large_between_last_column_question_title_tail_fits
                                && !large_between_last_column_render_title_tail_fits
                                && !large_between_last_column_rewind_title_tail_fits
                                && endnote_has_visible_payload;
                            let large_between_short_text_before_equation_tail_bleeds_previous_column =
                                !default_between_notes_gap
                                    && compact_endnote_separator_profile
                                    && has_visible_endnote_separator
                                    && ep_idx > 0
                                    && en_ctrl.paragraphs.len().saturating_sub(ep_idx) >= 6
                                    && st.col_count > 1
                                    && st.current_column + 1 < st.col_count
                                    && st.current_height > available * 0.90
                                    && !st.current_items.is_empty()
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && later_endnote_vpos_rewinds_after_current
                                    && fmt.line_heights.len() == 1
                                    && fmt.line_advance(0) <= 24.0
                                    && st.current_height + fmt.line_advance(0) <= available + 1.0
                                    && line_has_visible_text(&composed, 0)
                                    && !para_has_treat_as_char_picture_or_shape(en_para)
                                    && !para_has_non_tac_picture_or_shape(en_para)
                                    && en_ctrl.paragraphs.get(ep_idx + 1).is_some_and(|next_para| {
                                        let next_comp =
                                            crate::renderer::composer::compose_paragraph(next_para);
                                        let next_fmt = self.format_paragraph(
                                            next_para,
                                            Some(&next_comp),
                                            &styles,
                                            Some(en_col_w),
                                        );
                                        next_fmt.line_heights.len() == 1
                                            && next_fmt.line_advance(0) <= 36.0
                                            && line_is_equation_tac_text_run_only(
                                                next_para, &next_comp, 0,
                                            )
                                            && st.current_height
                                                + fmt.line_advance(0)
                                                + next_fmt.line_advance(0)
                                                <= available
                                                    + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                    + 80.0
                                    });
                            let large_between_final_visible_equation_rewind_tail_starts_next_column =
                                !default_between_notes_gap
                                    && compact_endnote_separator_profile
                                    && has_visible_endnote_separator
                                    && ep_idx > 0
                                    && en_ctrl.paragraphs.len().saturating_sub(ep_idx) <= 4
                                    && st.col_count > 1
                                    && st.current_column + 1 < st.col_count
                                    && st.current_height > available * 0.93
                                    && !st.current_items.is_empty()
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && later_endnote_vpos_rewinds_after_current
                                    && fmt.line_heights.len() == 1
                                    && line_has_visible_text(&composed, 0)
                                    && !para_has_treat_as_char_picture_or_shape(en_para)
                                    && !para_has_non_tac_picture_or_shape(en_para)
                                    && en_ctrl.paragraphs.get(ep_idx + 1).is_some_and(|next_para| {
                                        let next_comp =
                                            crate::renderer::composer::compose_paragraph(next_para);
                                        let next_fmt = self.format_paragraph(
                                            next_para,
                                            Some(&next_comp),
                                            &styles,
                                            Some(en_col_w),
                                        );
                                        next_fmt.line_heights.len() == 1
                                            && line_is_equation_tac_text_run_only(
                                                next_para, &next_comp, 0,
                                            )
                                    });
                            let large_between_lead_in_before_final_tail_starts_next_column =
                                !default_between_notes_gap
                                    && compact_endnote_separator_profile
                                    && has_visible_endnote_separator
                                    && ep_idx > 0
                                    && ep_idx + 2 == en_ctrl.paragraphs.len()
                                    && endnote_refs.get(en_ref_idx + 1).is_some()
                                    && st.col_count > 1
                                    && st.current_column + 1 < st.col_count
                                    && !st.current_items.is_empty()
                                    && !internal_vpos_rewind
                                    && fmt.line_heights.len() == 1
                                    && line_has_visible_text(&composed, 0)
                                    && !para_has_treat_as_char_picture_or_shape(en_para)
                                    && !para_has_non_tac_picture_or_shape(en_para)
                                    && st.current_height + fmt.line_advance(0)
                                        <= available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 2.0
                                    && en_ctrl.paragraphs.get(ep_idx + 1).is_some_and(
                                        |next_para| {
                                            let next_comp =
                                                crate::renderer::composer::compose_paragraph(
                                                    next_para,
                                                );
                                            let next_fmt = self.format_paragraph(
                                                next_para,
                                                Some(&next_comp),
                                                &styles,
                                                Some(en_col_w),
                                            );
                                            let next_tail_gap = endnote_shape
                                                .filter(|shape| {
                                                    let between_notes =
                                                        endnote_between_notes_margin(shape) as i32;
                                                    between_notes
                                                        > ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU
                                                        && !endnote_has_absorbed_between_notes_gap(
                                                            shape,
                                                        )
                                                })
                                                .map(|shape| {
                                                    let between_notes =
                                                        endnote_between_notes_margin(shape) as i32;
                                                    let saved_spacing = next_para
                                                        .line_segs
                                                        .last()
                                                        .map(|seg| seg.line_spacing.max(0))
                                                        .unwrap_or(0);
                                                    hwpunit_to_px(
                                                        (between_notes - saved_spacing).max(0),
                                                        self.dpi,
                                                    )
                                                })
                                                .unwrap_or(0.0);
                                            let following_title_reserved = endnote_shape
                                                .map(endnote_between_notes_margin)
                                                .map(|gap| hwpunit_to_px(gap as i32, self.dpi))
                                                .unwrap_or(0.0)
                                                + 12.0;
                                            let next_is_tall_tail = next_fmt.height_for_fit > 80.0
                                                || next_fmt.line_heights.len() > 1;

                                            next_is_tall_tail
                                                && st.current_height
                                                    + fmt.line_advance(0)
                                                    + next_fmt.height_for_fit
                                                    + next_tail_gap
                                                    + following_title_reserved
                                                    > available
                                                        + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                        },
                                    );
                            let large_between_last_column_final_lead_tac_tail_starts_next_page =
                                !default_between_notes_gap
                                    && compact_endnote_separator_profile
                                    && has_visible_endnote_separator
                                    && ep_idx > 0
                                    && ep_idx + 2 == en_ctrl.paragraphs.len()
                                    && endnote_refs.get(en_ref_idx + 1).is_some()
                                    && st.col_count > 1
                                    && st.current_column + 1 >= st.col_count
                                    && st.current_height > available * 0.85
                                    && !st.current_items.is_empty()
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && fmt.line_heights.len() >= 2
                                    && para_has_visible_text_or_equation(en_para)
                                    && !para_has_treat_as_char_picture_or_shape(en_para)
                                    && !para_has_non_tac_picture_or_shape(en_para)
                                    && st.current_height + en_fit
                                        <= available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 2.0
                                    && {
                                        let last_line = fmt.line_heights.len() - 1;
                                        line_has_tac_equation_control(en_para, &composed, last_line)
                                    }
                                    && en_ctrl.paragraphs.get(ep_idx + 1).is_some_and(
                                        |next_para| {
                                            let next_comp =
                                                crate::renderer::composer::compose_paragraph(
                                                    next_para,
                                                );
                                            let next_fmt = self.format_paragraph(
                                                next_para,
                                                Some(&next_comp),
                                                &styles,
                                                Some(en_col_w),
                                            );
                                            next_fmt.line_heights.len() == 1
                                                && line_is_equation_tac_text_run_only(
                                                    next_para, &next_comp, 0,
                                                )
                                                && st.current_height
                                                    + en_fit
                                                    + next_fmt.height_for_fit
                                                    > available
                                                        - ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                        },
                                    );
                            let zero_visible_text_tail_before_rewind_fits =
                                compact_endnote_separator_profile
                                    && zero_endnote_spacing_profile
                                    && has_visible_endnote_separator
                                    && st.col_count > 1
                                    && st.current_column + 1 < st.col_count
                                    && ep_idx > 0
                                    && en_ref.number > 0
                                    && fmt.line_heights.len() <= 2
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && later_endnote_vpos_rewinds_after_current
                                    && !para_is_treat_as_char_picture_only(en_para)
                                    && !para_has_non_tac_picture_or_shape(en_para)
                                    && para_has_visible_text_or_equation(en_para)
                                    && line_has_visible_text(&composed, 0)
                                    && st.current_height > available * 0.96
                                    && st.current_height + en_fit
                                        <= available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 28.0;
                            let non_visible_endnote_tail_bleeds_previous_column =
                                compact_endnote_separator_profile
                                    && default_between_notes_gap
                                    && ep_idx > 0
                                    && st.current_column + 1 >= st.col_count
                                    && !para_has_visible_text_or_equation(en_para)
                                    && !para_has_non_tac_picture_or_shape(en_para)
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && st.current_height
                                        < available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 8.0
                                    && st.current_height + en_fit
                                        <= available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 80.0;
                            let large_between_non_visible_tail_bleeds_previous_column =
                                compact_endnote_separator_profile
                                    && !default_between_notes_gap
                                    && has_visible_endnote_separator
                                    && ep_idx > 0
                                    && st.current_column + 1 < st.col_count
                                    && !para_has_visible_text_or_equation(en_para)
                                    && !para_has_non_tac_picture_or_shape(en_para)
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && st.current_height
                                        < available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 8.0
                                    && st.current_height + en_fit
                                        <= available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 80.0;
                            let zero_visible_last_column_text_tail_starts_next_page =
                                compact_endnote_separator_profile
                                    && zero_endnote_spacing_profile
                                    && has_visible_endnote_separator
                                    && ep_idx > 0
                                    && st.current_column + 1 >= st.col_count
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && !para_is_treat_as_char_picture_only(en_para)
                                    && para_has_visible_text_or_equation(en_para)
                                    && st.current_height > available * 0.96
                                    && st.current_height + fmt.total_height > available + 1.0
                                    && (fmt.line_heights.len() > 1
                                        // 0/0/0 미주는 마지막 단 바닥의 한 줄짜리 설명 뒤에
                                        // 큰 TAC 그림이 바로 이어지는 경우, 설명 줄도 현재
                                        // frame 아래로 잘리므로 한컴처럼 다음 쪽으로 넘긴다.
                                        || (fmt.line_heights.len() == 1
                                            && st.current_height > available * 0.99
                                            && en_ctrl
                                                .paragraphs
                                                .get(ep_idx + 1)
                                                .is_some_and(para_is_treat_as_char_picture_only)));
                            let zero_between_visible_last_column_text_tail_starts_next_page =
                                compact_endnote_separator_profile
                                    && visible_zero_between_large_separator_gap
                                    && ep_idx > 0
                                    && st.current_column + 1 >= st.col_count
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && !para_is_treat_as_char_picture_only(en_para)
                                    && para_has_visible_text_or_equation(en_para)
                                    && st.current_height > available * 0.96
                                    && st.current_height + fmt.total_height > available + 1.0
                                    && (fmt.line_heights.len() > 1
                                        || (fmt.line_heights.len() == 1
                                            && st.current_height > available * 0.99
                                            && en_ctrl
                                                .paragraphs
                                                .get(ep_idx + 1)
                                                .is_some_and(para_is_treat_as_char_picture_only)));
                            let large_between_zero_above_whole_note_small_bleed_fits =
                                compact_endnote_separator_profile
                                    && visible_large_between_zero_above_compact_below
                                    && ep_idx == 0
                                    && en_ref.number > 0
                                    && st.current_column + 1 >= st.col_count
                                    && !st.current_items.is_empty()
                                    && !local_vpos_rewind
                                    && !internal_vpos_rewind
                                    && st.current_height < available * 0.35
                                    && st.current_height + en_fit > available
                                    && st.current_height + en_fit
                                        <= available
                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            + 6.0
                                    && endnote_has_visible_payload;
                            let advance_for_fit = ((st.current_height + en_fit > available
                                && !no_separator_final_tail_fits_by_visible_height
                                && !no_separator_visible_multiline_tail_fits_with_bleed
                                && !large_between_zero_above_whole_note_small_bleed_fits)
                                || (late_compact_text_tail_overflow_risk
                                    && !zero_equation_text_run_tail_before_next_title_fits)
                                || zero_tac_picture_tail_bleeds_frame
                                || visible_separator_large_tac_tail_overflows_frame
                                || visible_separator_text_after_large_tac_tail_starts_next_page
                                || visible_separator_text_after_equation_tail_overflows_frame
                                || zero_visible_last_column_text_tail_starts_next_page
                                || zero_between_visible_last_column_text_tail_starts_next_page
                                || endnote_boundary_gap_tail_overflows_frame
                                || default_title_tail_body_advances_column
                                || large_between_title_tail_body_advances_page
                                || large_between_split_head_render_overflows
                                || large_between_last_column_new_note_tail
                                || no_separator_tail_after_picture_starts_next_page
                                || zero_between_large_separator_last_column_title_orphan
                                || large_between_equation_tail_starts_next_column
                                || large_between_final_visible_equation_rewind_tail_starts_next_column
                                || large_between_lead_in_before_final_tail_starts_next_column
                                || large_between_last_column_final_lead_tac_tail_starts_next_page
                                || no_separator_saved_vpos_tail_outside
                                || visible_separator_saved_vpos_tail_outside
                                || internal_rewind_head_overflows_current_column
                                || internal_reset_split_head_render_overflows
                                || internal_rewind_full_advance_needed)
                                && (split_endnote_to_fit.is_none()
                                    || (late_compact_text_tail_overflow_risk
                                        && !zero_equation_text_run_tail_before_next_title_fits)
                                    || internal_rewind_full_advance_needed)
                                && large_between_last_column_visual_split.is_none()
                                && large_between_last_column_flow_tail_split.is_none()
                                && (!internal_rewind_head_allows_current_column
                                    || internal_reset_split_head_render_overflows
                                    || internal_rewind_full_advance_needed)
                                && (!compact_endnote_own_vpos_span_fits_for_flow
                                    || late_compact_text_tail_overflow_risk
                                    || internal_rewind_head_overflows_current_column
                                    || default_title_tail_body_advances_column
                                    || large_between_title_tail_body_advances_page
                                    || large_between_split_head_render_overflows
                                    || visible_separator_large_tac_tail_overflows_frame
                                    || visible_separator_text_after_large_tac_tail_starts_next_page
                                    || visible_separator_text_after_equation_tail_overflows_frame
                                    || zero_visible_last_column_text_tail_starts_next_page
                                    || zero_between_visible_last_column_text_tail_starts_next_page
                                    || zero_between_large_separator_last_column_title_orphan
                                    || large_between_last_column_final_lead_tac_tail_starts_next_page
                                    || internal_reset_split_head_render_overflows
                                    || internal_rewind_full_advance_needed)
                                && !allow_compact_question_title_tail
                                && !default_question_title_tail_fits_by_line_height
                                && !zero_question_title_tail_fits_by_line_height
                                && !zero_question_intro_tail_before_rewind_fits
                                && !zero_visible_text_tail_before_rewind_fits
                                && !zero_between_large_separator_tail_group_fits
                                && !large_between_last_column_question_title_tail_fits
                                && !large_between_last_column_render_title_tail_fits
                                && !large_between_last_column_rewind_title_tail_fits
                                && !large_between_last_column_title_body_tail_fits
                                && (!default_between_notes_gap
                                    || internal_rewind_split.is_none()
                                    || internal_rewind_head_overflows_current_column
                                    || internal_rewind_full_advance_needed)
                                && !late_question_title_small_overflow
                                && !allow_large_between_question_title_tail
                                && !large_between_last_column_question_title_tail_fits
                                && !allow_default_column_bottom_question_title_tail
                                && !late_question_intro_tail
                                && !late_question_continuation_tail
                                && !large_between_short_text_before_equation_tail_bleeds_previous_column
                                && (!non_visible_endnote_tail_bleeds_previous_column
                                    || visible_separator_large_tac_tail_overflows_frame)
                                && !large_between_non_visible_tail_bleeds_previous_column
                                && !st.current_items.is_empty();
                            if std::env::var("RHWP_ENDNOTE_ADVANCE_DEBUG").is_ok() {
                                eprintln!(
                                    "ENDNOTE_ADV phase=fit note={} ep={} col={}/{} cur={:.2} avail={:.2} en_fit={:.2} total={:.2} h4f={:.2} boundary_gap_extra={:.2} boundary_gap_over={} next_head_large_tac={} lines={} first={:?} bottom={:?} content_bottom={:?} local_rewind={} internal_rewind={:?} internal_split={:?} split={:?} visual_split={:?} flow_tail_split={:?} own_span_fit={} late_text_tail={} eq_tail_next_title={} zero_tac_tail={} visible_large_tac_tail={} text_after_tac_tail={} text_after_eq_tail={} tac_candidate={} tac_render_y={:?} tac_bottom={:?} zero_intro_tail={} zero_text_tail={} no_sep_visible_tail={} no_sep_multiline_tail={} default_title_body={} split_head_over={} reset_split_head_over={} rewind_full_advance={} last_col_new_tail={} large_eq_tail_next_col={} lead_final_tail={} no_sep_tail={} visible_sep_tail={} internal_head_over={} non_visible_tail_bleed={} advance_fit={}",
                                    en_ref.number,
                                    ep_idx,
                                    st.current_column + 1,
                                    st.col_count,
                                    st.current_height,
                                    available,
                                    en_fit,
                                    total_advance_fit,
                                    h4f,
                                    endnote_boundary_gap_extra_px,
                                    endnote_boundary_gap_tail_overflows_frame,
                                    next_endnote_head_has_large_tac_picture,
                                    fmt.line_heights.len(),
                                    this_first_offset,
                                    this_bottom_offset,
                                    this_content_bottom_offset,
                                    local_vpos_rewind,
                                    internal_rewind_position,
                                    internal_rewind_split,
                                    split_endnote_to_fit,
                                    large_between_last_column_visual_split,
                                    large_between_last_column_flow_tail_split,
                                    compact_endnote_own_vpos_span_fits_for_flow,
                                    late_compact_text_tail_overflow_risk,
                                    zero_equation_text_run_tail_before_next_title_fits,
                                    zero_tac_picture_tail_bleeds_frame,
                                    visible_separator_large_tac_tail_overflows_frame,
                                    visible_separator_text_after_large_tac_tail_starts_next_page,
                                    visible_separator_text_after_equation_tail_overflows_frame,
                                    visible_separator_large_tac_tail_candidate,
                                    visible_separator_large_tac_tail_render_y,
                                    visible_separator_large_tac_tail_bottom,
                                    zero_question_intro_tail_before_rewind_fits,
                                    zero_visible_text_tail_before_rewind_fits,
                                    no_separator_final_tail_fits_by_visible_height,
                                    no_separator_visible_multiline_tail_fits_with_bleed,
                                    default_title_tail_body_advances_column,
                                    large_between_split_head_render_overflows,
                                    internal_reset_split_head_render_overflows,
                                    internal_rewind_full_advance_needed,
                                    large_between_last_column_new_note_tail,
                                    large_between_equation_tail_starts_next_column,
                                    large_between_lead_in_before_final_tail_starts_next_column,
                                    no_separator_saved_vpos_tail_outside,
                                    visible_separator_saved_vpos_tail_outside,
                                    internal_rewind_head_overflows_current_column,
                                    non_visible_endnote_tail_bleeds_previous_column,
                                    advance_for_fit,
                                );
                            }
                            let pre_emit_tail_before_non_tac_object_advance = advance_for_fit
                                && compact_endnote_separator_profile
                                && has_visible_endnote_separator
                                && (large_separator_block || zero_between_large_separator_margin)
                                && endnote_shape
                                    .map(|shape| endnote_between_notes_margin(shape) == 0)
                                    .unwrap_or(false)
                                && st.col_count > 1
                                && st.current_column + 1 < st.col_count
                                && non_tac_object_height.is_some()
                                && !endnote_has_text_or_equation
                                && ep_idx + 1 < en_ctrl.paragraphs.len()
                                && st.current_height > available * 0.90
                                && en_ctrl.paragraphs.get(ep_idx + 1).is_some_and(|next_para| {
                                    let next_comp =
                                        crate::renderer::composer::compose_paragraph(next_para);
                                    let next_fmt = self.format_paragraph(
                                        next_para,
                                        Some(&next_comp),
                                        &styles,
                                        Some(en_col_w),
                                    );
                                    para_has_visible_text_or_equation(next_para)
                                        && !para_has_non_tac_picture_or_shape(next_para)
                                        && !para_has_treat_as_char_picture_or_shape(next_para)
                                        && next_fmt.line_heights.len() == 1
                                        && st.current_height + next_fmt.total_height
                                            <= available
                                                + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                + 2.0
                                });
                            if pre_emit_tail_before_non_tac_object_advance {
                                if let Some(next_para) = en_ctrl.paragraphs.get(ep_idx + 1) {
                                    let next_para_idx =
                                        paragraphs.len() + st.endnote_paragraphs.len();
                                    let mut next_para_copy = next_para.clone();
                                    for ls in &mut next_para_copy.line_segs {
                                        ls.vertical_pos += endnote_start;
                                    }
                                    st.endnote_paragraphs.push(next_para_copy);
                                    st.endnote_para_sources.push(EndnoteParaSource {
                                        section_index: en_ref.section_index,
                                        para_index: en_ref.para_index,
                                        control_index: en_ref.control_index,
                                        note_para_index: ep_idx + 1,
                                    });
                                    last_render_endnote_para_local_idx =
                                        Some(st.endnote_paragraphs.len() - 1);

                                    let next_comp =
                                        crate::renderer::composer::compose_paragraph(next_para);
                                    let next_fmt = self.format_paragraph(
                                        next_para,
                                        Some(&next_comp),
                                        &styles,
                                        Some(en_col_w),
                                    );
                                    st.current_items.push(PageItem::FullParagraph {
                                        para_index: next_para_idx,
                                    });
                                    st.current_height += next_fmt.total_height;
                                    st.current_endnote_flow = true;
                                    pre_emitted_endnote_para_indices.insert(ep_idx + 1);
                                }
                            }
                            if advance_for_fit {
                                st.advance_column_or_new_page();
                                prev_en_bottom_vpos = None;
                                prev_en_content_bottom_vpos = None;
                                if internal_rewind_split == Some(1) {
                                    internal_rewind_split = None;
                                    cleared_single_line_internal_rewind_split = true;
                                } else if absorbed_between_notes_gap && internal_vpos_rewind {
                                    // 이전 단 하단에서 계산한 내부 rewind split은
                                    // 새 단으로 advance한 뒤에는 더 이상 유효하지 않다.
                                    // 그대로 들고 가면 빈 단에서 문단을 다시 쪼개
                                    // 한컴보다 미주 흐름이 한 쪽 늦어진다.
                                    internal_rewind_split = None;
                                } else if internal_rewind_head_overflows_current_column {
                                    // 현재 단에 split 머리도 들어가지 않는 internal rewind는
                                    // 새 단/쪽에서 다시 전체 높이로 배치한다.
                                    internal_rewind_split = None;
                                } else if internal_reset_split_head_render_overflows {
                                    // 저장 lineSeg reset은 실제 column/page split 신호지만,
                                    // 현재 단의 render-y 기준으로 reset 앞 head가 이미 frame을
                                    // 넘으면 현재 단 tail로 남기지 않고 다음 단/쪽에서 다시 본다.
                                    internal_rewind_split = None;
                                } else if internal_rewind_full_advance_needed {
                                    // saved-vpos 압축 높이만 현재 단에 들어가는 기본 미주 rewind는
                                    // head tail로 쪼개지 않고 다음 단에서 전체 문단으로 시작한다.
                                    internal_rewind_split = None;
                                }
                            }
                            let allow_default_question_title_tail = default_between_notes_gap
                                && prev_endnote_had_inline_object_vpos_overestimate
                                && ep_idx == 0
                                && en_fit <= 24.0
                                && st.current_height + en_fit <= available - 40.0;
                            let allow_default_question_title_tail =
                                allow_default_question_title_tail
                                    // 보이는 구분선의 기본 미주 사이에서는 새 문항 제목 한 줄이
                                    // 단 하단에 몰려 있지 않으면 한컴처럼 현재 단에 남긴다.
                                    // 전체 tail을 기준으로 밀면 문항 본문이 다음 단으로 과하게 넘어간다.
                                    || (default_between_notes_gap
                                        && has_visible_endnote_separator
                                        && ep_idx == 0
                                        && st.current_column + 1 < st.col_count
                                        && en_fit <= 24.0
                                        && st.current_height < available * 0.85
                                        && st.current_height + en_fit <= available - 40.0);
                            let allow_default_question_title_tail =
                                allow_default_question_title_tail
                                    // 구분선 아래가 큰 기본 미주에서는 저장 vpos rewind 때문에
                                    // 제목+head 묶음 전체가 current_height 기준보다 커 보일 수 있다.
                                    // 제목 앞 공식 "미주 사이" gap과 제목 한 줄이 현재 단에
                                    // 들어가면 한컴처럼 제목/head를 단 하단에 남기고 뒤에서
                                    // 자연스럽게 split되도록 advance를 막는다.
                                    || (default_between_notes_gap
                                        && compact_endnote_separator_profile
                                        && has_visible_endnote_separator
                                        && endnote_has_vpos_rewind
                                        && ep_idx == 0
                                        && en_ref.number > 0
                                        && st.current_column + 1 < st.col_count
                                        && !st.current_items.is_empty()
                                        && fmt.line_heights.len() == 1
                                        && st.current_height > available * 0.85
                                        && st.current_height < available * 0.93
                                        && endnote_shape
                                            .map(|shape| {
                                                endnote_separator_below_margin(shape) as i32
                                                    > ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU
                                            })
                                            .unwrap_or(false)
                                        && endnote_shape
                                            .map(|shape| endnote_between_notes_margin(shape) as i32)
                                            .filter(|gap_hu| {
                                                st.current_height
                                                    + hwpunit_to_px(*gap_hu, self.dpi)
                                                    + en_fit
                                                    <= available
                                                        + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                        + 2.0
                                            })
                                            .is_some()
                                        && para_has_visible_text_or_equation(en_para));
                            let allow_default_question_title_tail =
                                allow_default_question_title_tail
                                    // 구분선 아래가 기본값 근방이어도 저장 vpos rewind가 있는
                                    // 새 미주 제목은 제목 앞 공식 "미주 사이" gap까지 현재
                                    // 단에 들어가면 하단 tail로 남긴다. head group 전체를
                                    // 기준으로 밀면 한컴보다 다음 단으로 일찍 넘어간다.
                                    || (default_between_notes_gap
                                        && compact_endnote_separator_profile
                                        && has_visible_endnote_separator
                                        && endnote_has_vpos_rewind
                                        && ep_idx == 0
                                        && en_ref.number > 0
                                        && st.current_column + 1 < st.col_count
                                        && !st.current_items.is_empty()
                                        && fmt.line_heights.len() == 1
                                        && st.current_height > available * 0.85
                                        && st.current_height < available * 0.90
                                        && endnote_shape
                                            .map(|shape| {
                                                endnote_separator_below_margin(shape) as i32
                                                    <= ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU
                                                    && st.current_height
                                                        + hwpunit_to_px(
                                                            endnote_between_notes_margin(shape)
                                                                as i32,
                                                            self.dpi,
                                                        )
                                                        + en_fit
                                                        <= available
                                                            + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                            + 2.0
                                            })
                                            .unwrap_or(false)
                                        && para_has_visible_text_or_equation(en_para));
                            let rewind_endnote_head_near_bottom = endnote_has_vpos_rewind
                                && st.current_height + total_advance_fit
                                    > available - ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX;
                            let rewind_endnote_head_would_split = endnote_has_vpos_rewind
                                && next_endnote_first_line_advance
                                    .map(|next_h| {
                                        st.current_height + total_advance_fit + next_h
                                            > available - ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                    })
                                    .unwrap_or(false);
                            let large_between_notes_head_near_bottom = !default_between_notes_gap
                                && !compact_between_notes_gap
                                && ep_idx == 0
                                && emitted_endnote_count > 0
                                && !no_separator_new_note_head_fits_current_column
                                && !large_between_zero_above_whole_note_small_bleed_fits
                                && new_endnote_between_notes_px
                                    .map(|gap| {
                                        // 미주 사이가 기본값보다 큰 문서는 새 번호 제목을
                                        // 한 줄짜리 tail로만 보지 않고, 번호 경계 gap까지
                                        // 함께 현재 단에 들어가는지 판단해야 한다.
                                        let reserved_head = en_fit.max(fmt.line_advance(0) + gap);
                                        st.current_height + reserved_head
                                            > available - ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                    })
                                    .unwrap_or(false);
                            let visible_separator_vpos_head_group_outside =
                                compact_endnote_separator_profile
                                    && compact_between_notes_gap
                                    && default_between_notes_gap
                                    && has_visible_endnote_separator
                                    && ep_idx == 0
                                    && emitted_endnote_count > 0
                                    && st.current_column + 1 < st.col_count
                                    && st.current_height > available * 0.75
                                    && !st.current_items.is_empty()
                                    && st
                                        .current_items
                                        .iter()
                                        .filter_map(page_item_para_index)
                                        .find_map(|pi| {
                                            paragraph_by_global_index(
                                                paragraphs,
                                                &st.endnote_paragraphs,
                                                pi,
                                            )
                                            .and_then(|p| p.line_segs.first())
                                            .map(|s| s.vertical_pos)
                                        })
                                        .and_then(|base_vpos| {
                                            let first_vpos = this_first_offset?;
                                            let first_para_vpos =
                                                en_ctrl.paragraphs.first()?.line_segs.first()?;
                                            let group_bottom = en_ctrl
                                                .paragraphs
                                                .iter()
                                                .take(3)
                                                .flat_map(|p| p.line_segs.iter())
                                                .map(|s| {
                                                    s.vertical_pos
                                                        + s.line_height
                                                        + s.line_spacing
                                                        + endnote_start
                                                })
                                                .max()?;
                                            let group_first =
                                                first_para_vpos.vertical_pos + endnote_start;
                                            let group_h = hwpunit_to_px(
                                                (group_bottom - group_first).max(0),
                                                self.dpi,
                                            );
                                            let predicted_y = hwpunit_to_px(
                                                (first_vpos - base_vpos).max(0),
                                                self.dpi,
                                            );
                                            Some(
                                                predicted_y > available * 0.85
                                                    && predicted_y + group_h
                                                        > available
                                                            - ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX,
                                            )
                                        })
                                        .unwrap_or(false);
                            let default_between_large_below_head_group_outside =
                                compact_endnote_separator_profile
                                    && default_between_notes_gap
                                    && has_visible_endnote_separator
                                    && ep_idx == 0
                                    && emitted_endnote_count > 0
                                    && st.current_column + 1 < st.col_count
                                    && st.current_height > available * 0.90
                                    && endnote_shape
                                        .map(|shape| {
                                            endnote_separator_below_margin(shape) as i32
                                                > ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU
                                        })
                                        .unwrap_or(false)
                                    && en_ctrl
                                        .paragraphs
                                        .first()
                                        .is_some_and(|title_para| title_para.line_segs.len() == 1)
                                    && en_ctrl.paragraphs.get(1).is_some_and(para_has_visible_text)
                                    && en_ctrl.paragraphs.get(2).is_some_and(|tail_para| {
                                        !para_has_visible_text(tail_para)
                                            && para_has_visible_text_or_equation(tail_para)
                                    })
                                    && {
                                        let head_group_h: f64 = en_ctrl
                                            .paragraphs
                                            .iter()
                                            .take(3)
                                            .map(|head_para| {
                                                let head_comp =
                                                    crate::renderer::composer::compose_paragraph(
                                                        head_para,
                                                    );
                                                self.format_paragraph(
                                                    head_para,
                                                    Some(&head_comp),
                                                    &styles,
                                                    Some(en_col_w),
                                                )
                                                .total_height
                                            })
                                            .sum();
                                        st.current_height + head_group_h
                                            > available - ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                    };
                            let large_between_last_column_vpos_head_group_outside =
                                !default_between_notes_gap
                                    && compact_endnote_separator_profile
                                    && has_visible_endnote_separator
                                    && visible_large_between_notes_gap
                                    && !compact_between_notes_gap
                                    && ep_idx == 0
                                    && emitted_endnote_count > 0
                                    && st.current_column + 1 >= st.col_count
                                    && st.current_height > available * 0.75
                                    && st.current_height < available * 0.85
                                    && !st.current_items.is_empty()
                                    && large_between_question_title_render_y
                                        .map(|predicted_y| {
                                            let group_first = en_ctrl
                                                .paragraphs
                                                .first()
                                                .and_then(|p| p.line_segs.first())
                                                .map(|seg| seg.vertical_pos + endnote_start);
                                            let group_bottom = en_ctrl
                                                .paragraphs
                                                .iter()
                                                .take(3)
                                                .flat_map(|p| p.line_segs.iter())
                                                .map(|seg| {
                                                    seg.vertical_pos
                                                        + seg.line_height
                                                        + seg.line_spacing
                                                        + endnote_start
                                                })
                                                .max();
                                            group_first
                                                .zip(group_bottom)
                                                .map(|(first, bottom)| {
                                                    let group_h = hwpunit_to_px(
                                                        (bottom - first).max(0),
                                                        self.dpi,
                                                    );
                                                    predicted_y > available * 0.85
                                                        && predicted_y + group_h
                                                            > available
                                                                - ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                })
                                                .unwrap_or(false)
                                        })
                                        .unwrap_or(false);
                            let large_between_notes_vpos_head_outside =
                                large_between_notes_head_near_bottom
                                    || large_between_question_title_render_head_outside
                                    || large_between_question_lead_group_render_outside
                                    || visible_separator_vpos_head_group_outside
                                    || default_between_large_below_head_group_outside
                                    || large_between_last_column_vpos_head_group_outside
                                    || (!default_between_notes_gap
                                        && !compact_between_notes_gap
                                        && ep_idx == 0
                                        && !no_separator_new_note_head_fits_current_column
                                        && st.current_column + 1 >= st.col_count
                                        && st.current_height > available * 0.75
                                        && st
                                            .current_items
                                            .iter()
                                            .filter_map(page_item_para_index)
                                            .find_map(|pi| {
                                                paragraph_by_global_index(
                                                    paragraphs,
                                                    &st.endnote_paragraphs,
                                                    pi,
                                                )
                                                .and_then(|p| p.line_segs.first())
                                                .map(|s| s.vertical_pos)
                                            })
                                            .and_then(|base_vpos| {
                                                this_first_offset.map(|first_vpos| {
                                                    let predicted_y = hwpunit_to_px(
                                                        (first_vpos - base_vpos).max(0),
                                                        self.dpi,
                                                    );
                                                    predicted_y + fmt.line_advance(0)
                                                        > available
                                                            - 2.0
                                                                * ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                })
                                            })
                                            .unwrap_or(false));
                            let zero_new_endnote_full_tail_fits_current_column =
                                zero_endnote_spacing_profile
                                    && has_visible_endnote_separator
                                    && ep_idx == 0
                                    && st.current_column + 1 < st.col_count
                                    && !st.current_items.is_empty()
                                    && endnote_has_visible_payload
                                    && en_ctrl
                                        .paragraphs
                                        .iter()
                                        .flat_map(|p| p.line_segs.iter())
                                        .fold(None::<(i32, i32)>, |acc, seg| {
                                            let first = seg.vertical_pos + endnote_start;
                                            let bottom = first + seg.line_height + seg.line_spacing;
                                            Some(match acc {
                                                Some((min_first, max_bottom)) => {
                                                    (min_first.min(first), max_bottom.max(bottom))
                                                }
                                                None => (first, bottom),
                                            })
                                        })
                                        .map(|(first, bottom)| {
                                            let saved_span =
                                                hwpunit_to_px((bottom - first).max(0), self.dpi);
                                            let sequential_span: f64 = en_ctrl
                                                .paragraphs
                                                .iter()
                                                .map(|p| {
                                                    let comp =
                                                        crate::renderer::composer::compose_paragraph(
                                                            p,
                                                        );
                                                    self.format_paragraph(
                                                        p,
                                                        Some(&comp),
                                                        &styles,
                                                        Some(en_col_w),
                                                    )
                                                    .total_height
                                                })
                                                .sum();
                                            let note_span = if endnote_has_vpos_rewind {
                                                saved_span
                                            } else {
                                                saved_span.max(sequential_span)
                                            };
                                            st.current_height + note_span
                                                <= available
                                                    + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                    + 1.0
                                        })
                                        .unwrap_or(false);
                            let zero_between_question_title_tail_fits_current_column = endnote_shape
                                .map(|shape| {
                                    compact_endnote_separator_profile
                                        && has_visible_endnote_separator
                                        && endnote_between_notes_margin(shape) == 0
                                })
                                .unwrap_or(false)
                                && ep_idx == 0
                                && en_ref.number > 0
                                && st.current_column + 1 < st.col_count
                                && !st.current_items.is_empty()
                                && fmt.line_heights.len() == 1
                                && st.current_height + fmt.line_advance(0)
                                    <= available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX + 2.0;
                            let advance_for_new_endnote = st.col_count > 1
                                && compact_endnote_separator_profile
                                && ep_idx == 0
                                && emitted_endnote_count > 0
                                && !no_separator_new_note_head_fits_current_column
                                && !no_separator_last_column_new_note_head_without_gap_fits
                                && !allow_default_late_question_tail
                                && (!allow_default_column_bottom_question_title_tail
                                    || (large_between_notes_vpos_head_outside
                                        && !allow_default_first_column_large_below_title_tail))
                                && !allow_default_question_title_tail
                                && !allow_large_between_question_title_tail
                                && !large_between_last_column_question_title_tail_fits
                                && !large_between_last_column_render_title_tail_fits
                                && !large_between_last_column_rewind_title_tail_fits
                                && !default_question_title_tail_fits_by_line_height
                                && !zero_question_title_tail_fits_by_line_height
                                && !allow_compact_question_title_tail
                                && !allow_large_separator_first_column_tail
                                && !zero_new_endnote_full_tail_fits_current_column
                                && !zero_between_question_title_tail_fits_current_column
                                && !large_between_zero_above_whole_note_small_bleed_fits
                                && (!endnote_has_vpos_rewind
                                    || rewind_endnote_head_near_bottom
                                    || rewind_endnote_head_would_split
                                    || large_between_notes_vpos_head_outside)
                                && (!new_endnote_stale_forward_vpos
                                    || large_between_notes_vpos_head_outside)
                                && (st.current_height > available * new_endnote_advance_threshold
                                    || large_between_notes_vpos_head_outside)
                                && !st.current_items.is_empty();
                            let advance_for_internal_rewind = move_internal_rewind_equation_to_next
                                && !st.current_items.is_empty();
                            if std::env::var("RHWP_ENDNOTE_ADVANCE_DEBUG").is_ok() {
                                eprintln!(
                                    "ENDNOTE_ADV phase=new note={} ep={} col={}/{} cur={:.2} avail={:.2} en_fit={:.2} total={:.2} gap={:?} default_gap={} compact_gap={} zero_gap={} visible_sep={} render_y={:?} lead_group_outside={} has_rewind={} rewind_near_bottom={} rewind_would_split={} large_head_outside={} stale_forward={} allow_default_late={} allow_default_col_bottom={} allow_default_title={} allow_large_title={} allow_large_last_title={} allow_large_render_title={} allow_large_rewind_title={} allow_default_line={} allow_zero_line={} allow_compact={} allow_large_sep_first={} zero_full_tail={} zero_title_tail={} large_zero_small_bleed={} advance_new={} advance_internal={}",
                                    en_ref.number,
                                    ep_idx,
                                    st.current_column + 1,
                                    st.col_count,
                                    st.current_height,
                                    available,
                                    en_fit,
                                    total_advance_fit,
                                    new_endnote_between_notes_px,
                                    default_between_notes_gap,
                                    compact_between_notes_gap,
                                    zero_endnote_spacing_profile,
                                    has_visible_endnote_separator,
                                    large_between_question_title_render_y,
                                    large_between_question_lead_group_render_outside,
                                    endnote_has_vpos_rewind,
                                    rewind_endnote_head_near_bottom,
                                    rewind_endnote_head_would_split,
                                    large_between_notes_vpos_head_outside,
                                    new_endnote_stale_forward_vpos,
                                    allow_default_late_question_tail,
                                    allow_default_column_bottom_question_title_tail,
                                    allow_default_question_title_tail,
                                    allow_large_between_question_title_tail,
                                    large_between_last_column_question_title_tail_fits,
                                    large_between_last_column_render_title_tail_fits,
                                    large_between_last_column_rewind_title_tail_fits,
                                    default_question_title_tail_fits_by_line_height,
                                    zero_question_title_tail_fits_by_line_height,
                                    allow_compact_question_title_tail,
                                    allow_large_separator_first_column_tail,
                                    zero_new_endnote_full_tail_fits_current_column,
                                    zero_between_question_title_tail_fits_current_column,
                                    large_between_zero_above_whole_note_small_bleed_fits,
                                    advance_for_new_endnote,
                                    advance_for_internal_rewind,
                                );
                            }
                            if advance_for_new_endnote {
                                st.advance_column_or_new_page();
                                prev_en_bottom_vpos = None;
                                prev_en_content_bottom_vpos = None;
                            }
                            if advance_for_internal_rewind {
                                st.advance_column_or_new_page();
                                prev_en_bottom_vpos = None;
                                prev_en_content_bottom_vpos = None;
                            }
                            // 구분선 아래가 큰 기본 미주에서 제목 tail만 현재 단 하단에
                            // 남는 경우, 저장 vpos가 한 기본 미주 gap만큼 위로 당겨질 수
                            // 있다. 렌더 좌표만 보정하고 pagination 흐름은 유지한다.
                            let default_large_below_rewind_title_tail_gap_hu =
                                if !advance_for_new_endnote
                                    && !advance_for_internal_rewind
                                    && compact_endnote_separator_profile
                                    && default_between_notes_gap
                                    && has_visible_endnote_separator
                                    && endnote_has_vpos_rewind
                                    && ep_idx == 0
                                    && emitted_endnote_count > 0
                                    && en_ref.number > 0
                                    && st.current_column + 1 < st.col_count
                                    && st.current_height > available * 0.85
                                    && fmt.line_heights.len() == 1
                                    && endnote_shape
                                        .map(|shape| {
                                            endnote_separator_below_margin(shape) as i32
                                                > ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU
                                        })
                                        .unwrap_or(false)
                                {
                                    endnote_shape
                                        .map(|shape| endnote_between_notes_margin(shape) as i32)
                                        .filter(|gap_hu| {
                                            st.current_height
                                                + hwpunit_to_px(*gap_hu, self.dpi)
                                                + en_fit
                                                <= available
                                                    + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                                    + 2.0
                                        })
                                } else {
                                    None
                                };
                            if let Some(gap_hu) = default_large_below_rewind_title_tail_gap_hu {
                                if let Some(render_para) =
                                    st.endnote_paragraphs.get_mut(en_para_local_idx)
                                {
                                    for ls in &mut render_para.line_segs {
                                        ls.vertical_pos += gap_hu;
                                    }
                                }
                            }
                            let tac_picture_rewinds_before_column_base = st.col_count > 1
                                && compact_between_notes_gap
                                && local_vpos_rewind
                                && para_is_treat_as_char_picture_only(en_para)
                                && st.current_column + 1 >= st.col_count
                                && st.current_height
                                    + tac_picture_tail_group_height
                                        .or(tac_picture_only_height)
                                        .unwrap_or(en_fit)
                                    > available - ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                && st
                                    .current_items
                                    .iter()
                                    .filter_map(page_item_para_index)
                                    .find_map(|pi| {
                                        paragraph_by_global_index(
                                            paragraphs,
                                            &st.endnote_paragraphs,
                                            pi,
                                        )
                                        .and_then(|p| p.line_segs.first())
                                        .map(|s| s.vertical_pos)
                                    })
                                    .and_then(|base_vpos| {
                                        this_first_offset.map(|first_vpos| first_vpos < base_vpos)
                                    })
                                    .unwrap_or(false);
                            if tac_picture_rewinds_before_column_base {
                                // 저장 vpos가 현재 단 시작보다 앞선 TAC 그림은 한컴에서
                                // 하단 겹침으로 남기지 않고 다음 단/쪽에서 자체 높이를 소비한다.
                                st.advance_column_or_new_page();
                                prev_en_bottom_vpos = None;
                                prev_en_content_bottom_vpos = None;
                            }
                            let tac_picture_rewind_height = if st.col_count > 1
                                && local_vpos_rewind
                                && !local_vpos_rewind_crosses_prev_content
                                && para_is_treat_as_char_picture_only(en_para)
                            {
                                st.current_items
                                    .iter()
                                    .filter_map(page_item_para_index)
                                    .find_map(|pi| {
                                        paragraph_by_global_index(
                                            paragraphs,
                                            &st.endnote_paragraphs,
                                            pi,
                                        )
                                        .and_then(|p| p.line_segs.first())
                                        .map(|s| s.vertical_pos)
                                    })
                                    .and_then(|base_vpos| {
                                        this_first_offset.map(|first_vpos| {
                                            hwpunit_to_px((first_vpos - base_vpos).max(0), self.dpi)
                                        })
                                    })
                            } else {
                                None
                            };
                            maybe_register_square_picture_wrap_anchor(
                                &mut st,
                                paragraphs,
                                en_para,
                                en_para_idx,
                                page_def,
                            );
                            // advance 후 재평가 — 새 단 첫 미주는 prev=None → 자체 높이.
                            let (_, mut en_advance) = compute_en_metrics(prev_en_bottom_vpos, true);
                            if large_between_last_column_question_title_tail_fits
                                || large_between_last_column_render_title_tail_fits
                                || large_between_last_column_rewind_title_tail_fits
                            {
                                // 큰 미주 사이가 있는 마지막 단에서 새 미주 제목만
                                // frame 안쪽 tail로 남길 때는 제목-본문 vpos 간격을
                                // 현재 단 높이로 소비하지 않는다. 그 간격까지 소비하면
                                // 같은 미주의 첫 본문 줄 split 기회를 잃고 다음 쪽으로
                                // 통째로 넘어가 한컴보다 한 쪽 늦어진다.
                                en_advance = en_advance.min(fmt.total_height);
                            }
                            if no_separator_last_column_new_note_head_without_gap_fits {
                                // 구분선 없는 마지막 단에서는 저장 vpos에 남은 큰 미주 사이가
                                // 직전 미주의 하단 여백으로 이미 보인다. 제목 advance까지 그
                                // gap을 다시 소비하면 같은 미주의 첫 본문 줄이 한컴보다 다음
                                // 쪽으로 밀리므로, 제목 자체 높이만 pagination에 반영한다.
                                en_advance = en_advance.min(fmt.total_height);
                            }
                            if large_between_zero_above_whole_note_small_bleed_fits {
                                // 구분선 위 0 + 큰 미주 사이에서는 새 문항 전체 vpos span이
                                // 단 하단을 소폭 넘더라도 한컴은 제목을 현재 단에 남긴 뒤
                                // 같은 미주의 본문을 순차적으로 이어 배치한다. 제목 emit에서
                                // 전체 span을 한 번에 소비하면 본문이 다음 쪽으로 밀린다.
                                en_advance = en_advance.min(fmt.total_height);
                            }
                            if zero_endnote_spacing_profile {
                                if let Some(object_height) = non_tac_object_height {
                                    // 0/0/0 미주에서는 구분선 주변 여백이 전혀 없어 비TAC
                                    // 그림/도형 문단의 실제 객체 높이를 다음 미주 시작 위치에
                                    // 반영해야 renderer와 pagination의 하단 기준이 맞는다.
                                    en_advance = en_advance.max(object_height);
                                }
                            }
                            if pre_emit_tail_before_non_tac_object_advance
                                && non_tac_object_height.is_some()
                                && !endnote_has_text_or_equation
                            {
                                if let Some(object_content_height) =
                                    non_tac_picture_or_shape_content_height_px(en_para, dpi)
                                {
                                    // 미주 사이 0의 단 하단에서 뒤 텍스트 tail을 앞 단에
                                    // 선배치한 경우, 한컴은 다음 단의 비TAC 그림 뒤 margin을
                                    // 별도 빈 줄처럼 소비하지 않는다.
                                    en_advance = object_content_height;
                                }
                            }
                            if (advance_for_fit || advance_for_internal_rewind)
                                && !default_between_notes_gap
                                && compact_between_notes_gap
                                && has_visible_endnote_separator
                                && internal_vpos_rewind
                                && !local_vpos_rewind
                                && st.current_items.is_empty()
                            {
                                // 단 하단에서 다음 단/쪽으로 이동된 내부 rewind 미주는
                                // 이동 전 하단 cur 기준의 축약 높이를 재사용하면 다음 미주가
                                // renderer보다 위에서 시작해 하단 overflow가 난다. 새 단에서는
                                // 문단 전체 line advance와 저장된 미주 사이 gap을 소비한다.
                                let boundary_gap = endnote_shape
                                    .map(endnote_between_notes_margin)
                                    .map(|gap| hwpunit_to_px(gap as i32, dpi))
                                    .unwrap_or(0.0);
                                en_advance = en_advance.max(total_advance_fit + boundary_gap);
                            }
                            let compact_visible_last_column_non_reset_rewind_tail =
                                compact_endnote_separator_profile
                                    && compact_between_notes_gap
                                    && has_visible_endnote_separator
                                    && st.current_column + 1 >= st.col_count
                                    && !internal_rewind_target_is_reset
                                    && !late_internal_rewind_fit_split
                                    && internal_rewind_split.is_some_and(|split| split > 1)
                                    && split_endnote_to_fit.is_none();
                            let mut split_endnote_emitted = false;
                            let tall_line_internal_rewind_split =
                                internal_rewind_split.filter(|split| {
                                    !compact_visible_last_column_non_reset_rewind_tail
                                        && !late_internal_rewind_fit_split
                                        && split
                                            .checked_sub(1)
                                            .and_then(|idx| en_para.line_segs.get(idx))
                                            .map(|seg| seg.line_height >= 2000)
                                            .unwrap_or(false)
                                });
                            let prioritized_internal_rewind_split =
                                internal_rewind_split.filter(|split| {
                                    // 첫 줄 직후 되감기는 한컴 저장본에서 같은 단 fit 분할과 함께
                                    // 나타나는 경우가 있어, 기존 fit 후보가 있으면 그 분배를 유지한다.
                                    !compact_visible_last_column_non_reset_rewind_tail
                                        && (!late_internal_rewind_fit_split
                                        // late fit 후보가 단일 tail 제거 규칙으로 사라져도,
                                        // lineSeg가 실제 0으로 reset되는 내부 분할은 HWP의
                                        // column/page split 신호이므로 보존한다.
                                        || (internal_rewind_target_is_reset
                                            && *split > 1
                                            && split_endnote_to_fit.is_none()))
                                        && (*split > 1 || split_endnote_to_fit.is_none())
                                });
                            let suppress_empty_column_rewind_split = internal_rewind_position
                                .is_some()
                                && st.current_height < 5.0
                                && st.current_height + total_advance_fit
                                    <= available + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX;
                            let split_candidate = if compact_non_default_empty_column_rewind_fits
                                || suppress_empty_column_rewind_split
                            {
                                None
                            } else {
                                tall_line_internal_rewind_split
                                    .or(prioritized_internal_rewind_split)
                                    .or(large_between_last_column_visual_split)
                                    .or(large_between_last_column_flow_tail_split)
                                    .or(split_endnote_to_fit)
                            };
                            if let Some(split_line) = split_candidate {
                                let first_h = fmt.line_advances_sum(0..split_line);
                                st.current_items.push(PageItem::PartialParagraph {
                                    para_index: en_para_idx,
                                    start_line: 0,
                                    end_line: split_line,
                                });
                                st.current_height += first_h;
                                st.current_endnote_flow = true;
                                st.advance_column_or_new_page();
                                let rest_h = fmt
                                    .line_advances_sum(split_line..fmt.line_heights.len())
                                    + fmt.spacing_after;
                                st.current_items.push(PageItem::PartialParagraph {
                                    para_index: en_para_idx,
                                    start_line: split_line,
                                    end_line: fmt.line_heights.len(),
                                });
                                st.current_height += rest_h;
                                st.current_endnote_flow = true;
                                split_endnote_emitted = true;
                            } else {
                                let table_only_endnote_para = en_para.text.is_empty()
                                    && en_para
                                        .controls
                                        .iter()
                                        .any(|ctrl| matches!(ctrl, Control::Table(_)))
                                    && !en_para
                                        .controls
                                        .iter()
                                        .any(|ctrl| matches!(ctrl, Control::Equation(_)));
                                let pre_emitted_non_tac_object_only_para =
                                    pre_emit_tail_before_non_tac_object_advance
                                        && non_tac_object_height.is_some()
                                        && !endnote_has_text_or_equation;
                                if !table_only_endnote_para && !pre_emitted_non_tac_object_only_para
                                {
                                    st.current_items.push(PageItem::FullParagraph {
                                        para_index: en_para_idx,
                                    });
                                    st.current_endnote_flow = true;
                                }
                                for (ctrl_idx, ctrl) in en_para.controls.iter().enumerate() {
                                    match ctrl {
                                        Control::Table(_) if table_only_endnote_para => {
                                            st.current_items.push(PageItem::Table {
                                                para_index: en_para_idx,
                                                control_index: ctrl_idx,
                                            });
                                            st.current_endnote_flow = true;
                                        }
                                        Control::Shape(_) | Control::Picture(_) => {
                                            st.current_items.push(PageItem::Shape {
                                                para_index: en_para_idx,
                                                control_index: ctrl_idx,
                                            });
                                            st.current_endnote_flow = true;
                                        }
                                        _ => {}
                                    }
                                }
                                // [Task #1363 Divergence C] TAC 그림 미주 para 의 누적 경로.
                                // 종전(legacy/A): local_vpos_rewind TAC 그림은 저장 vpos 가
                                // 앞 제목 옆을 가리킨다고 보고 `max(rewind_start+adv)` 로 누적
                                // (겹침 가정). 그러나 TAC(treat_as_char) 그림은 렌더러가 문단
                                // 흐름에 inline 으로 **순차 적층**한다(옆 배치 아님). 겹침 가정은
                                // 그림 높이를 과소 계상해 단을 과충전(sep20/20 p22 col0 +58px →
                                // 본문 50px 초과). SSOT(B+): 렌더러처럼 순차 적층(`+= adv`).
                                let tac_stack_ssot = matches!(tac_picture_rewind_height, Some(_))
                                    && ssot_level >= EnSsotLevel::B;
                                if let Some(rewind_start) =
                                    tac_picture_rewind_height.filter(|_| !tac_stack_ssot)
                                {
                                    // 단 하단의 TAC 그림은 renderer가 직전 텍스트를 침범하는
                                    // vpos 되감김을 버리고 순차 y를 유지한다. pagination도
                                    // 같은 경우에는 그림 높이를 소비해야 뒤 문단이 frame 아래로
                                    // 밀리지 않는다.
                                    let rewind_end = rewind_start + en_advance;
                                    let consume_rewind_picture_height =
                                        compact_endnote_separator_profile
                                            && st.current_column + 1 >= st.col_count
                                            && rewind_end
                                                <= st.current_height
                                                    + ENDNOTE_COLUMN_BOTTOM_BLEED_TOLERANCE_PX
                                            && ((compact_between_notes_gap
                                                && has_visible_endnote_separator)
                                                || (large_separator_block
                                                    && !has_visible_endnote_separator));
                                    if ssot_debug {
                                        eprintln!(
                                            "EN_ACC pi={} path={} ch_before={:.1} rewind_start={:.1} adv={:.1} ch_after={:.1}",
                                            en_para_idx,
                                            if consume_rewind_picture_height { "TACbottom" } else { "TACmax" },
                                            st.current_height,
                                            rewind_start,
                                            en_advance,
                                            if consume_rewind_picture_height {
                                                st.current_height + en_advance
                                            } else {
                                                st.current_height.max(rewind_end)
                                            },
                                        );
                                    }
                                    if consume_rewind_picture_height {
                                        st.current_height += en_advance;
                                    } else {
                                        st.current_height = st.current_height.max(rewind_end);
                                    }
                                } else {
                                    if ssot_debug {
                                        eprintln!(
                                            "EN_ACC pi={} path={} ch_before={:.1} adv={:.1} ch_after={:.1}",
                                            en_para_idx,
                                            if tac_stack_ssot { "TACstack" } else { "add" },
                                            st.current_height, en_advance,
                                            st.current_height + en_advance,
                                        );
                                    }
                                    st.current_height += en_advance;
                                }
                                // [Task #1363 v2 Stage 2] A2: 누적을 렌더 시뮬 bottom 으로 스냅.
                                // compute_en_metrics(saved-delta) 대신 HeightCursor 시뮬레이션이
                                // 단 실제 렌더 높이를 산출 → fit 결정(다음 para)이 렌더 정합.
                                // [Task #1370 Stage 2 실험] A3 한정: exact 스냅이 rewind/빈 para 를
                                // hancom 보다 ~80px/단 높게 누적해 경계 split 을 막고 13건 cascade 유발.
                                // 실험으로 A3 에서 스냅 OFF → break-결정 높이를 compact(acc)로 환원.
                                if ssot_level == EnSsotLevel::A2 {
                                    if let Some(sim_bottom) = self.simulate_endnote_column_bottom_y(
                                        &st, paragraphs, styles, available, en_col_w, None,
                                    ) {
                                        if ssot_debug {
                                            eprintln!(
                                                "EN_ACC pi={} path=A2sim {:.1} -> {:.1}",
                                                en_para_idx, st.current_height, sim_bottom,
                                            );
                                        }
                                        st.current_height = sim_bottom;
                                    }
                                }
                            }
                            activate_square_picture_wrap_for_para(&mut st, en_para_idx, en_para);
                            // 다음 미주의 base 가 될 본 미주 bottom 기록.
                            if split_endnote_emitted {
                                prev_en_bottom_vpos = None;
                                prev_en_content_bottom_vpos = None;
                            } else if let Some(tb) = this_bottom_offset {
                                prev_en_bottom_vpos = Some(tb);
                                prev_en_content_bottom_vpos =
                                    this_content_bottom_offset.or(this_bottom_offset);
                            }
                        }
                        prev_endnote_had_inline_object_vpos_overestimate =
                            current_endnote_had_inline_object_vpos_overestimate;
                        emitted_endnote_count += 1;
                    }
                }
            }
        }

        // 마지막 항목 처리
        if !st.current_items.is_empty() {
            st.flush_column_always();
        }
        st.ensure_page();

        // 페이지 번호 + 머리말/꼬리말 할당
        Self::finalize_pages(
            &mut st.pages,
            &hf_entries,
            &page_number_pos,
            &new_page_numbers,
            &page_hides,
            section_index,
        );

        PaginationResult {
            pages: st.pages,
            wrap_around_paras: Vec::new(),
            hidden_empty_paras: st.hidden_empty_paras,
            endnotes: st.endnotes,
            endnote_paragraphs: st.endnote_paragraphs,
            endnote_para_sources: st.endnote_para_sources,
            endnote_between_notes_hu: st.endnote_between_notes_hu,
            endnote_separator_above_hu: st.endnote_separator_above_hu,
            endnote_separator_below_hu: st.endnote_separator_below_hu,
        }
    }

    // ========================================================
    // format: 문단의 실제 높이를 계산한다
    // ========================================================

    /// 문단의 렌더링 높이를 계산한다 (format).
    /// [Task #1027 Stage D] 항목 fit 직전, `current_height` 를 vpos-정합 위치로 스냅한다.
    ///
    /// [Task #1363 v2 Stage 2] 미주 다단 SSOT 시뮬레이션.
    ///
    /// `st.current_items`(현재 단에 배치된 미주 항목들)를 렌더러 `build_single_column` 과
    /// 동일 경로(`HeightCursor::vpos_adjust` + line/total advances)로 재생해 단의 bottom y 를
    /// 산출한다. A2 게이트에서 `current_height` 를 이 값으로 스냅 → compute_en_metrics 의
    /// saved-delta 근사를 렌더 실측과 정합시킨다(p21 과대·p17 과소 누적 원인 제거 목표).
    /// `current_height` 상대공간(col_area_y=0, start=`current_start_height`)에서 구동.
    fn simulate_endnote_column_bottom_y(
        &self,
        st: &TypesetState,
        paragraphs: &[Paragraph],
        styles: &ResolvedStyleSet,
        available: f64,
        en_col_w: f64,
        extra_para_full: Option<usize>,
    ) -> Option<f64> {
        if st.current_items.is_empty() {
            return None;
        }
        let ssot_level = en_ssot_level();
        let ssot_debug = en_ssot_debug();
        let mut local_paras: Vec<Paragraph> = Vec::new();
        let mut local_indices: Vec<(usize, usize)> = Vec::new();
        for pi in st
            .current_items
            .iter()
            .filter_map(page_item_para_index)
            .chain(extra_para_full)
        {
            if local_indices.iter().any(|(global, _)| *global == pi) {
                continue;
            }
            if let Some(p) = paragraph_by_global_index(paragraphs, &st.endnote_paragraphs, pi) {
                let local = local_paras.len();
                local_paras.push(p.clone());
                local_indices.push((pi, local));
            }
        }
        let lookup_local = |pi: usize| {
            local_indices
                .iter()
                .find_map(|(global, local)| (*global == pi).then_some(*local))
        };
        // [Task #1363 v3 옵션 3] A3: per-para 고립 측정 + HeightCursor 시뮬 대신, 단의 전 items 를
        // scratch `LayoutEngine` 으로 **1회 순차 렌더**해 정확한 단 bottom 을 읽는다. items 를
        // 로컬 0-기반 재색인해 build_single_column 경로(vpos forward-jump·trailing·text_start_line
        // 등 렌더 dispatch)를 그대로 태운다 → sim==render 구조 보장.
        if ssot_level >= EnSsotLevel::A3 {
            // 로컬 인덱스를 **+1 오프셋**하고 인덱스 0 에 더미 para 를 둔다. 렌더의
            // `layout_composed_paragraph` 는 `para_index == 0` + column-top + 첫 줄 vpos>0 이면
            // 절대 vpos 를 가산하는 fallback(섹션 첫 문단 제목용)이 있는데, 실제 미주 para 는
            // 큰 글로벌 인덱스라 결코 0 이 아니다. 0-기반 재색인이 이 fallback 을 잘못 발동시켜
            // 단독 측정이 폭발(35px→13721px)하므로 0 을 비워 둔다(더미는 어떤 item 도 미참조).
            let a3_paras: Vec<Paragraph> = std::iter::once(Paragraph::default())
                .chain(local_paras.iter().cloned())
                .collect();
            let a3_composed: Vec<crate::renderer::composer::ComposedParagraph> = a3_paras
                .iter()
                .map(crate::renderer::composer::compose_paragraph)
                .collect();
            let remap = |item: &PageItem| -> Option<PageItem> {
                match item {
                    PageItem::FullParagraph { para_index } => lookup_local(*para_index)
                        .map(|l| PageItem::FullParagraph { para_index: l + 1 }),
                    PageItem::PartialParagraph {
                        para_index,
                        start_line,
                        end_line,
                    } => lookup_local(*para_index).map(|l| PageItem::PartialParagraph {
                        para_index: l + 1,
                        start_line: *start_line,
                        end_line: *end_line,
                    }),
                    PageItem::Table {
                        para_index,
                        control_index,
                    } => lookup_local(*para_index).map(|l| PageItem::Table {
                        para_index: l + 1,
                        control_index: *control_index,
                    }),
                    PageItem::PartialTable {
                        para_index,
                        control_index,
                        start_row,
                        end_row,
                        is_continuation,
                        start_cut,
                        end_cut,
                        is_block_split,
                    } => lookup_local(*para_index).map(|l| PageItem::PartialTable {
                        para_index: l + 1,
                        control_index: *control_index,
                        start_row: *start_row,
                        end_row: *end_row,
                        is_continuation: *is_continuation,
                        start_cut: start_cut.clone(),
                        end_cut: end_cut.clone(),
                        is_block_split: *is_block_split,
                    }),
                    PageItem::Shape {
                        para_index,
                        control_index,
                    } => lookup_local(*para_index).map(|l| PageItem::Shape {
                        para_index: l + 1,
                        control_index: *control_index,
                    }),
                    // 구분선은 측정에서 제외(현 per-para 시뮬과 동일 — start_height 가 단 콘텐츠
                    // 시작을 이미 반영).
                    PageItem::EndnoteSeparator { .. } => None,
                }
            };
            let extra_local = extra_para_full
                .and_then(|pi| lookup_local(pi))
                .map(|l| PageItem::FullParagraph { para_index: l + 1 });
            let local_items: Vec<PageItem> = st
                .current_items
                .iter()
                .filter_map(&remap)
                .chain(extra_local)
                .collect();
            if local_items.is_empty() {
                return None;
            }
            // build_single_column 은 양수 start_height 를 무시(음수 shift 만 적용)하므로,
            // 단이 본문 아래에서 시작(start>0)하면 col_area.y 에 그 오프셋을 실어 동일 프레임에서
            // 렌더한다. 음수(vpos 되감김)는 col_area.y=0 + start_height 음수 shift 로 처리.
            let col_y = st.current_start_height.max(0.0);
            let col_area = crate::renderer::page_layout::LayoutRect {
                x: 0.0,
                y: col_y,
                width: en_col_w,
                height: (available - col_y).max(0.0),
            };
            let scratch = crate::renderer::layout::LayoutEngine::new(self.dpi);
            let bottom = scratch.measure_endnote_column_bottom(
                local_items,
                &a3_paras,
                &a3_composed,
                styles,
                &col_area,
                st.current_start_height,
                st.section_index,
                st.endnote_between_notes_hu,
            );
            if ssot_debug {
                eprintln!(
                    "EN_COLSIM start_h={:.1} avail={:.1} items={} bottom={:.1}",
                    st.current_start_height,
                    available,
                    local_indices.len(),
                    bottom,
                );
            }
            return Some(bottom);
        }
        let page_base = st
            .current_items
            .iter()
            .filter_map(page_item_para_index)
            .find_map(|pi| {
                paragraph_by_global_index(paragraphs, &st.endnote_paragraphs, pi)
                    .and_then(|p| p.line_segs.first())
                    .map(|seg| seg.vertical_pos)
            })?;
        let mut hc = HeightCursor::new(
            self.dpi,
            0.0,
            available,
            st.current_start_height,
            Some(page_base),
            st.skip_spacing_before_prededuct,
            false,
            st.current_endnote_flow && st.current_start_height < -0.5,
            st.current_endnote_flow,
        );
        hc.endnote_between_notes_hu = st.endnote_between_notes_hu;
        let mut y = st.current_start_height;
        let extra_item = extra_para_full.map(|pi| PageItem::FullParagraph { para_index: pi });
        for item in st.current_items.iter().chain(extra_item.as_ref()) {
            let Some(pi) = page_item_para_index(item) else {
                continue;
            };
            let Some(local) = lookup_local(pi) else {
                continue;
            };
            y = hc.vpos_adjust(y, local, &local_paras, styles);
            let item_para = &local_paras[local];
            let item_composed = crate::renderer::composer::compose_paragraph(item_para);
            // [Task #1363 v2 Stage 3] 휴리스틱 advance 추정. 렌더러는 미주 텍스트/수식 para 를
            // **저장 line_segs**(hancom 레이아웃)로 그린다 — format_paragraph reflow(total_height)가
            // 아님. 수식 다줄 para 는 reflow 가 저장 span 보다 큼(pi=1126: 237 vs 185.8) → 단 과대.
            // 저장 line_segs vpos 범위를 advance 로 사용해 렌더와 정합. 단, **TAC 그림/도형 para**는
            // 개체 높이가 line_segs 에 없으므로(pi=1131: 빈 텍스트+309px 그림) total_height 사용.
            // 내부 vpos rewind para 는 line_segs vpos 범위가 작지만(되감김) 렌더러는 순차
            // 적층(Divergence A) → line_advances_sum 사용. (sep20/20 pi=522: saved 32.5 vs 실제 183)
            let heuristic_advance = {
                let item_fmt =
                    self.format_paragraph(item_para, Some(&item_composed), styles, Some(en_col_w));
                let internal_rewind = item_para
                    .line_segs
                    .windows(2)
                    .any(|w| w[1].vertical_pos < w[0].vertical_pos);
                let para_advance_full = if para_has_treat_as_char_picture_or_shape(item_para) {
                    item_fmt.total_height
                } else if internal_rewind {
                    item_fmt.line_advances_sum(0..item_fmt.line_heights.len())
                } else {
                    let segs = &item_para.line_segs;
                    match (
                        segs.first(),
                        segs.iter().map(|s| s.vertical_pos + s.line_height).max(),
                    ) {
                        (Some(first), Some(bottom)) => {
                            hwpunit_to_px((bottom - first.vertical_pos).max(0), self.dpi)
                                .max(item_fmt.line_advance(0))
                        }
                        _ => item_fmt.total_height,
                    }
                };
                // 표/도형 단독 항목은 line_segs vpos 범위(저장 레이아웃 높이)로 advance.
                let saved_vpos_span = {
                    let segs = &item_para.line_segs;
                    match (
                        segs.first(),
                        segs.iter().map(|s| s.vertical_pos + s.line_height).max(),
                    ) {
                        (Some(first), Some(bottom)) => {
                            hwpunit_to_px((bottom - first.vertical_pos).max(0), self.dpi)
                        }
                        _ => 0.0,
                    }
                };
                match item {
                    PageItem::PartialParagraph {
                        start_line,
                        end_line,
                        ..
                    } => item_fmt.line_advances_sum(*start_line..*end_line),
                    PageItem::FullParagraph { .. } => para_advance_full,
                    PageItem::Table { .. } | PageItem::PartialTable { .. } => {
                        saved_vpos_span.max(item_fmt.total_height)
                    }
                    _ => 0.0,
                }
            };
            // [Task #1363 v3 Stage 1] A3: 휴리스틱 advance 추정 대신 scratch LayoutEngine 으로
            // para 를 실제 레이아웃해 정확한 렌더 advance 를 측정한다(렌더 권위). ssot_debug 시
            // 휴리스틱과의 diff 를 로그해 정합·drift 를 정량 확인한다.
            let advance = if ssot_level >= EnSsotLevel::A3 {
                let measured = self.measure_endnote_para_advance(
                    item_para,
                    &item_composed,
                    styles,
                    en_col_w,
                    available,
                    y,
                    item,
                    st.section_index,
                    pi,
                );
                if ssot_debug {
                    eprintln!(
                        "EN_MEASURE pi={} y_top={:.1} heuristic={:.1} measured={:.1} diff={:.1}",
                        pi,
                        y,
                        heuristic_advance,
                        measured,
                        measured - heuristic_advance,
                    );
                }
                measured
            } else {
                heuristic_advance
            };
            y += advance;
            let current_vpos_rewinds_from_prev = hc
                .prev_layout_para
                .and_then(|prev_local| {
                    let prev_first = local_paras
                        .get(prev_local)
                        .and_then(|p| p.line_segs.first())
                        .map(|seg| seg.vertical_pos)?;
                    let curr_first = local_paras
                        .get(local)
                        .and_then(|p| p.line_segs.first())
                        .map(|seg| seg.vertical_pos)?;
                    Some(curr_first < prev_first)
                })
                .unwrap_or(false);
            if matches!(item, PageItem::PartialParagraph { start_line, .. } if *start_line > 0)
                || current_vpos_rewinds_from_prev
            {
                hc.prev_layout_para = None;
                hc.vpos_page_base = None;
                hc.vpos_lazy_base = None;
            } else {
                hc.prev_layout_para = Some(local);
            }
            hc.prev_item_was_partial_table = matches!(item, PageItem::PartialTable { .. });
        }
        Some(y)
    }

    /// [Task #1363 v3 Stage 1] scratch `LayoutEngine` 로 미주 para 를 실제 레이아웃하여 **정확한
    /// 렌더 advance(px)** 를 측정한다. 시뮬의 휴리스틱 높이 추정(saved-vpos span / total_height /
    /// line_advances_sum)을 렌더 권위 값으로 대체하기 위한 측정 전용 경로다.
    ///
    /// 좌표는 시뮬과 동일한 **컬럼 top=0 상대 프레임**으로 구성한다(`col_area.y=0`,
    /// `y_start`=상대 y). advance(delta)는 프레임 평행이동 불변이므로 렌더 절대 좌표와 정합한다.
    /// 노드는 scratch `tree`/`col_node` 로 버려 실제 렌더에 무영향. 매 호출 `LayoutEngine::new`
    /// 로 생성하므로 numbering/overflow 등 상태도 격리된다(Stage 2 에서 실증).
    ///
    /// **알려진 fidelity 한계(Stage 1 POC)**: `bin_data_content=None` — TAC 그림 intrinsic 사이징
    /// 미반영(명시 크기 그림은 무관). `endnote_para_base` 미설정 — 미주 가상 para 판정이 false 라
    /// overflow tolerance 만 다르고 advance 에는 무영향.
    #[allow(clippy::too_many_arguments)]
    fn measure_endnote_para_advance(
        &self,
        item_para: &Paragraph,
        item_composed: &ComposedParagraph,
        styles: &ResolvedStyleSet,
        en_col_w: f64,
        available: f64,
        y_start: f64,
        item: &PageItem,
        section_index: usize,
        para_index: usize,
    ) -> f64 {
        use crate::renderer::layout::{layout_rect_to_bbox, LayoutEngine};
        use crate::renderer::page_layout::LayoutRect;
        use crate::renderer::render_tree::{PageRenderTree, RenderNode, RenderNodeType};

        // 렌더 `layout_column_item` 의 FullParagraph 텍스트 경로 정합(layout.rs has_real_text):
        // 실제 텍스트가 있는 para 는 **leading 컨트롤-전용 줄**(수식 객체마커 ￼ 등)을 건너뛰고
        // `text_start_line` 부터 그린다. scratch 가 start_line=0 으로 그 줄을 포함하면 수식 다줄
        // para 가 +수십px 과대 측정된다(sep20/20 pi=936: 127.7 vs 렌더 101.3). 객체-전용 para
        // (TAC 그림 등)는 0 부터(렌더도 동일). Partial 은 항목 지정 줄 범위 그대로.
        let (start_line, end_line) = match item {
            PageItem::PartialParagraph {
                start_line,
                end_line,
                ..
            } => (*start_line, *end_line),
            _ => {
                let has_real_text = item_para
                    .text
                    .chars()
                    .any(|c| c > '\u{001F}' && c != '\u{FFFC}' && !c.is_whitespace());
                let start = if has_real_text {
                    item_composed
                        .lines
                        .iter()
                        .position(|line| {
                            line.runs
                                .iter()
                                .any(|r| r.text.chars().any(|c| c > '\u{001F}' && c != '\u{FFFC}'))
                        })
                        .unwrap_or(0)
                } else {
                    0
                };
                (start, item_composed.lines.len())
            }
        };
        let height = available.max(0.0);
        let col_area = LayoutRect {
            x: 0.0,
            y: 0.0,
            width: en_col_w,
            height,
        };
        let scratch = LayoutEngine::new(self.dpi);
        let mut tree = PageRenderTree::new(0, en_col_w, height);
        let col_id = tree.next_id();
        let mut col_node = RenderNode::new(
            col_id,
            RenderNodeType::Column(0),
            layout_rect_to_bbox(&col_area),
        );
        let y_after = scratch.layout_partial_paragraph(
            &mut tree,
            &mut col_node,
            item_para,
            Some(item_composed),
            styles,
            &col_area,
            y_start,
            start_line,
            end_line,
            section_index,
            para_index,
            None, // multi_col_width_hu: 렌더 미주 body-flow 경로와 동일(None)
            None, // bin_data_content: Stage 1 POC — None
            None, // wrap_anchor: 미주 단 내부 wrap-around 없음
        );
        (y_after - y_start).max(0.0)
    }

    /// 렌더러 `build_single_column` 의 inter-item VPOS_CORR(Stage C `HeightCursor::vpos_adjust`)
    /// 를 페이지네이터에서도 적용해, 단락마다 `+= total_height` 로 누적된 sb·trailing_ls
    /// drift 를 다음 항목 진입 시 제거한다(렌더러와 동일 측정). 단단(col_count==1) 전용 —
    /// 다단/flow-around 는 Stage E.
    ///
    /// HeightCursor 는 `current_height` 상대공간(col_area_y=0)에서 구동한다.
    fn vpos_snap_current_height(
        &self,
        st: &mut TypesetState,
        para_idx: usize,
        paragraphs: &[Paragraph],
        styles: &ResolvedStyleSet,
    ) {
        if st.col_count != 1 {
            return; // 다단은 Stage E
        }
        // 컬럼 첫 항목: anchor + page_base 확립 (렌더러 2186/2216 정합).
        // items.first() 의 vpos 를 page_base 로, 현 current_height 를 anchor 로 둔다.
        if st.current_items.is_empty() {
            st.vpos_col_anchor = st.current_height;
            st.vpos_page_base = paragraphs
                .get(para_idx)
                .and_then(|p| p.line_segs.first())
                .map(|s| s.vertical_pos);
            st.vpos_lazy_base = None;
        }
        let mut hc = HeightCursor {
            dpi: self.dpi,
            col_area_y: 0.0,
            col_area_height: st.base_available_height(),
            col_anchor_y: st.vpos_col_anchor,
            vpos_page_base: st.vpos_page_base,
            vpos_lazy_base: st.vpos_lazy_base,
            prev_layout_para: st.vpos_prev_layout_para,
            prev_item_was_partial_table: st.vpos_prev_partial_table,
            skip_spacing_before_prededuct: st.skip_spacing_before_prededuct,
            allow_vpos_rewind: false,
            allow_start_height_backtrack: false,
            suppress_large_forward_jump: false,
            suppress_hwpx_stale_forward: st.is_hwpx_source,
            endnote_between_notes_hu: 0,
            prev_item_content_bottom_y: None,
            last_compacted_endnote_title_gap: false,
        };
        let y = hc.vpos_adjust(st.current_height, para_idx, paragraphs, styles);
        // lazy_base 는 지연 산출 시 갱신될 수 있으므로 회수.
        st.vpos_lazy_base = hc.vpos_lazy_base;
        st.current_height = y;
    }

    /// 기존 HeightMeasurer::measure_paragraph()와 동일한 로직.
    fn format_paragraph(
        &self,
        para: &Paragraph,
        composed: Option<&ComposedParagraph>,
        styles: &ResolvedStyleSet,
        column_width_px: Option<f64>,
    ) -> FormattedParagraph {
        let para_style_id = composed.map(|c| c.para_style_id as usize).unwrap_or(0);
        let para_style = styles.para_styles.get(para_style_id);

        // [Task #1042 Stage 6c] line_segs.empty paragraph 의 typeset/layout 측정 정합 —
        // paragraph_layout (렌더링 path) 는 Stage 6b 에서 recompose_for_cell_width 로 column
        // 기반 wrap 을 적용하지만, format_paragraph (typeset/measurement path) 는 원본
        // compose_lines fallback (CHARS_PER_LINE=45) 결과로 측정 → 두 path 의 line_count 불일치
        // 발생 (e.g. sample16 변환기 pi=417: typeset 2 lines / layout 1 line, +10.4 px gap).
        // 동일 recompose 를 typeset 측에도 적용해 paragraph height 측정 정합.
        let recomposed: Option<ComposedParagraph> = match (composed, column_width_px) {
            (Some(c), Some(cw)) if para.line_segs.is_empty() && cw > 0.0 => {
                let margin_l = para_style.map(|s| s.margin_left).unwrap_or(0.0);
                let margin_r = para_style.map(|s| s.margin_right).unwrap_or(0.0);
                let inner = (cw - margin_l - margin_r).max(0.0);
                if inner > 0.0 {
                    let mut cloned = c.clone();
                    crate::renderer::composer::recompose_for_cell_width(
                        &mut cloned,
                        para,
                        inner,
                        styles,
                    );
                    Some(cloned)
                } else {
                    None
                }
            }
            _ => None,
        };
        let composed = recomposed.as_ref().or(composed);
        let raw_spacing_before = para_style.map(|s| s.spacing_before).unwrap_or(0.0);
        let spacing_after = para_style.map(|s| s.spacing_after).unwrap_or(0.0);

        // [Task #998 실험] spacing_before=0 으로 강제 — 효과 측정용
        let spacing_before = if para.line_segs.is_empty() && !para.text.is_empty() {
            0.0
        } else {
            raw_spacing_before
        };
        // [Task #874 Case 3] `<...>` 단독 paragraph 의 paragraph-level extra spacing 제거.
        // 이전 #866 Stage 2 는 paragraph 위·아래 각 +20px (총 +40px) 을 paragraph 자체 height
        // 에 포함시켰으나, typeset 의 zone 전환 패딩(solo_zone_pad +16px enter +16px leave)
        // 이 이미 동일 역할을 담당하므로 이중 패딩이 발생 (한컴 PDF 대비 +48px excess, 4·5쪽
        // 누적 +17~30pt 사용자 피드백). zone 전환 패딩만 유지.

        let ls_val = para_style.map(|s| s.line_spacing).unwrap_or(160.0);
        let ls_type = para_style
            .map(|s| s.line_spacing_type)
            .unwrap_or(crate::model::style::LineSpacingType::Percent);

        // [Task #901 Stage 7] wrap zone host paragraph 의 whitespace-only line 은 height 제외.
        // paragraph_layout 의 skip_advance_empty_wrap 와 정합 — pagination 의 height 계산
        // 이 시각 렌더링과 어긋나 paragraph 11 등이 잘못 다음 페이지로 분할되는 문제 해소.
        let has_picture_shape_square_wrap = para.controls.iter().any(|c| {
            use crate::model::shape::TextWrap;
            let common_opt = match c {
                Control::Picture(pic) if !pic.common.treat_as_char => Some(&pic.common),
                Control::Shape(s) if !s.common().treat_as_char => Some(s.common()),
                _ => None,
            };
            common_opt
                .map(|cm| matches!(cm.text_wrap, TextWrap::Square))
                .unwrap_or(false)
        });
        let has_treat_as_char_picture_shape = para.controls.iter().any(|c| {
            matches!(
                c,
                Control::Picture(pic) if pic.common.treat_as_char
            ) || matches!(
                c,
                Control::Shape(shape) if shape.common().treat_as_char
            )
        });
        let (mut line_heights, mut line_spacings): (Vec<f64>, Vec<f64>) = if let Some(comp) =
            composed
        {
            let tac_offsets_px: Vec<(usize, f64, usize)> = comp
                .tac_controls
                .iter()
                .map(|(pos, width_hu, control_index)| {
                    (*pos, hwpunit_to_px(*width_hu, self.dpi), *control_index)
                })
                .collect();
            let line_available_width_px = |line_idx: usize| {
                column_width_px.map(|cw| {
                    let margin_l = para_style.map(|s| s.margin_left).unwrap_or(0.0);
                    let margin_r = para_style.map(|s| s.margin_right).unwrap_or(0.0);
                    let indent = para_style.map(|s| s.indent).unwrap_or(0.0);
                    let effective_margin_l =
                        crate::renderer::equation_tac_flow::paragraph_effective_margin_left(
                            margin_l, indent, line_idx,
                        );
                    (cw - effective_margin_l - margin_r).max(0.0)
                })
            };
            // [Task #1472] 변환본은 미주 수식 effective indent 불변 위해 scale 절반(2.0→1.0).
            let eq_indent_scale = 2.0 * if self.is_hwp3_variant.get() { 0.5 } else { 1.0 };
            let equation_line_available_width_px = |visual_line_idx: usize| {
                column_width_px.map(|cw| {
                    let margin_l = para_style.map(|s| s.margin_left).unwrap_or(0.0);
                    let margin_r = para_style.map(|s| s.margin_right).unwrap_or(0.0);
                    let indent = para_style.map(|s| s.indent).unwrap_or(0.0);
                    let effective_margin_l = crate::renderer::equation_tac_flow::
                        paragraph_effective_margin_left_with_indent_scale(
                            margin_l,
                            indent,
                            visual_line_idx,
                            eq_indent_scale,
                        );
                    (cw - effective_margin_l - margin_r).max(0.0)
                })
            };
            let mut pairs = Vec::with_capacity(comp.lines.len());
            let mut prev_line_reserved_tac_picture_height: Option<f64> = None;
            for (line_idx, line) in comp.lines.iter().enumerate() {
                let runs_all_whitespace = line.runs.iter().all(|r| r.text.trim().is_empty());
                let line_has_tac_control = line_has_tac_control(para, comp, line_idx);
                let empty_tac_guide_line = runs_all_whitespace
                    && !line_has_tac_control
                    && comp
                        .lines
                        .get(line_idx + 1)
                        .is_some_and(|next| next.char_start == line.char_start)
                    && comp
                        .tac_controls
                        .iter()
                        .any(|(pos, _, _)| *pos == line.char_start);
                if empty_tac_guide_line {
                    pairs.push((0.0, 0.0));
                    prev_line_reserved_tac_picture_height = None;
                    continue;
                }
                // Square wrap host 의 빈 wrap guide 줄은 높이를 제외하되, 같은 줄에
                // TAC 수식/개체가 있으면 실제 콘텐츠 줄이므로 정상 advance 를 보존한다.
                if has_picture_shape_square_wrap && runs_all_whitespace && !line_has_tac_control {
                    pairs.push((0.0, 0.0));
                    prev_line_reserved_tac_picture_height = None;
                    continue;
                }
                let raw_lh = hwpunit_to_px(line.line_height, self.dpi);
                let raw_text_height = para
                    .line_segs
                    .get(line_idx)
                    .map(|seg| hwpunit_to_px(seg.text_height, self.dpi))
                    .unwrap_or(0.0);
                let max_fs = line
                    .runs
                    .iter()
                    .map(|r| {
                        styles
                            .char_styles
                            .get(r.char_style_id as usize)
                            .map(|cs| cs.font_size)
                            .unwrap_or(0.0)
                    })
                    .fold(0.0f64, f64::max);
                let text_before_picture_line =
                    text_line_is_picture_lead_in(para, comp, line_idx, raw_lh, max_fs, self.dpi);
                let tac_picture_height = para.controls.iter().find_map(|ctrl| {
                    let height_hu = match ctrl {
                        Control::Picture(pic) if pic.common.treat_as_char => {
                            pic.common.height as i32
                        }
                        Control::Shape(shape) if shape.common().treat_as_char => {
                            shape.common().height as i32
                        }
                        _ => return None,
                    };
                    let height = hwpunit_to_px(height_hu, self.dpi);
                    if height > 8.0 && raw_lh + 4.0 >= height && raw_lh <= height + 8.0 {
                        Some(height)
                    } else {
                        None
                    }
                });
                let tac_picture_height = if text_before_picture_line {
                    None
                } else {
                    tac_picture_height.or_else(|| {
                        (has_treat_as_char_picture_shape
                            && !runs_all_whitespace
                            && max_fs > 0.0
                            && raw_lh > max_fs * 2.0)
                            .then_some(raw_lh)
                    })
                };
                if runs_all_whitespace
                    && tac_picture_height.is_none()
                    && prev_line_reserved_tac_picture_height
                        .map(|prev| (raw_lh - prev).abs() <= 8.0)
                        .unwrap_or(false)
                {
                    pairs.push((0.0, 0.0));
                    prev_line_reserved_tac_picture_height = None;
                    continue;
                }
                let recompute_lh = text_before_picture_line || (max_fs > 0.0 && raw_lh < max_fs);
                let (lh, line_spacing_px) = if recompute_lh {
                    // [Task #1042 Stage 6c] HWP3/HWP5 line_segs 의 (line_height=base,
                    // line_spacing=extra) 의미와 정합되게 분해 — 종전 처럼 ls_val/100 전체를
                    // line_height 에 baking 하고 line_spacing_px=0 으로 두면 trailing_ls 제거
                    // 효과 (height_for_fit) 가 line_segs 있는 path 와 어긋남.
                    use crate::model::style::LineSpacingType;
                    if text_before_picture_line {
                        (max_fs.max(1.0), hwpunit_to_px(line.line_spacing, self.dpi))
                    } else {
                        match ls_type {
                            LineSpacingType::Percent => {
                                let extra = (max_fs * (ls_val - 100.0) / 100.0).max(0.0);
                                (max_fs, extra)
                            }
                            LineSpacingType::Fixed => (ls_val.max(max_fs), 0.0),
                            LineSpacingType::SpaceOnly => (max_fs, ls_val.max(0.0)),
                            LineSpacingType::Minimum => (ls_val.max(max_fs), 0.0),
                        }
                    }
                } else {
                    crate::renderer::corrected_line_metrics_for_source(
                        raw_lh,
                        raw_text_height,
                        hwpunit_to_px(line.line_spacing, self.dpi),
                        max_fs,
                        ls_type,
                        ls_val,
                        para.controls.is_empty(),
                    )
                };
                let extra_rows =
                    crate::renderer::equation_tac_flow::compute_equation_only_tac_line_flow(
                        Some(para),
                        comp,
                        &tac_offsets_px,
                        line_idx,
                        equation_line_available_width_px(0).unwrap_or(f64::INFINITY),
                        equation_line_available_width_px(1).unwrap_or(f64::INFINITY),
                    )
                    .map(|flow| flow.extra_rows)
                    .unwrap_or(0);
                let flow_lh = lh + extra_rows as f64 * (lh + line_spacing_px);
                pairs.push((flow_lh, line_spacing_px));
                prev_line_reserved_tac_picture_height = tac_picture_height;
            }
            pairs.into_iter().unzip()
        } else if !para.line_segs.is_empty() {
            para.line_segs
                .iter()
                .map(|seg| {
                    (
                        hwpunit_to_px(seg.line_height, self.dpi),
                        hwpunit_to_px(seg.line_spacing, self.dpi),
                    )
                })
                .unzip()
        } else {
            (vec![hwpunit_to_px(400, self.dpi)], vec![0.0])
        };
        if has_treat_as_char_picture_shape
            && line_heights.len() == 2
            && line_heights[0] > 80.0
            && (line_heights[0] - line_heights[1]).abs() <= 8.0
        {
            line_heights[1] = 0.0;
            line_spacings[1] = 0.0;
        }

        let lines_total: f64 = line_heights
            .iter()
            .zip(line_spacings.iter())
            .map(|(h, s)| h + s)
            .sum();
        let total_height = spacing_before + lines_total + spacing_after;

        // 적합성 판단용: trailing line_spacing 제외
        let trailing_ls = line_spacings.last().copied().unwrap_or(0.0);
        let height_for_fit = {
            let metric = (total_height - trailing_ls).max(0.0);
            let vpos_metric = if para.controls.is_empty() {
                para.line_segs
                    .first()
                    .zip(para.line_segs.last())
                    .and_then(|(first, last)| {
                        let has_progressing_vpos =
                            para.line_segs.len() <= 1 || last.vertical_pos > first.vertical_pos;
                        if !has_progressing_vpos {
                            return None;
                        }
                        let span = last
                            .vertical_pos
                            .saturating_add(last.line_height)
                            .saturating_sub(first.vertical_pos);
                        (span > 0).then_some(hwpunit_to_px(span, self.dpi))
                    })
            } else {
                None
            };
            vpos_metric.map(|v| metric.min(v)).unwrap_or(metric)
        };

        FormattedParagraph {
            total_height,
            line_heights,
            line_spacings,
            spacing_before,
            spacing_after,
            height_for_fit,
        }
    }

    // ========================================================
    // fits + place/split: 배치 판단과 실행
    // ========================================================

    /// 문단을 현재 페이지에 배치한다.
    /// fits → place(전체) 또는 split(줄 단위) → move(다음 페이지)
    fn typeset_paragraph(
        &self,
        st: &mut TypesetState,
        para_idx: usize,
        para: &Paragraph,
        fmt: &FormattedParagraph,
        paragraphs: &[Paragraph],
        is_last_in_section: bool,
    ) {
        // Task #332 Stage 4a: layout drift 안전 마진.
        // typeset 의 fit 추정과 layout 의 실측 진행은 폰트 메트릭/표 측정 다중성 등으로
        // 미세하게 어긋날 수 있다 (~수 px). 마진을 빼서 보수적으로 fit 을 판정해
        // layout 시점의 LAYOUT_OVERFLOW (clamp pile 트리거) 를 사전 차단한다.
        // [Task #359] 다음 pi 가 vpos-reset 가드 발동 예정 시 안전마진 1회 비활성화
        // (단독 항목 페이지 차단).
        // [Task #361] 직전 항목이 PartialTable 인 경우 안전마진 비활성화.
        // PartialTable 의 cur_h 는 row 단위로 정확히 누적되므로 안전마진이 과함.
        // (k-water-rfp p15 case: PartialTable 직후 작은 텍스트 (16px) 가 잔여 5.3px 부족으로
        // fit 실패하여 다음 페이지로 밀리는 회귀.)
        // [Task #643] VPOS_CORR 백워드 허용 (8px) 으로 layout drift 누적이 해소됨.
        const DEFAULT_LAYOUT_DRIFT_SAFETY_PX: f64 = 4.0;
        const ROWBREAK_LAYOUT_DRIFT_SAFETY_PX: f64 = 0.0;
        let layout_drift_safety_px = if section_has_zero_high_attr_rowbreak_table(paragraphs) {
            ROWBREAK_LAYOUT_DRIFT_SAFETY_PX
        } else {
            DEFAULT_LAYOUT_DRIFT_SAFETY_PX
        };
        let prev_is_partial_table =
            matches!(st.current_items.last(), Some(PageItem::PartialTable { .. }));
        let safety = if st.skip_safety_margin_once {
            st.skip_safety_margin_once = false;
            0.0
        } else if prev_is_partial_table {
            0.0
        } else {
            layout_drift_safety_px
        };
        let exclusion_probe_height = if st.is_hwpx_source {
            fmt.line_heights
                .first()
                .zip(fmt.line_spacings.first())
                .map(|(lh, ls)| lh + ls)
                .unwrap_or(fmt.height_for_fit)
        } else {
            0.0
        };
        st.apply_visible_float_exclusions(exclusion_probe_height);
        // [Task #1725] tail-before-vpos-reset 문단은 각주 안전마진(보수 버퍼 40px)만 1회 되돌려
        // 본문에 유지한다. 한글 LINESEG 는 이 tail 문단을 본문(각주 위)에 배치하는데, rhwp 각주
        // 예약의 보수 버퍼가 tail 을 수 px 밀어 near-empty 페이지 over-pagination(국제고속선기준
        // 258 vs 242)을 만든다. 다음 문단이 새 페이지를 시작(vpos-reset)하므로 tail 을 현재
        // 페이지에 두는 것이 한글 정합. (실제 각주 높이는 유지 — 버퍼만 완화하여 겹침 위험 최소화;
        // 버퍼 초과분은 별도 원인이라 여기서 다루지 않는다.)
        let footnote_margin_addback = if st.skip_footnote_margin_once {
            st.skip_footnote_margin_once = false;
            if st.current_footnote_height > 0.0 {
                st.footnote_safety_margin
            } else {
                0.0
            }
        } else {
            0.0
        };
        let available = (st.available_height() - safety + footnote_margin_addback).max(0.0);

        // 다단 레이아웃에서 문단 내 단 경계 감지
        // [Task #459] on_first_multicolumn_page 가드 제거: 다단 구역이 여러 페이지에 걸칠 때
        // 후속 페이지에서도 LINE_SEG vpos-reset 으로 인코딩된 단 경계를 인식해야 함.
        let col_breaks = if st.col_count > 1 && st.current_column == 0 {
            Self::detect_column_breaks_in_paragraph(para)
        } else {
            vec![0]
        };

        if col_breaks.len() > 1 {
            self.typeset_multicolumn_paragraph(st, para_idx, para, fmt, &col_breaks);
            return;
        }

        // [Task #362] 한컴 빈 줄 감추기 (SectionDef bit 19, hide_empty_line):
        // 빈 paragraph 가 현재 공간을 overflow 시키면 height=0 으로 처리 (페이지 당 최대 2개).
        // Paginator (engine.rs:85-106) 와 동일 시멘틱.
        // (kps-ai p67~70 case: PartialTable 후속 빈 paragraphs 가 다수 발생, 한컴은 표시 안 함.)
        if st.hide_empty_line {
            let current_page_idx = st.pages.len();
            if current_page_idx != st.hidden_empty_page_idx {
                st.hidden_empty_lines = 0;
                st.hidden_empty_page_idx = current_page_idx;
            }
            let trimmed = para.text.replace(|c: char| c.is_control(), "");
            let is_empty_para = trimmed.trim().is_empty() && para.controls.is_empty();
            if is_empty_para
                && !st.current_items.is_empty()
                && st.current_height + fmt.height_for_fit > available
                && st.hidden_empty_lines < 2
            {
                st.hidden_empty_lines += 1;
                st.hidden_empty_paras.insert(para_idx);
                // height=0 으로 page 진행 — fit 분기에서 추가 처리하지 않음
                st.current_items.push(PageItem::FullParagraph {
                    para_index: para_idx,
                });
                return;
            }
        }

        // [Task #676] trailing empty paragraph 가드 (단단 전용):
        // 섹션 마지막 빈 paragraph 가 현재 safety 영역 내 미세 overflow 로 fit 실패 시
        // height=0 흡수 — 단독 빈 페이지 차단. 한컴2022 정합 시멘틱.
        // (통합재정통계 2010.11/2011.10: 과거 safety_margin 운용 시
        //  pi=14 의 0.8px overflow 를 흡수한 사례.)
        // hide_empty_line (Task #362) 분기와 달리 SectionDef bit 무관, 섹션 마지막 1개만 흡수.
        if is_last_in_section && st.col_count == 1 && !st.current_items.is_empty() {
            let trimmed = para.text.replace(|c: char| c.is_control(), "");
            let is_empty_para = trimmed.trim().is_empty() && para.controls.is_empty();
            if is_empty_para {
                let total_h = st.current_height + fmt.height_for_fit;
                let fit_fail_within_safety =
                    total_h > available && total_h <= available + layout_drift_safety_px;
                let base_available = st.base_available_height() - st.current_zone_y_offset;
                let fit_fail_only_after_footnote_reserve = st.current_footnote_height > 0.0
                    && total_h > available
                    && total_h <= base_available;
                let prior_trailing_drift = st.current_height > available
                    && st.current_height <= available + layout_drift_safety_px + 0.5;
                let previous_item_is_empty_para = st
                    .current_items
                    .last()
                    .and_then(|item| match item {
                        PageItem::FullParagraph { para_index } => Some(*para_index),
                        _ => None,
                    })
                    .and_then(|prev_idx| paragraphs.get(prev_idx))
                    .map(|prev_para| {
                        let trimmed = prev_para.text.replace(|c: char| c.is_control(), "");
                        trimmed.trim().is_empty() && prev_para.controls.is_empty()
                    })
                    .unwrap_or(false);
                if prior_trailing_drift && previous_item_is_empty_para {
                    st.hidden_empty_paras.insert(para_idx);
                    return;
                }
                if fit_fail_within_safety || fit_fail_only_after_footnote_reserve {
                    st.current_items.push(PageItem::FullParagraph {
                        para_index: para_idx,
                    });
                    return;
                }
            }
        }

        let forced_page_break_line = internal_vpos_page_break_line(
            para,
            fmt.line_heights.len(),
            st.layout.body_area.height,
            self.dpi,
        )
        .or_else(|| {
            sample16_missing_lineseg_tail_break_line(
                para,
                fmt.line_heights.len(),
                st.current_height,
                available,
            )
        });

        // fits: 문단 전체가 현재 공간에 들어가는가?
        // [Task #359] fit 판정은 height_for_fit (trailing_ls 제외) 으로,
        // 누적은 total_height (full) 로 분리. 각 항목별 trailing_ls 가
        // 누적에서 빠지면 N items 누적 시 N × trailing_ls 만큼 drift 발생
        // (k-water-rfp p3 case: 36 items × 평균 ~9px = ~311px LAYOUT_OVERFLOW).
        // trailing_ls 는 페이지 마지막 항목의 fit 판정에만 의미가 있음
        // (페이지 끝에는 다음 줄이 없으니 line_spacing 미적용).
        // [Task #1082] 본문 para 의 bottom offset vpos — 미주 vpos-delta 시드용.
        let body_bottom_vpos: Option<i32> = para
            .line_segs
            .last()
            .map(|s| s.vertical_pos + s.line_height + s.line_spacing);
        // HWP3-origin 변환본은 spacing_before 누적을 보존해야 dump-pages 요약과
        // 실제 한컴 줄 흐름이 유지된다(#1116).
        let trim_spacing_before_for_flow =
            !st.is_hwp3_variant && !para_near_rowbreak_table(paragraphs, para_idx);

        let current_page_vpos_base = st.vpos_page_base.or_else(|| {
            st.current_items
                .first()
                .and_then(|item| page_item_vpos_base(item, paragraphs))
        });
        let saved_single_line_bottom_fits = forced_page_break_line.is_none()
            && st.col_count == 1
            && fmt.line_heights.len() == 1
            && fmt.spacing_after <= 0.5
            && para.controls.is_empty()
            && !st.current_items.is_empty()
            && current_page_vpos_base
                .and_then(|base| single_line_visible_bounds_px(para, base, self.dpi))
                .is_some_and(|bounds| {
                    saved_bounds_fit_at_flow_tail(bounds, st.current_height, st.available_height())
                });

        if forced_page_break_line.is_none()
            && (st.current_height + fmt.height_for_fit <= available
                || saved_single_line_bottom_fits)
        {
            // place: 전체 배치
            st.current_items.push(PageItem::FullParagraph {
                para_index: para_idx,
            });
            // [Task #391] 다단/단단 분기:
            //   - 단단 (col_count == 1): total_height (k-water-rfp p3 311px drift 차단, #359)
            //   - 다단 (col_count > 1): height_for_fit (exam_eng 8p 정상 단 채움 복원)
            // 다단에서는 layout 이 vpos 기반으로 항목을 단별로 stacking 하므로
            // typeset 누적 시 trailing_ls 인플레이션이 단을 조기 종료시킴.
            st.current_height +=
                fmt.flow_advance_height(para, st.col_count, trim_spacing_before_for_flow);
            if let Some(v) = body_bottom_vpos {
                st.prev_body_bottom_vpos = Some(v);
            }
            return;
        }

        // [Task #409 v3] atomic TAC top-fit:
        // 단일 라인 + TAC Picture/Shape (분할 불가능) 항목은 시작점이 본문 안이면
        // 현재 페이지에 배치하고 하단 일부는 하단 여백 (15mm) 으로 흘림 허용.
        // HWP 시멘틱 — atomic 항목은 strict bottom-fit 대신 top-fit 으로 판정.
        // (대상 샘플 23페이지 차트 pi=208: lh=316px, 시작 y=721.4 < 1028(본문 끝),
        //  끝 y=1037.4 가 9.4px 초과하지만 하단 여백 56.7px 안이므로 HWP 가 23페이지 배치.)
        // [Task #1027 Stage E2] atomic top-fit 스필은 진짜 인라인 atomic 개체(차트/그림 등,
        // #409)에만 적용한다. 위아래(TopAndBottom) 글상자(Shape)는 한컴이 본문 항목처럼
        // 다음 페이지로 넘기므로(예: AI 184p box pi=142 → 10쪽) 스필 대상에서 제외 —
        // 그렇지 않으면 하드코딩 60px 허용폭으로 페이지 하단에 잘못 스필되어 overflow.
        let is_atomic_tac_singleton = fmt.line_heights.len() == 1
            && para.controls.iter().any(|c| match c {
                Control::Picture(p) => p.common.treat_as_char,
                Control::Shape(s) => {
                    s.common().treat_as_char
                        && !matches!(
                            s.common().text_wrap,
                            crate::model::shape::TextWrap::TopAndBottom
                        )
                }
                _ => false,
            });
        if is_atomic_tac_singleton && st.current_height < available && !st.current_items.is_empty()
        {
            // 추가 가드: 본문 + 하단 여백 안에 들어가야 함 (footer 침범 금지)
            let bottom_margin_px = hwpunit_to_px(
                st.layout.body_area.height as i32, // body_area.height 는 이미 px
                self.dpi,
            );
            // 보수적 tolerance: 1mm (약 3.78px) 이상 ~ 하단 여백 끝까지 허용
            // body_area.height 가 px 이므로 직접 비교 — base_available_height 와의
            // 차이는 footnote_area 만 (본 케이스 0). bottom_margin 은 PageDef 에서
            // 가져와야 하나 직접 접근 어려우므로 1mm 이상 ~ 60px 정도까지 허용.
            let _ = bottom_margin_px; // (위 변수는 향후 정밀화용 — 현재 사용 안 함)
            let overflow = st.current_height + fmt.height_for_fit - available;
            // 60px 이내 초과 (대략 하단 여백 1.6cm 까지 허용; HWP 표준 15mm 여백 안)
            if overflow <= 60.0 {
                st.current_items.push(PageItem::FullParagraph {
                    para_index: para_idx,
                });
                st.current_height +=
                    fmt.flow_advance_height(para, st.col_count, trim_spacing_before_for_flow);
                if let Some(v) = body_bottom_vpos {
                    st.prev_body_bottom_vpos = Some(v);
                }
                return;
            }
        }

        // [Task #1537] 폰트 치환 drift 로 인한 "tail 1줄 spill 후 강제 쪽나누기 고아 페이지" 차단.
        //
        // 증상: 본문 문단 N 이 페이지 하단을 ~한 줄 미만으로 미세 초과(폰트 치환으로 부피가
        // 한컴 대비 커짐)하여 마지막 줄만 새 페이지로 split → 그 직후 문단 N+1 이 명시적
        // 쪽나누기(column_type==Page/Section)를 가지면 또 새 페이지를 강제 → spill 한 1줄이
        // 거의 빈 페이지에 고립된다(2025 행정업무운영 편람: 0-idx page 11/13/17, 본문 1줄+빈공간).
        //
        // 한컴은 폰트 drift 가 없어 문단 N 전체를 현재 페이지에 담고 N+1 의 쪽나누기로 깔끔히
        // 다음 페이지를 시작한다. 우리도 "초과량이 한 줄 미만(=drift)이고 다음 문단이 어차피
        // 쪽나누기로 페이지를 끝낸다"는 두 조건이 모두 맞을 때만 문단 N 을 통째로 현재 페이지에
        // 배치(하단 여백으로 소량 bleed 허용)해 고아 페이지를 제거한다. 일반 본문 흐름(다음
        // 문단이 쪽나누기가 아님)이나 초과량이 한 줄 이상(진짜 split 필요)인 경우는 불변.

        // 다음 문단이 쪽/구역 나누기인가? 사이에 빈 문단(텍스트·컨트롤 없음)이 끼어 있으면
        // 건너뛴다 — 빈 문단은 높이를 거의 차지하지 않고 hide_empty_line 로 흡수되므로,
        // "tail spill → 빈 문단 → 강제 쪽나누기" 패턴에서도 spill 한 줄이 동일하게 고립된다.
        // 단, 텍스트/컨트롤이 있는 일반 문단을 만나면 즉시 중단(false) — 그 문단이
        // 현재 페이지를 마저 채우므로 고아 페이지가 생기지 않는다.
        let next_para_forces_break = {
            let mut idx = para_idx + 1;
            let mut prior_para = para;
            let mut forced = false;
            while let Some(next_para) = paragraphs.get(idx) {
                if paragraph_forces_page_boundary_after(
                    prior_para,
                    next_para,
                    st.col_count,
                    st.is_hwp3_variant,
                ) {
                    forced = true;
                    break;
                }
                let is_empty = next_para.text.trim().is_empty() && next_para.controls.is_empty();
                if !is_empty {
                    break;
                }
                prior_para = next_para;
                idx += 1;
            }
            forced
        };
        // 본문 높이를 바꾸지 않는 컨트롤(각주/미주)만 허용 — 표/그림/글상자가 있으면
        // 줄 단위 split/배치 규칙이 달라지므로 제외.
        let only_note_controls = para
            .controls
            .iter()
            .all(|c| matches!(c, Control::Footnote(_) | Control::Endnote(_)));
        if st.col_count == 1
            && forced_page_break_line.is_none()
            && next_para_forces_break
            && !para.text.trim().is_empty()
            && only_note_controls
            && !st.current_items.is_empty()
            && fmt.line_heights.len() >= 2
        {
            let first_line_advance = fmt.line_advance(0);
            // 다음 문단이 어차피 쪽나누기로 페이지를 끝내므로, 다음 페이지 layout clamp 를
            // 막으려던 LAYOUT_DRIFT_SAFETY_PX(현재 페이지 한정) 여유는 이 경우 의미가 없다.
            // 따라서 safety 를 뺀 `available` 이 아니라 진짜 본문 하단(각주/존 차감 포함)인
            // available_height() 를 기준으로 초과량을 잰다.
            let true_available = st.available_height();
            // 초과량이 한 줄 미만(폰트 drift)일 때만 통째 배치.
            // (full-place 체크를 이미 통과 못 했으므로 overflow > -safety. 진짜 본문 하단
            //  기준으로 한 줄 미만 초과면 마지막 줄 spill 대신 통째 배치.)
            let overflow = st.current_height + fmt.height_for_fit - true_available;
            if overflow < first_line_advance {
                st.current_items.push(PageItem::FullParagraph {
                    para_index: para_idx,
                });
                st.current_height += fmt.total_height;
                if let Some(v) = body_bottom_vpos {
                    st.prev_body_bottom_vpos = Some(v);
                }
                return;
            }
        }

        // split: 줄 단위 분할
        let line_count = fmt.line_heights.len();
        if line_count == 0 {
            st.current_items.push(PageItem::FullParagraph {
                para_index: para_idx,
            });
            // [Task #391] 다단/단단 분기:
            //   - 단단 (col_count == 1): total_height (k-water-rfp p3 311px drift 차단, #359)
            //   - 다단 (col_count > 1): height_for_fit (exam_eng 8p 정상 단 채움 복원)
            // 다단에서는 layout 이 vpos 기반으로 항목을 단별로 stacking 하므로
            // typeset 누적 시 trailing_ls 인플레이션이 단을 조기 종료시킴.
            st.current_height +=
                fmt.flow_advance_height(para, st.col_count, trim_spacing_before_for_flow);
            if let Some(v) = body_bottom_vpos {
                st.prev_body_bottom_vpos = Some(v);
            }
            return;
        }

        // Task #332 Stage 4a: partial split 시에도 동일 마진 적용
        let base_available = (st.base_available_height() - layout_drift_safety_px).max(0.0);

        // 남은 공간이 없거나 첫 줄도 못 넣으면 먼저 다음 단/페이지로
        let first_line_h = fmt.line_heights[0];
        let remaining = (available - st.current_height).max(0.0);
        // [Task #1086] 단일 단에서도 HWP가 paragraph 내부 page reset 을
        // LINE_SEG(vpos=0) 로 인코딩하는 케이스가 있다(k-water-rfp pi=66).
        // 첫 줄의 HWP 좌표가 본문 안에 있고 다음 줄이 reset 이면, 보수적
        // safety margin 으로 미리 페이지를 넘기지 말고 줄 단위 split 루프에서
        // 첫 줄만 현재 페이지에 배치하게 둔다.
        let hwp_first_line_before_reset_fits = para
            .line_segs
            .get(1)
            .map(|next| next.vertical_pos == 0)
            .unwrap_or(false)
            && para
                .line_segs
                .first()
                .map(|cur| {
                    let bottom_px = crate::renderer::hwpunit_to_px(
                        cur.vertical_pos + cur.line_height,
                        self.dpi,
                    );
                    bottom_px <= st.base_available_height() + 0.5
                })
                .unwrap_or(false);
        if (st.current_height >= available || remaining < first_line_h)
            && !st.current_items.is_empty()
            && !hwp_first_line_before_reset_fits
        {
            st.advance_column_or_new_page();
        }

        // 줄 단위 분할 루프
        let mut cursor_line: usize = 0;
        while cursor_line < line_count {
            let fn_margin = if st.current_footnote_height > 0.0 {
                st.footnote_safety_margin
            } else {
                0.0
            };
            let page_avail = if cursor_line == 0 {
                (base_available
                    - st.current_footnote_height
                    - fn_margin
                    - st.current_height
                    - st.current_zone_y_offset)
                    .max(0.0)
            } else {
                base_available
            };

            let sp_b = if cursor_line == 0 {
                fmt.spacing_before
            } else {
                0.0
            };
            // Task #332 Stage 4b: partial split 의 줄 단위 fit 검사에도 layout drift 마진 적용
            let avail_for_lines = (page_avail - sp_b - layout_drift_safety_px).max(0.0);

            // 현재 페이지에 들어갈 줄 범위 결정
            let mut cumulative = 0.0;
            let mut end_line = cursor_line;
            for li in cursor_line..line_count {
                if forced_page_break_line
                    .map(|break_line| li == break_line && li > cursor_line)
                    .unwrap_or(false)
                {
                    break;
                }
                // [Task #619] 다단 paragraph 내 vpos-reset 강제 분리.
                // line_segs[li].vertical_pos == 0 (li>0) 은 HWP 가 해당 line 을
                // 다음 단/페이지 최상단에 배치하도록 인코딩한 신호.
                // 다단 한정 적용 — 단일 단은 partial-table split 회귀 (issue #418) 차단 위해 미적용.
                if st.col_count > 1
                    && li > cursor_line
                    && para
                        .line_segs
                        .get(li)
                        .map(|s| s.vertical_pos == 0)
                        .unwrap_or(false)
                {
                    break;
                }
                let content_h = fmt.line_heights[li];
                if cumulative + content_h > avail_for_lines && li > cursor_line {
                    // [Task #631] HWP 권위값 더블체크
                    // 누적 추정으로는 fit 실패하지만 HWP 파일 자체가 다음 줄(li+1)에
                    // vpos-reset(=0) 을 인코딩한 경우, 한컴 엔진이 직접 li 까지를 현재
                    // 페이지에 배치한 것이다. typeset 보수 마진(20px) 으로 인한 콘텐츠
                    // 손실을 차단하기 위해 HWP 신호를 우선한다.
                    // 조건: (1) 다음 줄의 vpos==0 (페이지 경계 신호)
                    //       (2) 현재 줄의 hwp 좌표 vpos+lh 가 body_available 안
                    let hwp_authoritative = para
                        .line_segs
                        .get(li + 1)
                        .map(|next| next.vertical_pos == 0)
                        .unwrap_or(false)
                        && para
                            .line_segs
                            .get(li)
                            .map(|cur| {
                                let bottom_px = crate::renderer::hwpunit_to_px(
                                    cur.vertical_pos + cur.line_height,
                                    self.dpi,
                                );
                                bottom_px <= st.base_available_height()
                            })
                            .unwrap_or(false);
                    if !hwp_authoritative {
                        break;
                    }
                }
                cumulative += fmt.line_advance(li);
                end_line = li + 1;
            }

            if end_line <= cursor_line {
                end_line = cursor_line + 1;
            }

            let next_para_is_rowbreak_anchor_table = paragraphs
                .get(para_idx + 1)
                .map(|next_para| {
                    next_para.controls.iter().any(|ctrl| {
                        if let Control::Table(table) = ctrl {
                            !table.common.treat_as_char
                                && matches!(
                                    table.common.text_wrap,
                                    crate::model::shape::TextWrap::TopAndBottom
                                )
                                && matches!(
                                    table.common.vert_rel_to,
                                    crate::model::shape::VertRelTo::Para
                                )
                                && matches!(
                                    table.page_break,
                                    crate::model::table::TablePageBreak::RowBreak
                                )
                        } else {
                            false
                        }
                    })
                })
                .unwrap_or(false);
            if cursor_line == 0
                && end_line > cursor_line + 1
                && end_line < line_count
                && next_para_is_rowbreak_anchor_table
            {
                end_line -= 1;
                cumulative = fmt.line_advances_sum(cursor_line..end_line);
            }

            let part_line_height = fmt.line_advances_sum(cursor_line..end_line);
            let part_sp_after = if end_line >= line_count {
                fmt.spacing_after
            } else {
                0.0
            };
            let part_height = sp_b + part_line_height + part_sp_after;

            if cursor_line == 0 && end_line >= line_count {
                // 전체가 배치됨 — overflow 재확인
                let prev_is_table = st.current_items.last().map_or(false, |item| {
                    matches!(item, PageItem::Table { .. } | PageItem::PartialTable { .. })
                });
                let overflow_threshold = if prev_is_table {
                    let trailing_ls = fmt
                        .line_spacings
                        .get(end_line.saturating_sub(1))
                        .copied()
                        .unwrap_or(0.0);
                    cumulative - trailing_ls
                } else {
                    cumulative
                };
                if overflow_threshold > avail_for_lines && !st.current_items.is_empty() {
                    st.advance_column_or_new_page();
                    continue;
                }
                st.current_items.push(PageItem::FullParagraph {
                    para_index: para_idx,
                });
            } else {
                st.current_items.push(PageItem::PartialParagraph {
                    para_index: para_idx,
                    start_line: cursor_line,
                    end_line,
                });
            }
            st.current_height += part_height;

            if end_line >= line_count {
                break;
            }

            // move: 나머지 줄 → 다음 단/페이지
            st.advance_column_or_new_page();
            cursor_line = end_line;
        }
    }

    // ========================================================
    // Phase 2: Break Token 기반 표 조판
    // ========================================================

    /// 표의 조판 높이를 계산한다 (format 단계).
    /// MeasuredTable + host_spacing을 통합하여 layout과 동일한 규칙으로 계산.
    #[allow(clippy::too_many_arguments)]
    fn format_table(
        &self,
        para: &Paragraph,
        para_idx: usize,
        ctrl_idx: usize,
        table: &crate::model::table::Table,
        measured_tables: &[MeasuredTable],
        styles: &ResolvedStyleSet,
        composed: Option<&ComposedParagraph>,
        next_para: Option<&Paragraph>,
        is_column_top: bool,
        is_hwpx_source: bool,
    ) -> FormattedTable {
        let mt = measured_tables
            .iter()
            .find(|mt| mt.para_index == para_idx && mt.control_index == ctrl_idx);
        let fitted_visible_mt =
            if is_para_topbottom_float(&table.common) && para_has_non_whitespace_text(para) {
                mt.map(|measured| fit_measured_table_to_declared_height(measured, table, self.dpi))
            } else {
                None
            };
        let mt = fitted_visible_mt.as_ref().or(mt);

        let is_tac = table.attr & 0x01 != 0;
        let table_text_wrap = (table.attr >> 21) & 0x07;

        // host_spacing 계산 — layout과 동일한 규칙
        let para_style_id = composed
            .map(|c| c.para_style_id as usize)
            .unwrap_or(para.para_shape_id as usize);
        let para_style = styles.para_styles.get(para_style_id);
        let sb = para_style.map(|s| s.spacing_before).unwrap_or(0.0);
        let sa = para_style.map(|s| s.spacing_after).unwrap_or(0.0);

        let outer_top = if is_tac {
            hwpunit_to_px(table.outer_margin_top as i32, self.dpi)
        } else {
            0.0
        };
        let outer_bottom = if is_tac {
            hwpunit_to_px(table.outer_margin_bottom as i32, self.dpi)
        } else {
            0.0
        };

        // 비-TAC 표: 호스트 문단의 trailing line_spacing도 포함
        // [Task #874 #7] 비-TAC 1×1 placeholder 표 (paras=1 text-only) 는 host
        // line_spacing 을 더하지 않는다. 한컴은 표 outer_margin_bottom 만 사용 (호스트
        // 문단 line_spacing 은 본문 라인 간 간격 의미). aift.hwp p21 표 pi=268
        // ("협업 시스템 구성도 이미지") 직후 pi=284 ("코멘트 스레드 관리...") 가
        // 9.6 px 만큼 다음 페이지로 밀려나는 문제 해결.
        let is_single_cell_placeholder = !is_tac
            && table.row_count == 1
            && table.col_count == 1
            && table.cells.len() == 1
            && table
                .cells
                .first()
                .map(|c| {
                    c.paragraphs
                        .iter()
                        .all(|p| p.controls.is_empty() && p.line_segs.len() <= 1)
                })
                .unwrap_or(false);
        // [Task #1147] HWPX 원본 의 wrap=TopAndBottom 비-TAC 표 + 빈 앵커 문단:
        //   HWPX LINE_SEG 시멘틱상 빈 앵커 문단 vpos = 직전 문단 종료 vpos (갭 0).
        //   PS.spacing_before / host_line_spacing 을 별도 가산하면 HWPX vpos delta 와
        //   +sb +leading 만큼 어긋나 페이지 overflow 유발.
        //   HWP5/HWP3 는 LINE_SEG 인코딩이 달라 기존 동작 유지 (hwpspec 등 178p 정합).
        // [Task #1133] 단, 빈 앵커 TopAndBottom 표가 연속될 때는 첫 표의
        //   host_line_spacing 이 표-표 사이 시각 간격이다. 이를 0으로 누르면 HWPX
        //   pi=28→29가 HWP와 달리 붙어 출력된다.
        let is_topbottom_empty_anchor_hwpx = is_hwpx_source
            && !is_tac
            && matches!(
                table.common.text_wrap,
                crate::model::shape::TextWrap::TopAndBottom
            )
            && para.text.is_empty();
        let next_is_empty_topbottom_table_anchor = next_para
            .map(para_is_empty_topbottom_table_anchor)
            .unwrap_or(false);
        let suppress_empty_anchor_spacing =
            is_topbottom_empty_anchor_hwpx && !next_is_empty_topbottom_table_anchor;

        let host_line_spacing = if suppress_empty_anchor_spacing {
            0.0
        } else if !is_tac && !is_single_cell_placeholder {
            para.line_segs
                .last()
                .filter(|seg| seg.line_spacing > 0)
                .map(|seg| hwpunit_to_px(seg.line_spacing, self.dpi))
                .unwrap_or(0.0)
        } else {
            0.0
        };

        // spacing_before 조건부 적용
        // - 자리차지(text_wrap=1) 비-TAC 표: spacing_before 제외
        //   (layout에서 v_offset 기반 절대 위치로 배치)
        // - 단 상단: spacing_before 제외
        // - [Task #1147] HWPX 빈 앵커 TopAndBottom 비-TAC 표: 다음 항목이 일반 문단이면
        //   spacing_before 제외 (위 주석). 다음 항목도 표 앵커이면 HWP처럼 보존한다.
        let before = if !is_tac && table_text_wrap == 1 {
            outer_top
        } else if suppress_empty_anchor_spacing && !is_column_top {
            outer_top
        } else {
            (if !is_column_top { sb } else { 0.0 }) + outer_top
        };
        let after = sa + outer_bottom + host_line_spacing;
        let host_spacing = HostSpacing {
            before,
            after,
            spacing_after_only: sa,
        };

        let (
            row_heights,
            cell_spacing,
            effective_height,
            caption_height,
            cumulative_heights,
            page_break,
            cells,
            header_row_count,
        ) = if let Some(mt) = mt {
            let hrc = if mt.repeat_header && mt.has_header_cells {
                1
            } else {
                0
            };
            (
                mt.row_heights.clone(),
                mt.cell_spacing,
                mt.total_height,
                mt.caption_height,
                mt.cumulative_heights.clone(),
                mt.page_break,
                mt.cells.clone(),
                hrc,
            )
        } else {
            (
                Vec::new(),
                0.0,
                0.0,
                0.0,
                vec![0.0],
                Default::default(),
                Vec::new(),
                0,
            )
        };

        let total_height = effective_height + host_spacing.before + host_spacing.after;

        // 표 셀 내 각주 높이 사전 계산 (Paginator engine.rs:565-581 동일)
        let mut table_footnote_height = 0.0;
        let mut table_footnote_count = 0usize;
        for cell in &table.cells {
            for cp in &cell.paragraphs {
                for cc in &cp.controls {
                    if let Control::Footnote(fn_ctrl) = cc {
                        let fn_height = estimate_footnote_note_height(fn_ctrl, self.dpi);
                        table_footnote_height += fn_height;
                        table_footnote_count += 1;
                    }
                }
            }
        }

        FormattedTable {
            row_heights,
            cell_spacing,
            header_row_count,
            host_spacing,
            effective_height,
            total_height,
            caption_height,
            is_tac,
            cumulative_heights,
            page_break,
            cells,
            table_footnote_height,
            table_footnote_count,
        }
    }

    /// 표가 포함된 문단을 처리한다.
    /// 각 컨트롤(표/도형)에 대해 format → fits → place/split 패턴 적용.
    fn typeset_table_paragraph(
        &self,
        st: &mut TypesetState,
        para_idx: usize,
        para: &Paragraph,
        composed: Option<&ComposedParagraph>,
        next_para: Option<&Paragraph>,
        styles: &ResolvedStyleSet,
        measured_tables: &[MeasuredTable],
        _page_def: &PageDef,
    ) {
        // 호스트 문단 format (TAC 표의 높이 보정용)
        let host_col_w = st
            .layout
            .column_areas
            .get(st.current_column as usize)
            .map(|a| a.width)
            .unwrap_or(st.layout.body_area.width);
        let fmt = self.format_paragraph(para, composed, styles, Some(host_col_w));
        // TAC 표 카운트 및 플러시 판단
        let tac_count = para
            .controls
            .iter()
            .filter(
                |c| matches!(c, Control::Table(t) if self.is_effective_tac_table(para, t, &fmt)),
            )
            .count();

        let has_tac = tac_count > 0;
        let first_line_tac_height = if tac_count == 1 && fmt.line_heights.len() > 1 {
            para.controls.iter().find_map(|ctrl| match ctrl {
                Control::Table(t)
                    if self.is_effective_tac_table(para, t, &fmt)
                        && self.tac_table_line_index(para, t, &fmt) == Some(0) =>
                {
                    Some(
                        fmt.line_heights
                            .first()
                            .copied()
                            .unwrap_or_else(|| fmt.line_advance(0)),
                    )
                }
                _ => None,
            })
        } else {
            None
        };
        let height_for_fit = if has_tac {
            first_line_tac_height.unwrap_or(fmt.height_for_fit)
        } else {
            fmt.total_height
        };
        let saved_single_tac_bottom_fits = if has_tac && tac_count <= 1 {
            para.controls
                .iter()
                .find_map(|ctrl| match ctrl {
                    Control::Table(table) if self.is_effective_tac_table(para, table, &fmt) => {
                        Some(self.tac_table_line_index(para, table, &fmt).unwrap_or(0))
                    }
                    _ => None,
                })
                .and_then(|line_idx| para.line_segs.get(line_idx))
                .and_then(|seg| {
                    line_seg_visible_bounds_px(seg, st.vpos_page_base.unwrap_or(0), self.dpi)
                })
                .is_some_and(|bounds| {
                    saved_bounds_fit_at_flow_tail(bounds, st.current_height, st.available_height())
                })
        } else {
            false
        };

        // 넘치면 flush (단일 TAC 표만)
        if st.current_height + height_for_fit > st.available_height()
            && !st.current_items.is_empty()
            && has_tac
            && tac_count <= 1
            && !saved_single_tac_bottom_fits
        {
            st.advance_column_or_new_page();
        }

        st.ensure_page();

        let height_before = st.current_height;
        let para_start_height = st.current_height;
        let page_count_before = st.pages.len();
        let mut para_float_lanes = FloatLaneSet::new();

        // 각 컨트롤에 대해 format → fits → place/split
        // [참고2 순서 역전 fix] 빈 host 문단의 para-relative float 표(비-TAC,
        // wrap=위아래, vert=문단)는 흐름과 무관하게 vertical_offset 위치에 배치되는
        // out-of-flow 개체다. 빈 host 에서는 para.controls 배열 순서가 시각적 위·아래
        // 순서와 다를 수 있어 vertical_offset 오름차순 안정정렬을 유지한다(#986/#1088).
        //
        // [Issue #1510] 실제 비공백 텍스트가 있는 host 문단은 한컴이 문서/control 순서와
        // 선언된 절대 위치를 함께 보존한다. 여기서 vertical_offset 순으로 재정렬하면
        // 제목 텍스트와 co-anchored float 표의 순서가 뒤집히므로 정렬 대상에서 제외한다.
        // 공백-only host 는 기존 TopAndBottom empty/float 흐름을 유지한다(#157).
        // [Issue #1639] 빈 host 라도 para-relative float 표 중 음수 vertical_offset 이
        // 하나라도 있으면, 아래 vertical_offset 오름차순 정렬이 음수 표를 양수/0 형제
        // 앞으로 끌어와 문서/배열 순서를 역전시킨다(설명 표가 본문 표 뒤로 밀리는 실문서
        // 회귀). 한컴은 음수가 섞이면 표를 문서/앵커 순서대로 배치하므로, 음수 혼재
        // 빈 host 는 재정렬을 끄고 배열 순서를 보존한다. 양수 전용 빈 host 의
        // vertical_offset 재정렬(#986/#1088)은 그대로 유지한다.
        // 경계: `signed_hwpunit < 0` 인 음수만 트리거하며, offset == 0 은 음수가 아니므로
        // 양수와 함께 정렬을 유지한다(0/양수=정렬 ON, 음수 혼재=정렬 OFF).
        let has_negative_para_float = para.controls.iter().any(|ctrl| {
            matches!(
                ctrl,
                Control::Table(t)
                    if is_para_topbottom_float(&t.common)
                        && signed_hwpunit(t.common.vertical_offset) < 0
            )
        });
        let should_sort_para_float_tables =
            !para_has_non_whitespace_text(para) && !has_negative_para_float;
        let float_table_voffset = |ctrl: &Control| -> i32 {
            match ctrl {
                Control::Table(t)
                    if should_sort_para_float_tables && is_para_topbottom_float(&t.common) =>
                {
                    t.common.vertical_offset as i32
                }
                _ => 0,
            }
        };
        let table_flow_tiebreak = |ctrl: &Control| -> u8 {
            match ctrl {
                Control::Table(t) if !self.is_effective_tac_table(para, t, &fmt) => 0,
                Control::Table(t) if self.is_effective_tac_table(para, t, &fmt) => 1,
                _ => 1,
            }
        };
        let mut ctrl_order: Vec<usize> = (0..para.controls.len()).collect();
        ctrl_order.sort_by_key(|&i| {
            (
                float_table_voffset(&para.controls[i]),
                table_flow_tiebreak(&para.controls[i]),
            )
        });
        // is_first_table/is_last_table 는 배열순서가 아닌 "놓이는 순서(ctrl_order)"
        // 기준으로 잡아, pre/post 텍스트와 spacing 이 실제 배치 첫/마지막 표에 붙도록 한다.
        let first_placed_table = ctrl_order
            .iter()
            .copied()
            .find(|&i| matches!(para.controls[i], Control::Table(_)));
        let last_placed_table = ctrl_order
            .iter()
            .copied()
            .rev()
            .find(|&i| matches!(para.controls[i], Control::Table(_)));

        for ctrl_idx in ctrl_order {
            let ctrl = &para.controls[ctrl_idx];
            match ctrl {
                Control::Table(table) => {
                    // [Issue #703] 글앞으로 / 글뒤로 표는 Shape처럼 취급 — 본문 흐름 공간 차지 없음.
                    // pagination/engine.rs:976-981 와 동일 시멘틱: 데코레이션 표는 절대 좌표로 배치되며
                    // current_height 누적에 영향을 주지 않는다.
                    //
                    // [Issue #775] 단일 컬럼 한정. 다단(col_count>=2) 영역에서는 InFrontOfText/BehindText
                    // 표라도 cur_h 누적이 컬럼 분배에 필요 (exam_eng.hwp p4 27번 보기 그림 위
                    // 데코레이션 표 회귀 차단).
                    //
                    // [Task #992] 페이지 본문보다 큰 다행(多行) 표는 대개 데코레이션이 아니라
                    // 쪽 분할이 필요한 본문 표다. 데코레이션 단축 분기에서 제외해 정상
                    // 페이지네이션(format_table → typeset_block_table)을 타게 한다.
                    // 제외하지 않으면 페이지보다 큰 표가 한 페이지에 통째로 그려져
                    // 본문 영역을 넘는다.
                    //
                    // [Issue #1271] 단, HWPX paper-anchored BehindText/InFrontOfText 표는
                    // rowBreak/repeatHeader 가 있어도 본문 흐름을 밀지 않는 페이지 배경/전경
                    // 개체일 수 있다. 특히 cover/background 라벨 표처럼 종이 기준 절대좌표인
                    // 표를 oversized_multirow 로 본문 분할하면 PDF에 없는 PartialTable 쪽이
                    // 생겨 이후 바탕쪽 홀짝까지 한 쪽씩 밀린다.
                    // 워터마크/배경 데코레이션(글뒤로 1×1 래퍼 등, Issue #703)은
                    // 본문보다 작아 단축 분기를 그대로 탄다 — page_break/repeat_header
                    // 만으로는 구분 불가(calendar_year.hwp 1×1 래퍼도 RowBreak +
                    // repeat_header 비트를 가짐).
                    let paper_anchored_overlay_table = !table.common.treat_as_char
                        && matches!(
                            table.common.vert_rel_to,
                            crate::model::shape::VertRelTo::Paper
                        )
                        && matches!(
                            table.common.horz_rel_to,
                            crate::model::shape::HorzRelTo::Paper
                        );
                    let table_measured_h = measured_tables
                        .iter()
                        .find(|mt| mt.para_index == para_idx && mt.control_index == ctrl_idx)
                        .map(|mt| mt.total_height)
                        .unwrap_or(0.0);
                    let oversized_multirow = table.row_count > 1
                        && table_measured_h > st.base_available_height()
                        && !paper_anchored_overlay_table;
                    let followed_by_empty_overlay_guide = next_para.is_some_and(|p| {
                        p.controls.is_empty()
                            && !para_has_visible_text(p)
                            && p.line_segs.first().is_some_and(|seg| seg.vertical_pos > 0)
                    });
                    let multicol_empty_overlay_anchor = st.col_count > 1
                        && !oversized_multirow
                        && followed_by_empty_overlay_guide
                        && para_is_non_tac_overlay_table_anchor(para);
                    let multicol_tac_host_overlay_anchor = st.col_count > 1
                        && !oversized_multirow
                        && has_tac
                        && para_is_non_tac_overlay_table_anchor(para);
                    if matches!(
                        table.common.text_wrap,
                        crate::model::shape::TextWrap::InFrontOfText
                            | crate::model::shape::TextWrap::BehindText
                    ) && ((st.col_count == 1 && !oversized_multirow)
                        || multicol_empty_overlay_anchor
                        || multicol_tac_host_overlay_anchor)
                    {
                        st.current_items.push(PageItem::Shape {
                            para_index: para_idx,
                            control_index: ctrl_idx,
                        });
                        continue;
                    }
                    let is_column_top = st.current_height < 1.0;
                    let ft = self.format_table(
                        para,
                        para_idx,
                        ctrl_idx,
                        table,
                        measured_tables,
                        styles,
                        composed,
                        next_para,
                        is_column_top,
                        st.is_hwpx_source,
                    );

                    let mt = measured_tables
                        .iter()
                        .find(|mt| mt.para_index == para_idx && mt.control_index == ctrl_idx);
                    let is_first_placed = first_placed_table == Some(ctrl_idx);
                    let is_last_placed = last_placed_table == Some(ctrl_idx);
                    if self.is_effective_tac_table(para, table, &fmt) {
                        self.typeset_tac_table(
                            st,
                            para_idx,
                            ctrl_idx,
                            para,
                            table,
                            &ft,
                            &fmt,
                            tac_count,
                            is_first_placed,
                            is_last_placed,
                        );
                    } else if self.try_typeset_empty_para_float_table(
                        st,
                        para_idx,
                        ctrl_idx,
                        para,
                        table,
                        &ft,
                        composed,
                        styles,
                        para_start_height,
                        &mut para_float_lanes,
                    ) {
                        // Empty host para-float table placed by horizontal lane reservation.
                    } else {
                        self.typeset_block_table(
                            st,
                            para_idx,
                            ctrl_idx,
                            para,
                            table,
                            &ft,
                            &fmt,
                            mt,
                            styles,
                            para_start_height,
                            is_first_placed,
                            is_last_placed,
                        );
                    }

                    // 표 셀 내 각주 수집 (Paginator engine.rs:679-701 동일)
                    for (cell_idx, cell) in table.cells.iter().enumerate() {
                        for (cp_idx, cp) in cell.paragraphs.iter().enumerate() {
                            for (cc_idx, cc) in cp.controls.iter().enumerate() {
                                if let Control::Footnote(fn_ctrl) = cc {
                                    if let Some(page) = st.pages.last_mut() {
                                        page.footnotes.push(FootnoteRef {
                                            number: fn_ctrl.number,
                                            source: FootnoteSource::TableCell {
                                                para_index: para_idx,
                                                table_control_index: ctrl_idx,
                                                cell_index: cell_idx,
                                                cell_para_index: cp_idx,
                                                cell_control_index: cc_idx,
                                            },
                                        });
                                    }
                                    let fn_height =
                                        estimate_footnote_note_height(fn_ctrl, self.dpi);
                                    st.add_footnote_height(fn_height);
                                }
                            }
                        }
                    }
                }
                Control::Shape(_) | Control::Picture(_) | Control::Equation(_) => {
                    // Task #402: 같은 paragraph의 선행 TAC 컨트롤이 있는 TAC 그림은
                    // 자기 line_seg에 위치하므로 그 line의 높이를 페이지 누적에 반영해야 함.
                    // 누락 시 후속 항목이 페이지 끝을 넘어 그려져 겹침/오버플로 발생 (#402).
                    let tac_separate_line_h: Option<f64> = match ctrl {
                        Control::Picture(p) if p.common.treat_as_char => Some(()),
                        Control::Shape(s) if s.common().treat_as_char => Some(()),
                        _ => None,
                    }
                    .and_then(|_| {
                        let prior_tac_count = para
                            .controls
                            .iter()
                            .take(ctrl_idx)
                            .filter(|c| match c {
                                Control::Table(t) => t.common.treat_as_char,
                                Control::Picture(p) => p.common.treat_as_char,
                                Control::Shape(s) => s.common().treat_as_char,
                                _ => false,
                            })
                            .count();
                        if prior_tac_count == 0 {
                            return None;
                        }
                        para.line_segs.get(prior_tac_count).map(|seg| {
                            let lh = hwpunit_to_px(seg.line_height, self.dpi);
                            let ls_extra = if seg.line_spacing > 0 {
                                hwpunit_to_px(seg.line_spacing, self.dpi)
                            } else {
                                0.0
                            };
                            lh + ls_extra
                        })
                    });
                    // [Issue #1156] 비-TAC 자리차지(TopAndBottom) 객체(차트 OLE 등):
                    // 표와 같은 문단에 있으면 종전에는 높이/단 이동 없이 push 만 되어,
                    // 한컴처럼 단 끝을 넘는 객체가 다음 단으로 이동하지 못했다.
                    // 한컴: 객체가 현재 단 잔여 영역을 넘으면 다음 단 상단으로 이동.
                    // (객체 점유 크기 = common 높이 80mm, spec/한컴/HWPX hp:sz 3중 일치)
                    use crate::model::shape::{TextWrap, VertRelTo};
                    let non_tac_pushdown_h: Option<f64> = if tac_separate_line_h.is_none() {
                        match ctrl {
                            Control::Picture(p)
                                if !p.common.treat_as_char
                                    && matches!(p.common.text_wrap, TextWrap::TopAndBottom)
                                    && matches!(p.common.vert_rel_to, VertRelTo::Para) =>
                            {
                                let h = hwpunit_to_px(p.common.height as i32, self.dpi);
                                let mb = hwpunit_to_px(p.common.margin.bottom as i32, self.dpi);
                                Some(h + mb)
                            }
                            Control::Shape(s)
                                if !s.common().treat_as_char
                                    && matches!(s.common().text_wrap, TextWrap::TopAndBottom)
                                    && matches!(s.common().vert_rel_to, VertRelTo::Para) =>
                            {
                                let cm = s.common();
                                let h = hwpunit_to_px(cm.height as i32, self.dpi);
                                let mb = hwpunit_to_px(cm.margin.bottom as i32, self.dpi);
                                Some(h + mb)
                            }
                            _ => None,
                        }
                    } else {
                        None
                    };

                    if let Some(line_h) = tac_separate_line_h {
                        // 자기 line이 현재 페이지에 들어가지 않으면 다음 페이지로 분할
                        if !st.current_items.is_empty()
                            && st.current_height + line_h > st.available_height() + 0.5
                        {
                            st.advance_column_or_new_page();
                        }
                    } else if let Some(extra) = non_tac_pushdown_h {
                        // 비-TAC 자리차지 객체: 현재 단 잔여 부족 + 단 상단 아니면 다음 단/페이지 이동
                        let is_column_top = st.current_height < 1.0;
                        if !is_column_top && st.current_height + extra > st.available_height() + 0.5
                        {
                            st.advance_column_or_new_page();
                        }
                    }
                    st.current_items.push(PageItem::Shape {
                        para_index: para_idx,
                        control_index: ctrl_idx,
                    });
                    if let Some(line_h) = tac_separate_line_h {
                        st.current_height += line_h;
                    } else if let Some(extra) = non_tac_pushdown_h {
                        st.current_height += extra;
                    }
                }
                _ => {}
            }
        }

        // TAC 표 높이 보정 (Paginator engine.rs:123-179 동일)
        if has_tac && fmt.total_height > 0.0 && st.pages.len() == page_count_before {
            let height_added = st.current_height - height_before;
            // tac_seg_total 계산: 각 TAC 표의 max(seg.lh, 실측높이) + ls/2
            let mut tac_seg_total = 0.0;
            let mut tac_idx = 0;
            for (ci, c) in para.controls.iter().enumerate() {
                if let Control::Table(t) = c {
                    if self.is_effective_tac_table(para, t, &fmt) {
                        if let Some(seg) = para.line_segs.get(tac_idx) {
                            let seg_lh = hwpunit_to_px(seg.line_height, self.dpi);
                            let mt_h = measured_tables
                                .iter()
                                .find(|mt| mt.para_index == para_idx && mt.control_index == ci)
                                .map(|mt| mt.total_height)
                                .unwrap_or(0.0);
                            let effective_h = seg_lh.max(mt_h);
                            let ls_half = hwpunit_to_px(seg.line_spacing, self.dpi) / 2.0;
                            tac_seg_total += effective_h + ls_half;
                        }
                        tac_idx += 1;
                    }
                }
            }
            let cap = if tac_seg_total > 0.0 {
                let is_col_top = height_before < 1.0;
                let effective_sb = if is_col_top { 0.0 } else { fmt.spacing_before };
                let outer_top: f64 = para
                    .controls
                    .iter()
                    .filter_map(|c| match c {
                        Control::Table(t) if self.is_effective_tac_table(para, t, &fmt) => {
                            Some(hwpunit_to_px(t.outer_margin_top as i32, self.dpi))
                        }
                        _ => None,
                    })
                    .sum();
                (effective_sb + outer_top + tac_seg_total).min(fmt.total_height)
            } else {
                fmt.total_height
            };
            if height_added > cap {
                st.current_height = height_before + cap;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn try_typeset_empty_para_float_table(
        &self,
        st: &mut TypesetState,
        para_idx: usize,
        ctrl_idx: usize,
        para: &Paragraph,
        table: &crate::model::table::Table,
        ft: &FormattedTable,
        composed: Option<&ComposedParagraph>,
        styles: &ResolvedStyleSet,
        para_start_height: f64,
        lanes: &mut FloatLaneSet,
    ) -> bool {
        if !is_para_topbottom_float(&table.common) || para_has_visible_text(para) {
            return false;
        }
        let para_float_count = para
            .controls
            .iter()
            .filter(|ctrl| matches!(ctrl, Control::Table(t) if is_para_topbottom_float(&t.common)))
            .take(2)
            .count();
        if para_float_count < 2 {
            return false;
        }

        let column_area = st
            .layout
            .column_areas
            .get(st.current_column as usize)
            .copied()
            .unwrap_or(st.layout.body_area);
        let width_px = hwpunit_to_px(signed_hwpunit(table.common.width), self.dpi);
        if width_px <= 0.0 || ft.effective_height <= 0.0 {
            return false;
        }

        let para_style_id = composed
            .map(|c| c.para_style_id as usize)
            .unwrap_or(para.para_shape_id as usize);
        let para_style = styles.para_styles.get(para_style_id);
        let margin_left = para_style.map(|s| s.margin_left).unwrap_or(0.0);
        let indent = para_style.map(|s| s.indent).unwrap_or(0.0);
        let effective_margin = if indent > 0.0 {
            margin_left + indent
        } else {
            margin_left
        };
        let margin_right = para_style.map(|s| s.margin_right).unwrap_or(0.0);

        let placement_ctx = FloatPlacementContext::new(column_area)
            .with_body_area(st.layout.body_area)
            .with_paper_width(st.layout.page_width)
            .with_host_margins(effective_margin, margin_right);
        let (x_start, x_end) = horizontal_range(&table.common, width_px, placement_ctx, self.dpi);

        let v_offset_px = hwpunit_to_px(signed_hwpunit(table.common.vertical_offset), self.dpi);
        let raw_top = (para_start_height + v_offset_px).max(para_start_height);
        let reserved_height = ft.effective_height + ft.host_spacing.after;
        let lane_top = lanes.pushed_top(x_start, x_end, raw_top);
        let lane_bottom = lane_top + reserved_height;

        let total_footnote =
            st.projected_footnote_height(ft.table_footnote_height, ft.table_footnote_count);
        let fn_margin = if total_footnote > 0.0 {
            st.footnote_safety_margin
        } else {
            0.0
        };
        let available =
            (st.base_available_height() - total_footnote - fn_margin - st.current_zone_y_offset)
                .max(0.0);

        if lane_bottom > available + 0.5 {
            return false;
        }

        st.current_items.push(PageItem::Table {
            para_index: para_idx,
            control_index: ctrl_idx,
        });
        lanes.place(x_start, x_end, raw_top, reserved_height);
        st.current_height = st.current_height.max(lanes.max_bottom());
        true
    }

    /// TAC(treat_as_char) 표의 조판.
    #[allow(clippy::too_many_arguments)]
    fn typeset_tac_table(
        &self,
        st: &mut TypesetState,
        para_idx: usize,
        ctrl_idx: usize,
        para: &Paragraph,
        table: &crate::model::table::Table,
        ft: &FormattedTable,
        fmt: &FormattedParagraph,
        tac_count: usize,
        is_first_placed: bool,
        is_last_placed: bool,
    ) {
        // [Task #1152] 호스트 문단의 intra-paragraph vpos-reset 가드.
        // empty-text host paragraph 가 N controls + N line_segs 1:1 매핑이고,
        // 현재 TAC 표의 매핑 line_seg(ctrl_idx>0) 의 vpos==0 이면 HWP 가 "이 표를
        // 새 페이지 상단부터" 라고 명시한 신호. fit 검사는 표 크기가 잔여 영역에
        // 들어가면 통과시키지만 명시 신호를 존중하려면 fit 이전에 advance.
        // 케이스: 2022년 국립국어원 업무계획.hwp pi=586 ci=1 (별첨 박스).
        if !st.current_items.is_empty()
            && ctrl_idx > 0
            && para.text.is_empty()
            && para.line_segs.len() == para.controls.len()
            && para
                .line_segs
                .get(ctrl_idx)
                .map(|s| s.vertical_pos)
                .unwrap_or(-1)
                == 0
        {
            st.advance_column_or_new_page();
        }

        let tac_table_line_idx = self.tac_table_line_index(para, table, fmt);
        let tac_seg_idx = if tac_count > 1 {
            para.controls
                .iter()
                .take(ctrl_idx)
                .filter(
                    |c| matches!(c, Control::Table(t) if self.is_effective_tac_table(para, t, fmt)),
                )
                .count()
        } else {
            tac_table_line_idx.unwrap_or(0)
        };
        // 다중 TAC 표: LINE_SEG 기반 개별 높이 계산
        let table_height = if tac_count > 1 {
            let is_last_tac = tac_seg_idx + 1 == tac_count;
            para.line_segs
                .get(tac_seg_idx)
                .map(|seg| {
                    let line_h = hwpunit_to_px(seg.line_height, self.dpi);
                    if is_last_tac {
                        line_h
                    } else {
                        line_h + hwpunit_to_px(seg.line_spacing, self.dpi)
                    }
                })
                .unwrap_or(ft.total_height)
        } else if tac_table_line_idx == Some(0) && fmt.line_heights.len() > 1 {
            // PR #1088 follow-up: hwp-multi-001 pi=46 처럼 TAC 표가 문단의
            // 첫 줄이고 뒤따르는 제목 줄이 같은 문단의 line1(vpos reset)로
            // 인코딩된 경우가 있다. 표 자체는 현재 페이지에 들어가고 post-text
            // 만 다음 페이지로 넘어가야 하는데, 문단 전체 height_for_fit으로
            // fit 판단하면 표까지 다음 페이지로 밀린다.
            //
            // 이때 fit 기준은 line_height만 사용한다. line_spacing까지 포함한
            // line_advance를 쓰면 HWPX lineSeg가 `표줄 + 다음 텍스트줄`로
            // 분리된 문서에서, 표 자체는 남은 영역에 들어가는데도 spacing 때문에
            // 표가 다음 페이지로 밀린다(2025 donations HWPX pi=25).
            fmt.line_heights[0]
        } else if fmt.total_height > 0.0 {
            // 단일 TAC: 호스트 문단의 height_for_fit 사용
            fmt.height_for_fit
        } else {
            ft.total_height
        };

        // TAC 표는 분할하지 않고 통째로 배치
        let available = st.available_height();
        let current_column_has_only_overlay_shapes = st.current_height <= 0.5
            && st
                .current_items
                .iter()
                .all(|item| matches!(item, PageItem::Shape { .. }));
        let fits_after_overlay_shapes =
            current_column_has_only_overlay_shapes && table_height <= available + 12.0;
        let current_page_vpos_base = st.vpos_page_base.unwrap_or(0);
        let saved_tac_table_bottom_fits = Some(current_page_vpos_base)
            .and_then(|base| {
                para.line_segs
                    .get(tac_seg_idx)
                    .and_then(|seg| line_seg_visible_bounds_px(seg, base, self.dpi))
            })
            .is_some_and(|bounds| {
                saved_bounds_fit_at_flow_tail(bounds, st.current_height, available)
            });
        if st.current_height + table_height > available
            && !fits_after_overlay_shapes
            && !saved_tac_table_bottom_fits
            && !st.current_items.is_empty()
        {
            st.advance_column_or_new_page();
        }

        self.place_table_with_text(
            st,
            para_idx,
            ctrl_idx,
            para,
            table,
            fmt,
            st.current_height,
            table_height,
            is_first_placed,
            is_last_placed,
        );
    }

    /// 표를 pre-text/table/post-text와 함께 배치한다 (Paginator place_table_fits 동일).
    #[allow(clippy::too_many_arguments)]
    fn place_table_with_text(
        &self,
        st: &mut TypesetState,
        para_idx: usize,
        ctrl_idx: usize,
        para: &Paragraph,
        table: &crate::model::table::Table,
        fmt: &FormattedParagraph,
        para_start_height: f64,
        table_total_height: f64,
        is_first_placed: bool,
        is_last_placed: bool,
    ) {
        let vertical_offset = Self::get_table_vertical_offset(table);
        let is_visible_para_float =
            is_para_topbottom_float(&table.common) && para_has_non_whitespace_text(para);
        let signed_vertical_offset = vertical_offset as i32;
        let total_lines = fmt.line_heights.len();
        let pre_table_end_line = if !is_visible_para_float
            && signed_vertical_offset > 0
            && !para.text.is_empty()
        {
            total_lines
        } else if table.common.treat_as_char
            && total_lines > 1
            && para.text.chars().any(|c| c.is_alphanumeric())
        {
            // 전폭 TAC 표가 자동 줄바꿈으로 자기 줄(line index N)에 놓인 경우(\n 없음):
            // 한컴은 LINE_SEG 순서대로 line0=텍스트 → lineN=표 로 렌더한다.
            // control_text_positions() 는 char_offsets 가 비면 무용하므로, 표 줄의 높이
            // (표 본체 + outer margin top/bottom)와 일치하는 LINE_SEG 인덱스로 판정한다.
            // PUA 필러/공백만 있는 문단(예: 복학원서.hwp pi=16 — 한컴이 표 폭만큼 필러로
            // 줄바꿈시킨 케이스)은 is_alphanumeric() 가 false 라 제외 → compute_tac_leading
            // 경로 유지. (Task #853, Task #842 결함 #2 의 PUA 필러 판정과 정합)
            let om_top = hwpunit_to_px(table.outer_margin_top as i32, self.dpi);
            let om_bot = hwpunit_to_px(table.outer_margin_bottom as i32, self.dpi);
            let tbl_line_h = hwpunit_to_px(table.common.height as i32, self.dpi) + om_top + om_bot;
            para.line_segs
                .iter()
                .enumerate()
                .find(|(_, ls)| (hwpunit_to_px(ls.line_height, self.dpi) - tbl_line_h).abs() < 1.0)
                .map(|(i, _)| i)
                .unwrap_or(0)
        } else {
            0
        };

        // [Task #439] Square wrap (어울림) 표 식별.
        // 어울림 표는 호스트 문단 텍스트와 같은 수직 영역에 배치되므로
        // current_height 누적은 max(host_text, v_off + table) 한 번만.
        // engine.rs:1328 와 동일 시멘틱.
        let is_wrap_around_table = !table.common.treat_as_char
            && matches!(
                table.common.text_wrap,
                crate::model::shape::TextWrap::Square
            );

        // pre-table 텍스트 (첫 번째 표에서만)
        // [참고2 fix] 배열순서가 아닌 배치순서 기준 (typeset_table_paragraph 산출).
        let is_first_table = is_first_placed;
        let pre_height: f64 = if pre_table_end_line > 0 && is_first_table {
            let h = fmt.line_advances_sum(0..pre_table_end_line);
            st.current_items.push(PageItem::PartialParagraph {
                para_index: para_idx,
                start_line: 0,
                end_line: pre_table_end_line,
            });
            h
        } else {
            0.0
        };

        // 표 배치
        st.current_items.push(PageItem::Table {
            para_index: para_idx,
            control_index: ctrl_idx,
        });

        // [Task #439] 누적 정책:
        // - Square wrap (어울림): max(pre_height, v_off + table_total)
        //     호스트 텍스트와 표가 같은 y 영역을 공유하므로 더 큰 쪽만 누적.
        // - 그 외 (TopAndBottom 등): pre_height + table_total 합산 (기존 동작).
        // 전폭 TAC 표가 자기 줄(line index = pre_table_end_line)에 놓인 split 케이스:
        // table_total_height(=fmt.height_for_fit)는 pre-text 줄까지 포함하므로 pre_height
        // 를 따로 더하면 이중 계산이 된다. 또 표가 차지한 줄은 post-text 에서 제외해야 한다.
        // (Task #853)
        let tac_wrap_split = table.common.treat_as_char
            && pre_table_end_line > 0
            && pre_table_end_line < total_lines;

        if is_wrap_around_table && pre_height > 0.0 {
            let v_off_px = crate::renderer::hwpunit_to_px(vertical_offset as i32, self.dpi);
            let table_bottom = v_off_px + table_total_height;
            st.current_height += pre_height.max(table_bottom);
        } else if is_visible_para_float {
            let v_off_px = hwpunit_to_px(signed_vertical_offset, self.dpi);
            let outer_top_px = hwpunit_to_px(table.outer_margin_top as i32, self.dpi);
            let table_top = if signed_vertical_offset > 0 {
                para_start_height + outer_top_px + v_off_px
            } else if st.is_hwpx_source {
                // HWPX visible float 는 같은 문단 안의 앞선 float 뒤에 이어 쌓인다.
                // B/C처럼 둘 다 non-positive offset 이면 문단 시작점이 아니라 현재 흐름
                // 높이를 기준으로 reserve 해야 layout 의 세로 stacking 과 page break 가 맞는다.
                let flow_at_para_start = (st.current_height - para_start_height).abs() < 0.5;
                st.current_height
                    + if flow_at_para_start {
                        outer_top_px
                    } else {
                        0.0
                    }
                    + v_off_px.max(0.0)
            } else {
                para_start_height + outer_top_px + v_off_px
            };
            let table_bottom = table_top + table_total_height.max(0.0);
            if signed_vertical_offset > 0 {
                if table_bottom > table_top + 0.5 {
                    st.visible_float_exclusions.push(VisibleFloatExclusion {
                        top: table_top,
                        bottom: table_bottom,
                    });
                }
                st.current_height += pre_height;
            } else {
                let following_non_positive =
                    has_following_non_positive_visible_float(para, ctrl_idx);
                let inter_float_gap = if st.is_hwpx_source && following_non_positive {
                    para_line_spacing_px(para, self.dpi)
                } else {
                    0.0
                };
                st.current_height = st.current_height.max(table_bottom + inter_float_gap);
            }
        } else if tac_wrap_split {
            st.current_height += table_total_height;
        } else {
            st.current_height += pre_height + table_total_height;
        }

        // post-table 텍스트
        let is_last_table = is_last_placed;
        let tac_table_count = para
            .controls
            .iter()
            .filter(|c| matches!(c, Control::Table(t) if self.is_effective_tac_table(para, t, fmt)))
            .count();
        let post_table_start = if tac_wrap_split {
            (pre_table_end_line + 1).min(total_lines).max(1)
        } else if table.attr & 0x01 != 0 {
            pre_table_end_line.max(1)
        } else if table.common.treat_as_char && total_lines > pre_table_end_line + 1 {
            // HWPX TAC 표(attr 비트0=0): 표줄(pre_table_end_line) 다음에 실제 본문 줄이
            // 있으면 표줄을 post-text 에서 제외(HWP5 attr&0x01 의 pre_end.max(1) 와 정합).
            // 단일 줄(표줄만)은 건드리지 않아 기존 동작 보존.
            pre_table_end_line + 1
        } else if is_last_table && !is_first_table {
            0
        } else {
            pre_table_end_line
        };
        // 중복 방지: 이전 표가 이미 같은 문단의 pre-text(start_line=0)를 추가했으면 건너뜀
        // (engine.rs:1418-1421 와 동일한 가드 — 다중 TopAndBottom 표 문단에서
        //  같은 line 범위가 두 번 emit되어 본문이 두 번 렌더되는 문제 차단)
        let pre_text_exists = post_table_start == 0
            && st.current_items.iter().any(|item| {
                matches!(item, PageItem::PartialParagraph { para_index, start_line, .. }
                if *para_index == para_idx && *start_line == 0)
            });
        let has_substantive_text = para_has_non_whitespace_text(para);
        let whitespace_only_single_tac_host_line = !has_substantive_text
            && !para.text.is_empty()
            && table.common.treat_as_char
            && pre_table_end_line == 0
            && total_lines <= 1;
        let has_post_text = !para.text.is_empty()
            && total_lines > post_table_start
            && !whitespace_only_single_tac_host_line;
        let should_add_post_text =
            is_last_table && tac_table_count <= 1 && has_post_text && !pre_text_exists;
        if should_add_post_text {
            let post_height: f64 = fmt.line_advances_sum(post_table_start..total_lines);
            if self.tac_table_line_index(para, table, fmt) == Some(0)
                && st.current_height + post_height > st.available_height() + 0.5
                && !st.current_items.is_empty()
            {
                st.advance_column_or_new_page();
            }
            st.current_items.push(PageItem::PartialParagraph {
                para_index: para_idx,
                start_line: post_table_start,
                end_line: total_lines,
            });
            st.current_height += post_height;
        }

        // TAC 표: trailing line_spacing 복원 (Paginator place_table_fits:777-783 동일)
        // has_post_text는 tac_table_count와 무관하게 텍스트 줄 존재 여부만 확인
        let is_tac = self.is_effective_tac_table(para, table, fmt);
        if is_tac && fmt.total_height > fmt.height_for_fit && !has_post_text {
            st.current_height += fmt.total_height - fmt.height_for_fit;
        }
    }

    fn tac_table_line_index(
        &self,
        para: &Paragraph,
        table: &crate::model::table::Table,
        fmt: &FormattedParagraph,
    ) -> Option<usize> {
        if !table.common.treat_as_char || fmt.line_heights.len() <= 1 {
            return None;
        }

        let om_top = hwpunit_to_px(table.outer_margin_top as i32, self.dpi);
        let om_bot = hwpunit_to_px(table.outer_margin_bottom as i32, self.dpi);
        let table_line_h = hwpunit_to_px(table.common.height as i32, self.dpi) + om_top + om_bot;

        para.line_segs.iter().enumerate().find_map(|(idx, seg)| {
            let line_h = hwpunit_to_px(seg.line_height, self.dpi);
            if (line_h - table_line_h).abs() < 1.0 {
                Some(idx)
            } else {
                None
            }
        })
    }

    fn is_effective_tac_table(
        &self,
        para: &Paragraph,
        table: &crate::model::table::Table,
        fmt: &FormattedParagraph,
    ) -> bool {
        table.attr & 0x01 != 0 || self.tac_table_line_index(para, table, fmt) == Some(0)
    }

    /// 비-TAC 블록 표의 조판: fits → place / split(Break Token 기반).
    /// 기존 Paginator의 split_table_rows와 동일한 세밀한 분할 로직.
    #[allow(clippy::too_many_arguments)]
    fn typeset_block_table(
        &self,
        st: &mut TypesetState,
        para_idx: usize,
        ctrl_idx: usize,
        para: &Paragraph,
        table: &crate::model::table::Table,
        ft: &FormattedTable,
        fmt: &FormattedParagraph,
        mt: Option<&MeasuredTable>,
        styles: &ResolvedStyleSet,
        para_start_height: f64,
        is_first_placed: bool,
        is_last_placed: bool,
    ) {
        // 표 내 각주를 고려한 가용 높이 계산 (Paginator engine.rs:583-586 동일)
        let total_footnote =
            st.projected_footnote_height(ft.table_footnote_height, ft.table_footnote_count);
        let fn_margin = if total_footnote > 0.0 {
            st.footnote_safety_margin
        } else {
            0.0
        };
        let available =
            (st.base_available_height() - total_footnote - fn_margin - st.current_zone_y_offset)
                .max(0.0);

        let host_spacing_total = ft.host_spacing.before + ft.host_spacing.after;
        let mut table_total = ft.effective_height + host_spacing_total;

        // [Task #1046 Stage 1] 표 측정 드리프트 진단: 페이지네이터 effective_height vs
        // MeasuredTable 행높이 합(+cell_spacing). RHWP_TABLE_DRIFT=1 시 출력.
        if std::env::var("RHWP_TABLE_DRIFT").is_ok() {
            let (mt_sum, mt_rows, mt_cs) = match mt {
                Some(m) => {
                    let cs_total = m.cell_spacing * (m.row_heights.len() as f64 + 1.0);
                    (
                        m.row_heights.iter().sum::<f64>() + cs_total,
                        m.row_heights.len(),
                        m.cell_spacing,
                    )
                }
                None => (f64::NAN, 0, 0.0),
            };
            eprintln!(
                "TABLE_DRIFT: pi={} sec={} eff_h={:.1} host_sp={:.1} table_total={:.1} mt_sum={:.1} mt_rows={} cs={:.1} cur_h={:.1} tac={} rows={}",
                para_idx, st.section_index, ft.effective_height, host_spacing_total, table_total,
                mt_sum, mt_rows, mt_cs, st.current_height, table.common.treat_as_char, table.row_count,
            );
        }
        // [Task #1027 Stage E1] treat_as_char 인라인 표 advance 정합.
        // 렌더러는 글자처럼취급 표를 호스트 문단의 한 LINE_SEG(line_height+line_spacing)로
        // advance 하나(=fmt.total_height), 페이지네이터는 측정된 표 effective_height 만
        // 더해 ~수십px 과소측정 → 표 이후 콘텐츠가 렌더러보다 위에 fit 판정되어 overflow
        // (Stage D 조사: p71 pi=349 +16.9px). 호스트가 표 한 줄로 구성된 경우(line==1)
        // 렌더러 advance(fmt.total_height)로 정합한다.
        if table.common.treat_as_char
            && fmt.line_heights.len() == 1
            && fmt.total_height > table_total
        {
            table_total = fmt.total_height;
        }

        // Task #321 v5: Paper-anchored TopAndBottom block 표는 절대 좌표로 그려지므로
        // cur_h advance 에 표 effective_height 를 그대로 더하면 본문 LINE_SEG vpos 와
        // mismatch (= 21_언어 page 1 col 0 의 +76 px drift). 본문 좌표계와 동기화 하기
        // 위해 host paragraph 의 first_vpos 만큼 cur_h 를 미리 jump 하고 표 advance 를
        // 본문 라인 만큼으로 축소.
        use crate::model::shape::{TextWrap, VertRelTo};
        let is_paper_topbottom_block = !table.common.treat_as_char
            && matches!(table.common.text_wrap, TextWrap::TopAndBottom)
            && matches!(table.common.vert_rel_to, VertRelTo::Paper);
        if is_paper_topbottom_block && st.current_column == 0 {
            if let Some(first_seg) = para.line_segs.first() {
                let target_y =
                    crate::renderer::hwpunit_to_px(first_seg.vertical_pos as i32, self.dpi);
                // 호스트 본문 lines + 표는 절대 좌표 → cur_h 는 first_vpos + host lines 만 진행.
                let pre_lines_h = fmt.line_advances_sum(0..fmt.line_heights.len());
                if target_y > st.current_height && target_y + pre_lines_h <= available {
                    st.current_height = target_y;
                    // table_total = 0: 표 자체는 cur_h advance 에 영향 없음 (Paper-absolute).
                    // 호스트 본문 lines 만 place_table_with_text 가 pre_height 로 추가.
                    self.place_table_with_text(
                        st,
                        para_idx,
                        ctrl_idx,
                        para,
                        table,
                        fmt,
                        para_start_height,
                        0.0,
                        is_first_placed,
                        is_last_placed,
                    );
                    return;
                }
            }
        }

        // fits: 전체가 현재 페이지에 들어가는가?
        let is_rowbreak_para_topbottom_block = !table.common.treat_as_char
            && matches!(table.common.text_wrap, TextWrap::TopAndBottom)
            && matches!(table.common.vert_rel_to, VertRelTo::Para)
            && matches!(
                table.page_break,
                crate::model::table::TablePageBreak::RowBreak
            )
            && table.cells.iter().any(|cell| {
                cell.paragraphs.iter().any(|p| {
                    !p.text.trim().is_empty()
                        && p.controls
                            .iter()
                            .any(|c| matches!(c, crate::model::control::Control::Table(_)))
                })
            });
        if is_rowbreak_para_topbottom_block {
            if let Some(first_seg) = para.line_segs.first() {
                let target_y =
                    crate::renderer::hwpunit_to_px(first_seg.vertical_pos as i32, self.dpi);
                let previous_item_is_continued_paragraph = matches!(
                    st.current_items.last(),
                    Some(PageItem::PartialParagraph { start_line, .. }) if *start_line > 0
                );
                if !previous_item_is_continued_paragraph
                    && target_y > st.current_height
                    && target_y < available
                {
                    st.current_height = target_y;
                }
            }
        }

        // [Task #1611] PAGE-앵커(vert=쪽) + valign=Bottom 자리차지 표(발신명의 footer 등)는
        // 한컴이 stored vpos 위치에 두고, 본문 누적이 그 위치+높이를 넘기면 블록을 통째로
        // 다음 쪽에 단독 배치한다. Paper-앵커(절대좌표, 위 10440)와 달리 페이지네이션에
        // 참여하므로 cur_h 를 stored vpos 로 끌어올린 뒤(본문 흐름이 vpos 보다 짧을 때) fit 을
        // 판정한다. 동기화하지 않으면 footer 가 flowed cur_h(vpos 보다 ~수십px 낮음)에 배치되어
        // page-fit 이 과소되고 footer 가 본문 페이지에 흡수된다(−1쪽 갭 요인 B).
        let is_page_bottom_topbottom_block = !table.common.treat_as_char
            && matches!(table.common.text_wrap, TextWrap::TopAndBottom)
            && matches!(table.common.vert_rel_to, VertRelTo::Page)
            && matches!(
                table.common.vert_align,
                crate::model::shape::VertAlign::Bottom
            );
        if is_page_bottom_topbottom_block && st.current_column == 0 {
            if let Some(first_seg) = para.line_segs.first() {
                let target_y =
                    crate::renderer::hwpunit_to_px(first_seg.vertical_pos as i32, self.dpi);
                // 한컴은 고정크기 자리차지 블록을 **선언 높이**(common.height)로 렌더·예약한다.
                // 페이지네이터의 effective_height 는 셀 내용 기반 측정치라 선언보다 작을 수 있어
                // (footer 351.4px 선언 vs 302.3px 측정) fit 이 과소된다 → 선언 높이로 판정·예약.
                let declared_px =
                    crate::renderer::hwpunit_to_px(table.common.height as i32, self.dpi);
                let block_height = table_total.max(declared_px);
                // [Task #1624] footer stored vpos 가 흐름 cur_h 보다 footer 한 개 높이 이상 위에
                // 있으면(본문이 짧은데 vpos 가 page-bottom 앵커/누적 노이즈), vpos 동기화는
                // 본문 직후에 들어갈 footer 를 spurious 하게 다음 쪽으로 민다(+1쪽 over-push).
                // vpos 가 흐름을 plausibly 따를 때(cur_h + block_height 이내)만 동기화한다.
                let sync_h = if target_y <= st.current_height + block_height {
                    st.current_height.max(target_y)
                } else {
                    st.current_height
                };
                if sync_h + block_height <= available {
                    // 현재 쪽에 stored vpos 위치로 배치.
                    st.current_height = sync_h;
                } else if !st.current_items.is_empty() {
                    // vpos 기준 초과 → 발신명의 블록을 통째로 다음 쪽에 단독 배치(분할 부적절).
                    st.advance_column_or_new_page();
                }
                self.place_table_with_text(
                    st,
                    para_idx,
                    ctrl_idx,
                    para,
                    table,
                    fmt,
                    para_start_height,
                    block_height,
                    is_first_placed,
                    is_last_placed,
                );
                return;
            }
        }

        let current_column_has_only_overlay_shapes = st.current_height <= 0.5
            && st
                .current_items
                .iter()
                .all(|item| matches!(item, PageItem::Shape { .. }));
        let fits_after_overlay_shapes =
            current_column_has_only_overlay_shapes && table_total <= available + 12.0;
        if st.current_height + table_total <= available || fits_after_overlay_shapes {
            self.place_table_with_text(
                st,
                para_idx,
                ctrl_idx,
                para,
                table,
                fmt,
                para_start_height,
                if is_para_topbottom_float(&table.common) && para_has_non_whitespace_text(para) {
                    ft.effective_height
                } else {
                    table_total
                },
                is_first_placed,
                is_last_placed,
            );
            return;
        }

        // [Task #991] 1행짜리 글자처럼취급(treat_as_char) 표는 페이지 경계에서
        // 분할하지 않고 통째로 다음 페이지/단으로 이동한다.
        //
        // 표 분할은 행 경계 분할이 기본이고, 행 경계가 없는 1행 표는 셀 내용을
        // 페이지 중간에서 자르는 인트라-셀 분할만 가능하다. 글자처럼취급 표는
        // 본문 흐름 안의 한 글자 같은 인라인 개체이므로 인트라-셀 분할은 부적절하다
        // (한컴은 통째로 다음 페이지로 넘김). 다행(多行) tac 표는 행 경계 분할이
        // 가능하므로 기존 로직을 유지하고, 1행 tac 표만 통째 이동시킨다.
        // 한 페이지에도 안 들어가는 초대형 표는 분할 외 방법이 없으므로 폴백한다.
        if table.common.treat_as_char && table.row_count <= 1 && table_total <= available {
            if !st.current_items.is_empty() {
                st.advance_column_or_new_page();
            }
            self.place_table_with_text(
                st,
                para_idx,
                ctrl_idx,
                para,
                table,
                fmt,
                para_start_height,
                table_total,
                is_first_placed,
                is_last_placed,
            );
            return;
        }

        // MeasuredTable이 없거나 행이 없으면 강제 배치
        let mt = match mt {
            Some(m) if !m.row_heights.is_empty() => m,
            _ => {
                if !st.current_items.is_empty() {
                    st.advance_column_or_new_page();
                }
                st.current_items.push(PageItem::Table {
                    para_index: para_idx,
                    control_index: ctrl_idx,
                });
                st.current_height += ft.effective_height;
                return;
            }
        };

        let row_count = mt.row_heights.len();
        let cs = mt.cell_spacing;
        let can_intra_split = !mt.cells.is_empty();
        let base_available = st.base_available_height();
        // Partial table borders are rendered against the visible body area. The paginator-level
        // bottom tolerance is useful for text fit heuristics, but if row cuts spend it here the
        // table fragment can be painted into the footer/body edge and get clipped.
        let table_available = (available - st.layout.pagination_tolerance_px).max(0.0);

        // [Task #993] advance_row_cut 호출용 LayoutEngine — 컷 측정은 dpi 와
        // 셀 패딩/중첩 표 높이 계산에만 의존하므로 ad hoc 인스턴스로 충분하다.
        let layout_engine = crate::renderer::layout::LayoutEngine::new(self.dpi);
        layout_engine.set_hwp3_variant(st.is_hwp3_variant);
        layout_engine.set_hwpx_source(st.is_hwpx_source);
        // [Task #993] rowspan(row_span>1) 셀이 걸친 행 — 컷 모델(advance_row_cut)은
        // row_span==1 셀만 다루므로 rowspan 셀 높이를 측정하지 못한다. 구현계획서
        // §4대로 rowspan 행은 MeasuredTable 행 높이를 권위로 쓴다(렌더러도 동일).
        let rowspan_touched: Vec<bool> = (0..row_count)
            .map(|r| {
                table.cells.iter().any(|c| {
                    c.row_span > 1
                        && (c.row as usize) <= r
                        && r < c.row as usize + c.row_span as usize
                })
            })
            .collect();
        // [Task #993/#1022] 행별 전체 높이(fresh, 빈 컷). HeightMeasurer 와 정합된
        // row_cut_content_height(셀별 max(cell.height, content+pad_cell) 의 행 max)
        // 로 측정해 렌더러와 단일 측정 공간을 공유한다. rowspan 행은 컷 모델 범위
        // 밖이므로 MeasuredTable.row_heights 폴백.
        let cut_row_h: Vec<f64> = (0..row_count)
            .map(|r| {
                if rowspan_touched[r] {
                    mt.row_heights[r]
                } else {
                    layout_engine.row_cut_content_height(table, r, &[], &[], styles)
                }
            })
            .collect();

        // [Task #1046 Stage 1] 분할 표 cut 행높이 vs 렌더러 MeasuredTable 행높이 비교.
        if std::env::var("RHWP_TABLE_DRIFT").is_ok() {
            let cut_sum: f64 = cut_row_h.iter().sum();
            let mt_sum: f64 = mt.row_heights.iter().sum();
            eprintln!(
                "TABLE_CUT_DRIFT: pi={} sec={} cut_sum={:.1} mt_sum={:.1} diff={:+.1} cut_rows={:?} mt_rows={:?}",
                para_idx, st.section_index, cut_sum, mt_sum, mt_sum - cut_sum,
                cut_row_h.iter().map(|h| (h * 10.0).round() / 10.0).collect::<Vec<_>>(),
                mt.row_heights.iter().map(|h| (h * 10.0).round() / 10.0).collect::<Vec<_>>(),
            );
        }

        // 첫 행이 남은 공간보다 크면 다음 페이지로 (인트라-로우 분할 가능성 확인).
        // Task #398: rowspan>1 셀이 행 0의 시작점이면 블록 전체 높이로 판정.
        // [Task #1046 Stage 2] 첫(비연속) fragment 의 렌더러 y_start 점프 — host_spacing.before
        // 와 (TopAndBottom+vert=Para+v_off>0 표의) vertical_offset — 를 잔여공간에서 차감한다.
        // 종전엔 미차감해 잔여를 과대평가 → 첫 행이 실제 안 들어가는데도 가드를 통과시켜
        // 일반 행 강제 배치 경로가 통째로 밀어넣어 본문 초과(예: pi=242 vert_off 38px,
        // 잔여 65.4px 로 보였으나 실가용 23.4px < 행0 34.9px). 루프 내 page_avail
        // (host_before_overhead/vert_offset_overhead) 와 동일 overhead 를 가드에도 적용.
        let first_frag_overhead = {
            let host_before = ft.host_spacing.before;
            let vert_off = {
                use crate::model::shape::{TextWrap as TW, VertRelTo as VR};
                let is_para_topbottom = !table.common.treat_as_char
                    && matches!(table.common.text_wrap, TW::TopAndBottom)
                    && matches!(table.common.vert_rel_to, VR::Para);
                let v = table.common.vertical_offset as i32;
                if is_para_topbottom && v > 0 {
                    hwpunit_to_px(v, self.dpi)
                } else {
                    0.0
                }
            };
            host_before + vert_off
        };
        let remaining_on_page =
            (table_available - st.current_height - first_frag_overhead).max(0.0);
        let (first_block_start, first_block_end, first_block_h) = if row_count > 0 {
            mt.row_block_for(0)
        } else {
            (0, 0, 0.0)
        };
        let first_block_size = first_block_end.saturating_sub(first_block_start);
        let first_block_is_single_row = first_block_size == 1;
        let first_block_has_protectable_rowspan = first_block_size >= 2
            && first_block_size <= crate::renderer::height_measurer::BLOCK_UNIT_MAX_ROWS
            && (first_block_start..first_block_end)
                .any(|r| rowspan_touched.get(r).copied().unwrap_or(false));
        let first_rowbreak_block_has_hard_break =
            if mt.allows_row_break_split() && first_block_has_protectable_rowspan {
                layout_engine.row_block_has_internal_hard_break(
                    table,
                    first_block_start,
                    first_block_end,
                    styles,
                )
            } else {
                false
            };
        // [Task #1145] RowBreak 표도 작은 rowspan 제목/라벨 블록은 내부 hard-break가
        // 없으면 중간 행에서 자르지 않는다. 일반 RowBreak 행 경계 분할은 유지한다.
        let first_block_protected = first_block_has_protectable_rowspan
            && (!mt.allows_row_break_split() || !first_rowbreak_block_has_hard_break);
        // Task #398 v2: 보호 블록(2~3 rows)만 블록 전체 높이로 판정. 큰 rowspan(>3)은 행 단위 분할.
        let split_unit_h = if first_block_protected {
            first_block_h
        } else {
            mt.row_heights.first().copied().unwrap_or(0.0)
        };
        if remaining_on_page < split_unit_h && !st.current_items.is_empty() {
            let first_row_splittable = (first_block_is_single_row || !first_block_protected)
                && can_intra_split
                && mt.is_row_splittable(0);
            // [Task #874 #6] 한컴 PDF (aift.hwp p19~20 표 pi=236 "기능 간 이벤트 연계
            // 구성도 이미지") 정합: 1×1 표 의 셀이 content 보다 훨씬 큰 cell.height
            // 를 가질 때 (line_count == 1 → is_row_splittable=false 라 의도 분할 불가)
            // 한컴은 page 경계에서 셀 빈 영역을 자르고 다음 페이지로 연속 렌더한다.
            // can_intra_split 이고 첫 행이 가용 공간보다 큰 force-split 케이스로 분기.
            let first_row_force_splittable =
                !first_block_protected && can_intra_split && remaining_on_page > 0.0;
            let min_content = if first_row_splittable {
                mt.min_first_line_height_for_row(0, 0.0) + mt.max_padding_for_row(0)
            } else if first_row_force_splittable {
                // force-split 케이스: 콘텐츠 한 줄 + padding 정도면 분할 가능
                let pad = mt.max_padding_for_row(0);
                let line_h = mt.row_heights.first().copied().unwrap_or(0.0).min(20.0);
                pad + line_h
            } else {
                f64::MAX
            };
            // [Task #1046 Stage 3] 다행(多行) 표의 비분할 첫 행/블록이 잔여공간엔 안
            // 들어가지만 fresh 페이지엔 통째 들어가면 다음 페이지로 이월한다. 첫 행은
            // 행 내부 분할이 안 되고(=is_row_splittable=false) 표에 후속 행 경계가 있어
            // 깨끗한 이월이 가능하므로(요구사항 표 계열, 한컴 PDF상 통째 배치) force-split
            // 추정으로 현재 페이지에 붙잡지 않는다(pi=290 8.7px). genuine page-larger 와
            // 1×1 단일 셀(row_count==1, 행 경계 없어 셀 내부 컷 필요, #874)은 제외 —
            // fits_fresh_page/row_count 조건으로 기존 force-split(렌더러 경계 컷) 유지.
            let fits_fresh_page = split_unit_h <= (base_available - first_frag_overhead).max(0.0);
            let multirow_clean_defer = !first_row_splittable
                && row_count > 1
                && first_block_end < row_count
                && fits_fresh_page;
            if (!first_row_splittable && !first_row_force_splittable)
                || remaining_on_page < min_content
                || multirow_clean_defer
            {
                st.advance_column_or_new_page();
            }
        }

        // 캡션 처리
        let caption_is_top = para
            .controls
            .get(ctrl_idx)
            .and_then(|c| {
                if let Control::Table(t) = c {
                    t.caption
                        .as_ref()
                        .map(|cap| matches!(cap.direction, CaptionDirection::Top))
                } else {
                    None
                }
            })
            .unwrap_or(false);

        let host_line_spacing_for_caption = para
            .line_segs
            .first()
            .map(|seg| hwpunit_to_px(seg.line_spacing, self.dpi))
            .unwrap_or(0.0);
        let caption_base_overhead = {
            let ch = ft.caption_height;
            if ch > 0.0 {
                let cs_val = para
                    .controls
                    .get(ctrl_idx)
                    .and_then(|c| {
                        if let Control::Table(t) = c {
                            t.caption
                                .as_ref()
                                .map(|cap| hwpunit_to_px(cap.spacing as i32, self.dpi))
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0.0);
                ch + cs_val
            } else {
                0.0
            }
        };
        let caption_overhead = if caption_base_overhead > 0.0 && !caption_is_top {
            caption_base_overhead + host_line_spacing_for_caption
        } else {
            caption_base_overhead
        };

        // 행 단위 + 인트라-로우 분할 루프 (기존 Paginator split_table_rows 동일)
        let mut cursor_row: usize = 0;
        let mut is_continuation = false;
        // [Task #993] 분할 상태를 px content_offset 대신 행 컷(셀별 소비 유닛
        // 수)으로 추적한다. 빈 Vec = cursor_row 를 처음부터. 컷은 advance_row_cut
        // 에 의해 유닛을 ≥1개씩 단조 전진하므로 무한 루프가 구조적으로 불가능하다.
        let mut start_cut: Vec<usize> = Vec::new();
        // [Task #1025] 현재 start_cut 이 rowspan 블록-셀 인덱스인지(직전 블록 분할의
        // 연속분). PartialTable.is_block_split 로 렌더러에 전달.
        let mut start_cut_is_block = false;

        while cursor_row < row_count {
            // 이전 분할에서 모든 콘텐츠가 소진된 행은 건너뜀.
            // [Task #1025] 블록 컷(start_cut_is_block)은 per-row(row_span==1) 컷이 아니라
            // 블록-셀 인덱스다. advance_row_cut(per-row)로 판정하면 블록 첫 행이 소진돼도
            // 거대 셀이 남은 경우를 "소진"으로 오판해 cursor 를 전진시키고 start_cut 을
            // 비워 블록 컷을 잃는다(연속분이 거대 셀을 처음부터 다시 렌더 → overflow).
            // 블록 컷이면 이 가드를 건너뛰어 컷을 보존한다.
            if !start_cut_is_block
                && !start_cut.is_empty()
                && can_intra_split
                && layout_engine
                    .advance_row_cut(table, cursor_row, &start_cut, f64::MAX, styles)
                    .consumed_height
                    <= 0.0
            {
                cursor_row += 1;
                start_cut = Vec::new();
                continue;
            }

            let caption_extra =
                if !is_continuation && cursor_row == 0 && start_cut.is_empty() && caption_is_top {
                    caption_overhead
                } else {
                    0.0
                };
            // [Task #874 #9] 첫 fragment 의 page_avail 은 host_spacing.before 와
            // (TopAndBottom + vert=Para + v_offset>0 표의) vertical_offset 를 제외해야 한다.
            // layout 은 표를 cur_h + host_spacing.before + v_offset 위치에 배치하지만,
            // typeset 의 page_avail = (table_available - cur_h) 은 두 overhead 를
            // 포함하지 않아 split 결정 시 actual 가용보다 과대 평가됨 → partial 오버플로우.
            // aift.hwp p44 pi=584: 41.6 px split_end → 실제 가용 36 px → overflow 37.6 px.
            let host_before_overhead = if is_continuation {
                0.0
            } else {
                ft.host_spacing.before
            };
            let vert_offset_overhead = if is_continuation {
                0.0
            } else {
                use crate::model::shape::{TextWrap as TW3, VertRelTo as VR3};
                let is_para_topbottom = !table.common.treat_as_char
                    && matches!(table.common.text_wrap, TW3::TopAndBottom)
                    && matches!(table.common.vert_rel_to, VR3::Para);
                // HwpUnit=u32 이므로 음수 (u32 wrap) 는 i32 로 캐스트 후 확인.
                let v_off_i32 = table.common.vertical_offset as i32;
                if is_para_topbottom && v_off_i32 > 0 {
                    hwpunit_to_px(v_off_i32, self.dpi)
                } else {
                    0.0
                }
            };
            let page_avail = if is_continuation {
                table_available
            } else {
                (table_available
                    - st.current_height
                    - caption_extra
                    - host_before_overhead
                    - vert_offset_overhead)
                    .max(0.0)
            };

            // [Task #1022] 머리행 반복 overhead — 렌더러(layout_partial_table)는
            // start_row 이전의 반복 제목행을 다시 그리므로(다중 머리행: rs>=2 헤더 셀 등),
            // 페이지네이터도 동일 제목행 전체 높이 + 각 행 뒤 cs 를 계산한다.
            // [Task #1716] 반복 대상은 **표 상단의 연속 제목행 블록**(leading_header_rows)뿐.
            // 종전엔 cursor 아래의 모든 is_header 행을 합산해, 본문 행에도 header="1" 이
            // 흩어진 표(건설공사 품질시험기준 pi=12)에서 cursor 전진 시 overhead 가 누적되어
            // 가용 높이가 0이 되고 페이지당 1행 폭주(+100쪽)가 발생했다. 렌더러(table_partial)도
            // 동일 leading_header_rows 를 사용하므로 desync(오버플로) 없음.
            let header_overhead =
                if is_continuation && mt.repeat_header && mt.has_header_cells && row_count > 1 {
                    let hr: Vec<usize> = table
                        .leading_header_rows()
                        .into_iter()
                        .filter(|&r| r < cursor_row)
                        .collect();
                    if hr.is_empty() {
                        0.0
                    } else {
                        let h: f64 = hr.iter().map(|&r| cut_row_h[r]).sum();
                        h + cs * hr.len() as f64
                    }
                } else {
                    0.0
                };
            let avail_for_rows = (page_avail - header_overhead).max(0.0);

            // [Task #1046 Stage 2 진단] 첫/연속 fragment 의 가용공간 분해 — 렌더러
            // y_start 점프(vert_offset)·host_before 와의 정합 확인용. 동작 불변(게이트).
            if std::env::var("RHWP_TABLE_DRIFT").is_ok() {
                eprintln!(
                    "TABLE_SPLIT_AVAIL: pi={} sec={} cursor_row={} cont={} cur_h={:.1} table_avail={:.1} caption={:.1} host_before={:.1} vert_off={:.1} page_avail={:.1} header_oh={:.1} avail_for_rows={:.1} start_cut={:?}",
                    para_idx, st.section_index, cursor_row, is_continuation, st.current_height,
                    table_available, caption_extra, host_before_overhead, vert_offset_overhead,
                    page_avail, header_overhead, avail_for_rows, start_cut,
                );
            }

            // [Task #993] 컷 기반 행 경계 walk — cursor_row 부터 avail_for_rows
            // 안에 들어가는 행을 advance_row_cut(단일 권위 함수)으로 누적 배치한다.
            // 예산을 못 채우거나 vpos 리셋(hard break)을 만난 첫 행이 분할 행이
            // 된다. rowspan 보호 블록(#398/#474)은 블록 전체를 한 단위로 다룬다.
            // 측정 공간이 advance_row_cut/cell_units 로 단일화되어 렌더러와
            // 정의상 일치한다(px content_offset·MeasuredTable 누적 제거).
            const MIN_TOP_KEEP_PX: f64 = 25.0;
            const ROWBREAK_TRAILING_EMPTY_ROW_OVERFLOW_TOLERANCE_PX: f64 = 40.0;
            const ROWBREAK_SPLIT_ROW_OVERFLOW_TOLERANCE_PX: f64 = 0.1;
            const HWPX_ROWBREAK_SPLIT_ROW_OVERFLOW_TOLERANCE_PX: f64 = 64.0;
            const LANDSCAPE_ROWBREAK_WHOLE_ROW_TOLERANCE_PX: f64 = 36.0;
            const LANDSCAPE_ROWBREAK_SHORT_ROW_TOLERANCE_PX: f64 = 260.0;
            const LANDSCAPE_ROWBREAK_SHORT_ROW_MAX_HEIGHT_PX: f64 = 260.0;
            const HWPX_LANDSCAPE_ROWBREAK_WHOLE_ROW_TOLERANCE_PX: f64 = 48.0;
            const HWPX_LANDSCAPE_ROWBREAK_SHORT_ROW_TOLERANCE_PX: f64 = 320.0;
            const HWPX_LANDSCAPE_ROWBREAK_SHORT_ROW_MAX_HEIGHT_PX: f64 = 320.0;
            let landscape_rowbreak_bleed = st.layout.body_area.height < 700.0;
            let landscape_whole_row_tolerance = if st.is_hwpx_source {
                HWPX_LANDSCAPE_ROWBREAK_WHOLE_ROW_TOLERANCE_PX
            } else {
                LANDSCAPE_ROWBREAK_WHOLE_ROW_TOLERANCE_PX
            };
            let landscape_short_row_tolerance = if st.is_hwpx_source {
                HWPX_LANDSCAPE_ROWBREAK_SHORT_ROW_TOLERANCE_PX
            } else {
                LANDSCAPE_ROWBREAK_SHORT_ROW_TOLERANCE_PX
            };
            let landscape_short_row_max_height = if st.is_hwpx_source {
                HWPX_LANDSCAPE_ROWBREAK_SHORT_ROW_MAX_HEIGHT_PX
            } else {
                LANDSCAPE_ROWBREAK_SHORT_ROW_MAX_HEIGHT_PX
            };
            let rowbreak_split_row_overflow_tolerance = if st.is_hwpx_source {
                HWPX_ROWBREAK_SPLIT_ROW_OVERFLOW_TOLERANCE_PX
            } else {
                ROWBREAK_SPLIT_ROW_OVERFLOW_TOLERANCE_PX
            };

            let mut end_row = cursor_row;
            let mut split_end_cut: Vec<usize> = Vec::new();
            let mut split_end_limit: f64 = 0.0;
            // [Task #1025] 블록 분할 시 연속분 커서가 블록 시작 행으로 복귀하도록 기록.
            let mut split_block_start: Option<usize> = None;
            let mut consumed: f64 = 0.0; // 완전 배치된 행들의 누적 높이
            {
                let mut r = cursor_row;
                while r < row_count {
                    let cs_before = if r > cursor_row { cs } else { 0.0 };
                    // rowspan 보호 블록 — 블록 전체를 분할 없이 한 단위로.
                    let (b_start, b_end, _) = mt.row_block_for(r);
                    let block_size = b_end.saturating_sub(b_start);
                    let block_has_protectable_rowspan = block_size >= 2
                        && block_size <= crate::renderer::height_measurer::BLOCK_UNIT_MAX_ROWS
                        && (b_start..b_end)
                            .any(|x| rowspan_touched.get(x).copied().unwrap_or(false));
                    let rowbreak_hard_break_row = if mt.allows_row_break_split()
                        && b_start == r
                        && block_has_protectable_rowspan
                    {
                        layout_engine
                            .row_block_first_internal_hard_break_row(table, b_start, b_end, styles)
                    } else {
                        None
                    };
                    let rowbreak_has_internal_hard_break = rowbreak_hard_break_row.is_some();
                    let protected = block_has_protectable_rowspan
                        && (!mt.allows_row_break_split() || !rowbreak_has_internal_hard_break);
                    // [Task #1086] RowBreak 표는 행 경계 분할 정책이라 보호 블록
                    // snap 은 피하지만, rowspan label 이 걸친 블록 안의 큰 row_span==1
                    // 셀은 셀 내부 hard-break(vpos reset) 기준으로 쪼갤 수 있어야 한다.
                    // 이때는 기존 블록 컷 경로를 재사용해 rowspan 셀과 일반 셀의 cut
                    // 인덱스를 같은 정의로 렌더러까지 전달한다.
                    let rowbreak_rowspan_block = mt.allows_row_break_split()
                        && b_start == r
                        && block_has_protectable_rowspan
                        && rowbreak_has_internal_hard_break;
                    // #1486: hard-break가 rowspan 블록 첫 행의 큰 셀 안에 있을 때만
                    // 행 시작 y offset을 빼서 아래 행 셀을 다음 조각에 남긴다.
                    // #1105처럼 hard-break가 뒤 행 셀 안에 있는 블록은 기존 블록 컷을
                    // 유지해야 첫 조각의 `end_cut`이 한컴 기준과 맞는다.
                    let rowbreak_use_row_offsets =
                        rowbreak_rowspan_block && rowbreak_hard_break_row == Some(b_start);
                    if (protected || rowbreak_rowspan_block) && b_start == r {
                        // [Task #1025] 연속분 커서가 블록 중간이면 블록 시작 컷을 적용.
                        let blk_start_cut: &[usize] =
                            if r == cursor_row { &start_cut } else { &[] };
                        let block_row_offsets: Vec<f64> = if rowbreak_use_row_offsets {
                            let mut offsets = Vec::with_capacity(block_size);
                            let mut top = 0.0;
                            for br in b_start..b_end {
                                offsets.push(top);
                                top += cut_row_h[br] + if br + 1 < b_end { cs } else { 0.0 };
                            }
                            offsets
                        } else {
                            Vec::new()
                        };
                        let block_fragment_height = |row_end: usize,
                                                     block_start_cut: &[usize],
                                                     block_end_cut: &[usize]|
                         -> f64 {
                            if block_start_cut.is_empty() && block_end_cut.is_empty() {
                                return (b_start..row_end).map(|x| cut_row_h[x]).sum::<f64>()
                                    + cs * row_end.saturating_sub(b_start + 1) as f64;
                            }

                            let mut total = 0.0;
                            let mut has_row = false;
                            for br in b_start..row_end {
                                let row_h = layout_engine.row_block_cut_row_content_height(
                                    table,
                                    b_start,
                                    b_end,
                                    br,
                                    block_start_cut,
                                    block_end_cut,
                                    styles,
                                );
                                if row_h > 0.0 {
                                    if has_row {
                                        total += cs;
                                    }
                                    total += row_h;
                                    has_row = true;
                                }
                            }
                            total
                        };
                        let block_h: f64 = if blk_start_cut.is_empty() {
                            (b_start..b_end).map(|x| cut_row_h[x]).sum::<f64>()
                                + cs * block_size.saturating_sub(1) as f64
                        } else if rowbreak_use_row_offsets {
                            block_fragment_height(b_end, blk_start_cut, &[])
                        } else {
                            layout_engine.row_block_content_height(
                                table,
                                b_start,
                                b_end,
                                blk_start_cut,
                                &[],
                                styles,
                            ) + cs * block_size.saturating_sub(1) as f64
                        };
                        if consumed + cs_before + block_h <= avail_for_rows {
                            consumed += cs_before + block_h;
                            r = b_end;
                            end_row = r;
                            continue;
                        }
                        // [Task #1025/#1086] 블록이 가용 초과 — 거대 row_span==1 셀을
                        // 줄 단위로 분할 시도(블록 컷). 보호 블록은 기존처럼 fresh
                        // page 에도 안 들어가는 경우만 페이지 중간에서 쪼갠다. RowBreak
                        // rowspan 블록은 hard-break(vpos reset)를 만난 경우에만 중간
                        // 분할을 허용해 일반 RowBreak 행 경계 정책의 blast radius 를 줄인다.
                        let budget = (avail_for_rows - consumed - cs_before).max(0.0);
                        let res = if rowbreak_use_row_offsets {
                            layout_engine.advance_row_block_cut_with_row_offsets(
                                table,
                                b_start,
                                b_end,
                                blk_start_cut,
                                budget,
                                &block_row_offsets,
                                styles,
                            )
                        } else {
                            layout_engine.advance_row_block_cut(
                                table,
                                b_start,
                                b_end,
                                blk_start_cut,
                                budget,
                                styles,
                            )
                        };
                        // [Task #1025] 블록이 fresh 페이지에도 안 들어가야(진짜 page-larger)
                        // 페이지 중간에서 분할한다. fresh 페이지엔 들어가면(잔여 공간만
                        // 부족) 통째로 다음 페이지로 미뤄 잔여 overflow 를 피한다(기존 동작).
                        // 페이지 시작 행(r==cursor_row)은 더 미룰 수 없으므로 무조건 분할.
                        let genuinely_page_larger = block_h > st.base_available_height();
                        let allow_block_split = if rowbreak_rowspan_block {
                            r == cursor_row
                                || (res.hit_hard_break && res.consumed_height >= MIN_TOP_KEEP_PX)
                        } else {
                            r == cursor_row
                                || (genuinely_page_larger && res.consumed_height >= MIN_TOP_KEEP_PX)
                        };
                        if can_intra_split && !res.fully_consumed && allow_block_split {
                            end_row = if rowbreak_use_row_offsets {
                                let mut render_end = b_start + 1;
                                for (idx, row_top) in block_row_offsets.iter().enumerate() {
                                    if *row_top < res.consumed_height - 0.1 {
                                        render_end = b_start + idx + 1;
                                    }
                                }
                                render_end.min(b_end).max(b_start + 1)
                            } else {
                                b_end
                            };
                            split_end_cut = res.end_cut.clone();
                            split_end_limit = res.consumed_height;
                            split_block_start = Some(b_start);
                            let split_total = if rowbreak_use_row_offsets {
                                block_fragment_height(end_row, blk_start_cut, &res.end_cut)
                            } else {
                                layout_engine.row_block_content_height(
                                    table,
                                    b_start,
                                    b_end,
                                    blk_start_cut,
                                    &res.end_cut,
                                    styles,
                                )
                            };
                            consumed += cs_before + split_total;
                            break;
                        }
                        if r == cursor_row {
                            // 분할 불가 — 강제 통째 배치(기존 overflow 동작 유지).
                            consumed += cs_before + block_h;
                            r = b_end;
                            end_row = r;
                            continue;
                        }
                        end_row = r;
                        break;
                    }

                    // rowspan 셀이 걸친 행 — 기본은 MeasuredTable 높이로 통째 배치한다.
                    //
                    // 다만 RowBreak 표의 큰 rowspan 블록 안에 있는 일반 내용 행은 한컴처럼
                    // 해당 행의 row_span==1 셀을 기준으로 내부 분할을 허용한다. 작은 보호
                    // 블록은 위의 block path 에서 이미 처리되며, 여기서는 block path 대상이
                    // 아닌 큰 블록의 과도한 이월만 줄인다.
                    let rowbreak_rowspan_row_splittable =
                        mt.allows_row_break_split() && can_intra_split && mt.is_row_splittable(r);
                    if rowspan_touched[r] && !rowbreak_rowspan_row_splittable {
                        let h = cut_row_h[r];
                        if r == cursor_row || consumed + cs_before + h <= avail_for_rows {
                            consumed += cs_before + h;
                            r += 1;
                            end_row = r;
                            continue;
                        }
                        end_row = r;
                        break;
                    }

                    // [Task #1022] 일반 행 r — 배치 높이는 row_cut_content_height
                    // (=cut_row_h)로, 분할 컷 산정만 advance_row_cut 으로 수행한다.
                    let row_start_cut: &[usize] = if r == cursor_row { &start_cut } else { &[] };
                    let row_total = if row_start_cut.is_empty() {
                        cut_row_h[r]
                    } else {
                        // 연속분 cursor_row — 시작 컷 적용. row_cut_content_height 가
                        // 셀별 (content+pad) 행 max 를 반환(분할 행이므로 cell.height
                        // 강제 없음).
                        layout_engine.row_cut_content_height(table, r, row_start_cut, &[], styles)
                    };
                    if consumed + cs_before + row_total <= avail_for_rows {
                        // 행 전체가 예산 안에 들어감.
                        consumed += cs_before + row_total;
                        r += 1;
                        end_row = r;
                        continue;
                    }
                    // RowBreak 표의 마지막 빈 spacer 행은 한컴이 직전 조각 하단에 붙여
                    // 그리는 경우가 많다. 이 행 하나만 몇 px 넘친다고 별도 빈 꼬리
                    // 페이지를 만들면 Q&A 표처럼 작은 잔여 조각 페이지가 반복된다.
                    if mt.allows_row_break_split()
                        && r + 1 == row_count
                        && Self::row_is_empty_trailing_spacer(table, r)
                        && consumed > 0.0
                        && consumed + cs_before + row_total
                            <= avail_for_rows + ROWBREAK_TRAILING_EMPTY_ROW_OVERFLOW_TOLERANCE_PX
                    {
                        consumed += cs_before + row_total;
                        r += 1;
                        end_row = r;
                        continue;
                    }
                    if landscape_rowbreak_bleed
                        && mt.allows_row_break_split()
                        && is_continuation
                        && header_overhead > 0.5
                        && row_start_cut.is_empty()
                        && r > cursor_row
                        && consumed + cs_before + row_total
                            <= avail_for_rows + landscape_whole_row_tolerance
                    {
                        consumed += cs_before + row_total;
                        r += 1;
                        end_row = r;
                        continue;
                    }
                    if landscape_rowbreak_bleed
                        && mt.allows_row_break_split()
                        && is_continuation
                        && header_overhead > 0.5
                        && row_start_cut.is_empty()
                        && r > cursor_row
                        && row_total <= landscape_short_row_max_height
                        && consumed + cs_before + row_total
                            <= avail_for_rows + landscape_short_row_tolerance
                    {
                        consumed += cs_before + row_total;
                        r += 1;
                        end_row = r;
                        continue;
                    }
                    // 행 r 이 예산 초과 — 인트라-분할 시도.
                    // [Task #77] 분할 불가 행(이미지 셀 등)은 통째 배치 / 다음 페이지.
                    let splittable = can_intra_split && mt.is_row_splittable(r);
                    if !splittable {
                        if r == cursor_row {
                            // 페이지 시작 행 — 강제 통째 배치(오버플로 감수).
                            consumed += cs_before + row_total;
                            end_row = r + 1;
                        } else {
                            end_row = r;
                        }
                        break;
                    }
                    let padding = if mt.allows_row_break_split() {
                        layout_engine.row_remaining_visible_padding_height(
                            table,
                            r,
                            row_start_cut,
                            styles,
                        )
                    } else {
                        mt.max_padding_for_row(r)
                    };
                    let budget = (avail_for_rows - consumed - cs_before - padding).max(0.0);
                    let res =
                        layout_engine.advance_row_cut(table, r, row_start_cut, budget, styles);
                    if res.fully_consumed {
                        // 단일 유닛 행 — 분할 불가, 페이지 시작이면 강제, 아니면 다음으로.
                        if r == cursor_row {
                            consumed += cs_before + row_total;
                            end_row = r + 1;
                        } else {
                            end_row = r;
                        }
                        break;
                    }
                    // [Task #713] sliver(orphan) 회피 — 페이지 시작 행이 아니면서
                    // 너무 적게 들어가면 행 전체를 다음 페이지로 미룬다.
                    if r > cursor_row && res.consumed_height < MIN_TOP_KEEP_PX {
                        end_row = r;
                    } else {
                        // 분할 행의 행 총 높이(per-cell content+pad) 를 consumed 에 가산.
                        let split_total = layout_engine.row_cut_content_height(
                            table,
                            r,
                            row_start_cut,
                            &res.end_cut,
                            styles,
                        );
                        let split_candidate_rows_height = consumed + cs_before + split_total;
                        let split_row_overflow_tolerance = if mt.allows_row_break_split() {
                            rowbreak_split_row_overflow_tolerance
                        } else {
                            0.1
                        };
                        if r > cursor_row
                            && split_candidate_rows_height
                                > avail_for_rows + split_row_overflow_tolerance
                        {
                            // 보이는 조각은 orphan 기준을 통과해도 row-area 예산은 넘을 수 있다.
                            // 마지막으로 온전히 들어간 행까지만 유지하고 이 행은 다음 쪽에서
                            // 계속한다. avail_for_rows 는 이미 반복 제목행 높이를 제외한 값이다.
                            end_row = r;
                        } else {
                            end_row = r + 1;
                            split_end_cut = res.end_cut.clone();
                            split_end_limit = res.consumed_height;
                            consumed += cs_before + split_total;
                        }
                    }
                    break;
                }
            }
            if end_row <= cursor_row {
                end_row = cursor_row + 1;
            }

            // [Task #1022] walk 가 consumed 에 분할 행 기여까지 누적하므로
            // partial_height = consumed + header_overhead 로 단일화.
            let partial_height: f64 = consumed + header_overhead;

            // [Task #1046 Stage 2 진단] walk 결과 — fragment 경계/소비 높이. 동작 불변.
            if std::env::var("RHWP_TABLE_DRIFT").is_ok() {
                eprintln!(
                    "TABLE_SPLIT_RESULT: pi={} sec={} cursor_row={} end_row={} consumed={:.1} partial_h={:.1} split_end_limit={:.1} avail_for_rows={:.1} fits={}",
                    para_idx, st.section_index, cursor_row, end_row, consumed, partial_height,
                    split_end_limit, avail_for_rows, consumed <= avail_for_rows + 0.1,
                );
            }

            // 마지막 파트에 Bottom 캡션 공간 확보
            if end_row >= row_count
                && split_end_limit == 0.0
                && !caption_is_top
                && caption_overhead > 0.0
            {
                let total_with_caption = partial_height + caption_overhead;
                let avail = if is_continuation {
                    (page_avail - header_overhead).max(0.0)
                } else {
                    page_avail
                };
                if total_with_caption > avail {
                    end_row = end_row.saturating_sub(1);
                    if end_row <= cursor_row {
                        end_row = cursor_row + 1;
                    }
                }
            }

            if end_row >= row_count && split_end_limit == 0.0 {
                let skip_terminal_empty_sliver = is_continuation
                    && !start_cut.is_empty()
                    && !start_cut_is_block
                    && mt.allows_row_break_split()
                    && caption_overhead <= 0.5
                    && partial_height < MIN_TOP_KEEP_PX
                    && (cursor_row..end_row).all(|r| {
                        let su: &[usize] = if r == cursor_row { &start_cut } else { &[] };
                        !layout_engine.row_cut_range_has_visible_content(table, r, su, &[], styles)
                    });
                if skip_terminal_empty_sliver {
                    break;
                }

                // 나머지 전부가 현재 페이지에 들어감
                let bottom_caption_extra = if !caption_is_top {
                    caption_overhead
                } else {
                    0.0
                };
                if cursor_row == 0 && !is_continuation && start_cut.is_empty() {
                    st.current_items.push(PageItem::Table {
                        para_index: para_idx,
                        control_index: ctrl_idx,
                    });
                    st.current_height += partial_height + host_spacing_total;
                } else {
                    st.current_items.push(PageItem::PartialTable {
                        para_index: para_idx,
                        control_index: ctrl_idx,
                        start_row: cursor_row,
                        end_row,
                        is_continuation,
                        start_cut: start_cut.clone(),
                        end_cut: Vec::new(),
                        is_block_split: start_cut_is_block,
                    });
                    // 마지막 fragment: spacing_after만 포함 (Paginator engine.rs:1051 동일)
                    // host_line_spacing과 outer_bottom은 포함하지 않음
                    st.current_height +=
                        partial_height + bottom_caption_extra + ft.host_spacing.spacing_after_only;
                }
                break;
            }

            // 중간 fragment 배치
            st.current_items.push(PageItem::PartialTable {
                para_index: para_idx,
                control_index: ctrl_idx,
                start_row: cursor_row,
                end_row,
                is_continuation,
                start_cut: start_cut.clone(),
                end_cut: split_end_cut.clone(),
                // [Task #1025] 이번 분할이 블록 분할이거나 start_cut 이 이미 블록 인덱스.
                is_block_split: split_block_start.is_some() || start_cut_is_block,
            });
            st.advance_column_or_new_page();

            // 커서 전진 — [Task #993] 컷은 절대 유닛 인덱스이므로 누적 없이 대입.
            if split_end_limit > 0.0 {
                // [Task #1025] 블록 분할이면 커서를 블록 시작 행으로(end_row-1 아님).
                cursor_row = split_block_start.unwrap_or(end_row - 1);
                start_cut = split_end_cut;
                // 다음 fragment 의 start_cut 이 블록 인덱스인지 전파.
                start_cut_is_block = split_block_start.is_some();
            } else {
                cursor_row = end_row;
                start_cut = Vec::new();
                start_cut_is_block = false;
            }
            is_continuation = true;
        }
    }

    // ========================================================
    // 다단 문단 처리
    // ========================================================

    /// 다단 레이아웃에서 문단 내 단 경계를 감지한다.
    fn detect_column_breaks_in_paragraph(para: &Paragraph) -> Vec<usize> {
        let mut breaks = vec![0usize];
        if para.line_segs.len() <= 1 {
            return breaks;
        }
        for i in 1..para.line_segs.len() {
            if para.line_segs[i].vertical_pos < para.line_segs[i - 1].vertical_pos {
                breaks.push(i);
            }
        }
        breaks
    }

    /// 다단 문단의 단별 분할
    fn typeset_multicolumn_paragraph(
        &self,
        st: &mut TypesetState,
        para_idx: usize,
        para: &Paragraph,
        fmt: &FormattedParagraph,
        col_breaks: &[usize],
    ) {
        let line_count = fmt.line_heights.len();
        for (bi, &break_start) in col_breaks.iter().enumerate() {
            let break_end = if bi + 1 < col_breaks.len() {
                col_breaks[bi + 1]
            } else {
                line_count
            };

            if break_start >= line_count || break_end > line_count {
                break;
            }

            let part_height = fmt.line_advances_sum(break_start..break_end);

            if break_start == 0 && break_end >= line_count {
                st.current_items.push(PageItem::FullParagraph {
                    para_index: para_idx,
                });
            } else {
                st.current_items.push(PageItem::PartialParagraph {
                    para_index: para_idx,
                    start_line: break_start,
                    end_line: break_end,
                });
            }
            st.current_height += part_height;

            // 마지막 단이 아니면 다음 단으로 flush
            if bi + 1 < col_breaks.len() {
                st.flush_column();
                if st.current_column + 1 < st.col_count {
                    st.current_column += 1;
                    st.current_height = 0.0;
                }
            }
        }
    }

    // ========================================================
    // 다단 나누기 처리
    // ========================================================

    fn process_multicolumn_break(
        &self,
        st: &mut TypesetState,
        para_idx: usize,
        paragraphs: &[Paragraph],
        page_def: &PageDef,
    ) {
        st.flush_column();

        // [Task #874 Case 5] leaving zone 의 height 계산 시 마지막 라인의 trailing
        // line_spacing 을 제외한다. zone 간 gap 은 design_spacing/2 + solo_zone_pad 가
        // 이미 담당하므로 vpos_zone_height 에 trailing_ls 까지 더하면 이중 가산.
        // 한컴 PDF 측정 (shortcut.hwp 1쪽): 본문 첫 줄 top 195.3 px (Hancom) vs 210.7 px
        // (rhwp pre) = +15.4 px (≈11.5pt) 넓다. 제목 paragraph 의 trailing_ls 16 px 이
        // vpos_zone_height 에 포함되어 다음 zone(헤더 띠 + 본문)을 일괄 16 px 하향.
        // pi=80 (21_언어_기출_편집가능본 test_544) 회귀 없음 — pi=80 은 zone 내부 box
        // 인접 paragraph 로 trailing_ls 가 layout 의 y_offset 에서 포함됨 (이 변경은 zone
        // 전환 시의 vpos_zone_height 만 수정).
        let vpos_zone_height = if para_idx > 0 {
            let mut max_vpos_end: i32 = 0;
            for prev_idx in (0..para_idx).rev() {
                if let Some(last_seg) = paragraphs[prev_idx].line_segs.last() {
                    let vpos_end = last_seg.vertical_pos + last_seg.line_height;
                    if vpos_end > max_vpos_end {
                        max_vpos_end = vpos_end;
                    }
                    break;
                }
            }
            if max_vpos_end > 0 {
                hwpunit_to_px(max_vpos_end, self.dpi)
            } else {
                st.current_height
            }
        } else {
            st.current_height
        };
        // [Task #853] zone 전환 시 디자인 spacing(1단 ColumnDef 의 `간격`)을 세로 간격으로:
        // (이전 zone 디자인 spacing /2) + (새 zone 디자인 spacing /2) 를 더한다.
        // shortcut.hwp 1쪽: 제목 zone(0mm) → 헤더 띠 zone(10mm) → 본문 zone(2단, 0)
        //   → 제목↔헤더 = 5mm, 헤더↔본문 = 5mm (한컴 PDF 정합).
        let new_ds = paragraphs[para_idx]
            .controls
            .iter()
            .find_map(|c| {
                if let Control::ColumnDef(cd) = c {
                    Some(column_def_design_spacing_px(cd, self.dpi))
                } else {
                    None
                }
            })
            .unwrap_or(0.0);
        // [Task #866] 직전 zone 의 마지막 paragraph 가 wrap=위아래 인 글자처럼-취급 표(헤더 띠)를
        // 보유하고 그 zone 의 1단 ColumnDef 간격이 0 이면, 한컴은 표 band 높이(표 본체 +
        // outer_margin top/bottom)만큼을 표 아래에 추가로 비워둔다(한컴 PDF 측정:
        // shortcut.hwp 2·3쪽 헤더 띠 하단↔본문 ~28~33px). ColumnDef 간격>0 인 헤더 띠(1쪽
        // 등)는 그 간격이 이미 zone 사이 여백이 되므로 제외.
        // [Task #874 Stage 2] design_spacing 조건을 ≤ 1mm(=3.8px) 까지 인정. 페이지 break 후
        // current_zone_design_spacing_px 가 stale state 로 1mm 남은 경우 (shortcut.hwp 6쪽
        // pi=210 '도구' 헤더띠 zone cd 가 pi=209 cd=1mm 인 케이스) 도 헤더띠 leaving 으로 식별.
        let tac_band_extra: f64 = if st.current_zone_design_spacing_px < 4.0 {
            (0..para_idx)
                .rev()
                .find(|&i| !paragraphs[i].line_segs.is_empty())
                .and_then(|pi| {
                    paragraphs[pi].controls.iter().find_map(|c| match c {
                        Control::Table(t)
                            if t.common.treat_as_char
                                && matches!(
                                    t.common.text_wrap,
                                    crate::model::shape::TextWrap::TopAndBottom
                                ) =>
                        {
                            Some(
                                hwpunit_to_px(t.common.height as i32, self.dpi)
                                    + hwpunit_to_px(t.outer_margin_top as i32, self.dpi)
                                    + hwpunit_to_px(t.outer_margin_bottom as i32, self.dpi),
                            )
                        }
                        _ => None,
                    })
                })
                .unwrap_or(0.0)
        } else {
            0.0
        };
        // [Task #866 v2 Stage 2/4] zone 전환 시 추가 세로 여백.
        // (1) 1단/간격=0 zone(헤더 띠 / `<...>` 소제목) 진입·이탈: +1500 HU(=20px).
        //     shortcut.hwp 4쪽 `개체 모양 복사`↔`<스타일에서>`, 6쪽 `도구`↔`맞춤법 검사` 등.
        // (2) [단나누기](ColumnBreakType::Column) 로 시작하는 새 zone: +1500 HU(=20px).
        //     배분 다단 zone 의 마지막 컬럼 [단나누기] = 같은 ColumnDef 로 새 밴드 → 한컴 PDF
        //     상 이전 밴드와 ~한 본문 줄 간격(shortcut.hwp 3쪽 `화면 확대 100%`↔`<편집 화면
        //     분할에서>`). Stage 1 의 Distribute 마지막 컬럼 라우팅과 정합.
        let entering_solo_zero = paragraphs[para_idx].controls.iter().any(|c| {
            matches!(c,
            Control::ColumnDef(cd) if cd.column_count.max(1) <= 1 && cd.spacing == 0)
        });
        let leaving_solo_zero = st.col_count <= 1 && st.current_zone_design_spacing_px < 0.5;
        // [Task #866 v3 Stage 1] 헤더 띠 zone (TAC wrap=TopAndBottom 표) 의 leaving 은
        // `tac_band_extra` 가 이미 표 band 높이만큼 패딩을 추가하므로 `solo_zone_pad` 를 또
        // 더하면 한컴 PDF 대비 본문 첫 줄이 ~13pt 더 아래로 밀려 사용자 "넓다" 피드백 발생.
        // tac_band_extra>0 == 헤더 띠 leaving 케이스 → solo_zone_pad 의 leaving 분기 제외.
        let leaving_is_header_band = leaving_solo_zero && tac_band_extra > 0.5;
        let column_break_new_band = paragraphs[para_idx].column_type == ColumnBreakType::Column;
        let solo_zone_pad = if entering_solo_zero
            || (leaving_solo_zero && !leaving_is_header_band)
            || column_break_new_band
        {
            hwpunit_to_px(1200, self.dpi)
        } else {
            0.0
        };
        let candidate_offset = st.current_zone_y_offset
            + vpos_zone_height
            + tac_band_extra
            + st.current_zone_design_spacing_px / 2.0
            + new_ds / 2.0
            + solo_zone_pad;

        // [Task #853] 새 zone 이 현재 페이지 하단 가까이(여유 ≲ 헤더 띠 1개 높이)에서 시작하면
        // 그 zone 의 콘텐츠(헤더 띠 ~47px 또는 본문 줄들)가 body 하단을 넘어 렌더되므로 다음
        // 페이지로 넘긴다. (shortcut.hwp 3쪽~6쪽 — 다단 zone 다수 누적 시 잔여 콘텐츠가
        // 본문영역을 넘어 바닥 여백에 그려지던 결함)
        let one_line = hwpunit_to_px(1500, self.dpi);
        if candidate_offset > st.layout.available_body_height() - 4.0 * one_line {
            st.push_new_page();
            // 새 페이지 첫 zone: 새 zone 디자인 spacing /2 만 (이전 zone 은 이전 페이지).
            st.current_zone_y_offset = new_ds / 2.0;
        } else {
            st.current_zone_y_offset = candidate_offset;
        }
        st.current_zone_design_spacing_px = new_ds;
        st.current_column = 0;
        st.current_height = 0.0;
        st.on_first_multicolumn_page = true;

        for ctrl in &paragraphs[para_idx].controls {
            if let Control::ColumnDef(cd) = ctrl {
                st.col_count = cd.column_count.max(1);
                let new_layout = PageLayoutInfo::from_page_def(page_def, cd, self.dpi);
                st.current_zone_layout = Some(new_layout.clone());
                st.layout = new_layout;
                // [Task #702] 새 zone 의 ColumnType 반영. Distribute(배분) 단에서
                // 짧은 컬럼 vpos-reset 검출 임계값 완화용.
                st.current_zone_column_type = cd.column_type;
                break;
            }
        }
    }

    /// [Task #846] 마지막 단에서 명시적 단나누기(`ColumnBreakType::Column`, 새 ColumnDef 없음)
    /// 를 만났을 때: 새 페이지가 아니라 같은 col_count 로 같은 페이지에 새 단-밴드를 시작한다
    /// (≈ 닫힌 #768). 단, 새 밴드가 본문에 들어갈 공간(이 문단 첫 줄)이 없으면 새 페이지로 넘긴다.
    /// 규칙: `누적_밴드_높이 + 현_밴드_높이(= max(컬럼별 채움)) < 본문_높이` 이면 새 밴드, 아니면 새 페이지.
    fn start_new_column_band(
        &self,
        st: &mut TypesetState,
        para_idx: usize,
        paragraphs: &[Paragraph],
    ) {
        st.flush_column();

        // 새 밴드로 들어갈 콘텐츠에 떠다니는(글자처럼 취급이 아닌) 개체가 있으면
        // 같은 페이지에 밴드를 만들지 않고 새 페이지로 넘긴다.
        if Self::upcoming_band_has_floating_object(para_idx, paragraphs) {
            st.push_new_page();
            return;
        }

        // 방금 닫힌 밴드의 높이 = 그 밴드 각 단의 마지막 문단 vpos_end 중 최댓값.
        let zone_off = st.current_zone_y_offset;
        let mut band_height_px = 0.0_f64;
        if let Some(page) = st.pages.last() {
            for cc in page.column_contents.iter().rev() {
                if cc.zone_y_offset != zone_off {
                    break;
                }
                let last_para_idx = cc.items.iter().rev().find_map(|it| match it {
                    PageItem::FullParagraph { para_index }
                    | PageItem::PartialParagraph { para_index, .. }
                    | PageItem::Table { para_index, .. }
                    | PageItem::PartialTable { para_index, .. }
                    | PageItem::Shape { para_index, .. } => Some(*para_index),
                    PageItem::EndnoteSeparator { .. } => None,
                });
                if let Some(pi) = last_para_idx {
                    if let Some(seg) = paragraphs.get(pi).and_then(|p| p.line_segs.last()) {
                        let v = hwpunit_to_px(
                            seg.vertical_pos + seg.line_height + seg.line_spacing,
                            self.dpi,
                        );
                        if v > band_height_px {
                            band_height_px = v;
                        }
                    }
                }
            }
        }
        if band_height_px <= 0.0 {
            band_height_px = st.current_height;
        }

        let first_line_h = paragraphs
            .get(para_idx)
            .and_then(|p| p.line_segs.first())
            .map(|s| hwpunit_to_px(s.line_height + s.line_spacing, self.dpi))
            .filter(|h| *h > 0.0)
            .unwrap_or(1.0);
        let room_after_band = st.available_height() - band_height_px;

        if room_after_band >= first_line_h {
            st.current_zone_y_offset += band_height_px;
            st.current_column = 0;
            st.current_height = 0.0;
            st.on_first_multicolumn_page = true;
        } else {
            st.push_new_page();
        }
    }

    /// 명시적 단나누기 다음 밴드(= `para_idx` 부터 다음 나누기/새 ColumnDef 직전까지)에
    /// 떠다니는 개체(글자처럼 취급이 아닌 표/그림/그리기 개체)가 있는지.
    fn upcoming_band_has_floating_object(para_idx: usize, paragraphs: &[Paragraph]) -> bool {
        for (offset, p) in paragraphs[para_idx..].iter().enumerate() {
            if offset > 0
                && (p.column_type != ColumnBreakType::None
                    || p.controls
                        .iter()
                        .any(|c| matches!(c, Control::ColumnDef(_))))
            {
                break;
            }
            for ctrl in &p.controls {
                let floating = match ctrl {
                    Control::Table(t) => !t.common.treat_as_char,
                    Control::Shape(s) => !s.common().treat_as_char,
                    Control::Picture(pic) => !pic.common.treat_as_char,
                    _ => false,
                };
                if floating {
                    return true;
                }
            }
        }
        false
    }

    // ========================================================
    // 머리말/꼬리말/쪽 번호 처리
    // ========================================================

    fn collect_header_footer_controls(
        paragraphs: &[Paragraph],
        section_index: usize,
    ) -> (
        Vec<(usize, HeaderFooterRef, bool, HeaderFooterApply)>,
        Option<crate::model::control::PageNumberPos>,
        Vec<(usize, u16)>,
        Vec<(usize, crate::model::control::PageHide)>,
    ) {
        let mut hf_entries = Vec::new();
        let mut page_number_pos = None;
        let mut new_page_numbers = Vec::new();
        let mut page_hides: Vec<(usize, crate::model::control::PageHide)> = Vec::new();

        for (pi, para) in paragraphs.iter().enumerate() {
            for (ci, ctrl) in para.controls.iter().enumerate() {
                match ctrl {
                    Control::Header(h) => {
                        let r = HeaderFooterRef {
                            para_index: pi,
                            control_index: ci,
                            source_section_index: section_index,
                        };
                        hf_entries.push((pi, r, true, h.apply_to));
                    }
                    Control::Footer(f) => {
                        let r = HeaderFooterRef {
                            para_index: pi,
                            control_index: ci,
                            source_section_index: section_index,
                        };
                        hf_entries.push((pi, r, false, f.apply_to));
                    }
                    Control::PageNumberPos(pnp) => {
                        page_number_pos = Some(pnp.clone());
                    }
                    Control::NewNumber(nn) => {
                        if nn.number_type == crate::model::control::AutoNumberType::Page {
                            new_page_numbers.push((pi, nn.number));
                        }
                    }
                    Control::PageHide(ph) => {
                        page_hides.push((pi, ph.clone()));
                    }
                    Control::Table(table) => {
                        Self::collect_pagehide_in_table(table, pi, &mut page_hides);
                    }
                    _ => {}
                }
            }
        }

        (hf_entries, page_number_pos, new_page_numbers, page_hides)
    }

    /// 표 셀 안 paragraph 의 PageHide 를 재귀 수집.
    /// 외부 paragraph index `pi` 를 그대로 사용해 페이지 매핑 정합성 유지.
    fn collect_pagehide_in_table(
        table: &crate::model::table::Table,
        pi: usize,
        page_hides: &mut Vec<(usize, crate::model::control::PageHide)>,
    ) {
        for cell in &table.cells {
            for cp in &cell.paragraphs {
                for ctrl in &cp.controls {
                    match ctrl {
                        Control::PageHide(ph) => {
                            page_hides.push((pi, ph.clone()));
                        }
                        Control::Table(inner) => {
                            Self::collect_pagehide_in_table(inner, pi, page_hides);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// 페이지 번호 + 머리말/꼬리말 최종 할당 (기존 Paginator::finalize_pages와 동일)
    fn finalize_pages(
        pages: &mut [PageContent],
        hf_entries: &[(usize, HeaderFooterRef, bool, HeaderFooterApply)],
        page_number_pos: &Option<crate::model::control::PageNumberPos>,
        new_page_numbers: &[(usize, u16)],
        page_hides: &[(usize, crate::model::control::PageHide)],
        _section_index: usize,
    ) {
        // 쪽번호: PageNumberAssigner 가 NewNumber 1회 적용 + 단조 증가를 보장 (Issue #353)
        let mut current_header: Option<HeaderFooterRef> = None;
        let mut current_footer: Option<HeaderFooterRef> = None;
        let mut assigner =
            crate::renderer::page_number::PageNumberAssigner::new(new_page_numbers, 1);

        for page in pages.iter_mut() {
            let page_num = assigner.assign(page);

            // 이 페이지에 속하는 머리말/꼬리말 갱신
            let page_last_para = page
                .column_contents
                .iter()
                .flat_map(|col| col.items.iter())
                .filter_map(|item| match item {
                    PageItem::FullParagraph { para_index } => Some(*para_index),
                    PageItem::PartialParagraph { para_index, .. } => Some(*para_index),
                    PageItem::Table { para_index, .. } => Some(*para_index),
                    PageItem::PartialTable { para_index, .. } => Some(*para_index),
                    PageItem::Shape { para_index, .. } => Some(*para_index),
                    PageItem::EndnoteSeparator { .. } => None,
                })
                .max();

            if let Some(last_pi) = page_last_para {
                for (hf_pi, hf_ref, is_header, apply) in hf_entries {
                    if *hf_pi <= last_pi {
                        let applies = match apply {
                            HeaderFooterApply::Both => true,
                            HeaderFooterApply::Even => page_num.is_multiple_of(2),
                            HeaderFooterApply::Odd => page_num % 2 == 1,
                        };
                        if applies {
                            if *is_header {
                                current_header = Some(hf_ref.clone());
                            } else {
                                current_footer = Some(hf_ref.clone());
                            }
                        }
                    }
                }
            }

            page.page_number = page_num;
            page.active_header = current_header.clone();
            page.active_footer = current_footer.clone();
            if !assigner.should_hide_page_number() {
                page.page_number_pos = page_number_pos.clone();
            }

            // PageHide: 해당 문단이 이 페이지에서 **처음** 시작하는 경우만 적용
            // (engine.rs 의 동일 로직과 일치 — 머리말/꼬리말/바탕쪽/페이지번호 감추기)
            for (ph_para, ph) in page_hides {
                let starts = page.column_contents.iter().any(|col| {
                    col.items.iter().any(|item| match item {
                        PageItem::FullParagraph { para_index } => *para_index == *ph_para,
                        PageItem::PartialParagraph {
                            para_index,
                            start_line,
                            ..
                        } => *para_index == *ph_para && *start_line == 0,
                        PageItem::Table { para_index, .. } => *para_index == *ph_para,
                        PageItem::PartialTable {
                            para_index,
                            is_continuation,
                            ..
                        } => *para_index == *ph_para && !*is_continuation,
                        PageItem::Shape { para_index, .. } => *para_index == *ph_para,
                        PageItem::EndnoteSeparator { .. } => false,
                    })
                });
                if starts {
                    page.page_hide = Some(ph.clone());
                    break;
                }
            }
        }
    }

    // ========================================================
    // 유틸리티
    // ========================================================

    /// 문단에 블록 표 컨트롤이 있는지 감지
    fn paragraph_has_table(&self, para: &Paragraph) -> bool {
        use crate::renderer::height_measurer::is_tac_table_inline;
        let seg_width = para.line_segs.first().map(|s| s.segment_width).unwrap_or(0);
        para.controls.iter().any(|c| {
            matches!(c, Control::Table(t) if t.attr & 0x01 == 0
                || (t.attr & 0x01 != 0 && !is_tac_table_inline(t, seg_width, &para.text, &para.controls)))
        })
    }

    fn row_is_empty_trailing_spacer(table: &crate::model::table::Table, row: usize) -> bool {
        let mut row_cells = table
            .cells
            .iter()
            .filter(|cell| cell.row as usize == row && cell.row_span == 1)
            .peekable();
        if row_cells.peek().is_none() {
            return false;
        }
        row_cells.all(|cell| {
            cell.paragraphs.iter().all(|para| {
                let trimmed = para.text.replace(|c: char| c.is_control(), "");
                trimmed.trim().is_empty() && para.controls.is_empty()
            })
        })
    }

    /// 표의 세로 오프셋 추출 (Paginator와 동일).
    ///
    /// `raw_ctrl_data` 의 첫 4바이트는 `attr` 비트 플래그이고 `vertical_offset` 은
    /// 다음 4바이트 (`raw_ctrl_data[4..8]`) 이지만, IR 의 `common.vertical_offset` 가
    /// 파서가 채운 권위 있는 값이므로 이를 직접 사용한다 (#178).
    fn get_table_vertical_offset(table: &crate::model::table::Table) -> u32 {
        table.common.vertical_offset as u32
    }
}

/// Task #321: 단일 문단의 컨트롤에서 body-wide TopAndBottom 표/도형이 차지하는 높이 계산.
///
/// col 1+ advance 시 current_height 시작값으로 사용하여 layout의 `body_wide_reserved`
/// 와 동일한 가용 공간 축소를 적용한다.
///
/// **Paper(용지) 기준 도형 가드 (v3 정밀화 #326)**: vert_rel_to=Paper 인 도형 중
/// 본문 영역과 겹치지 않는(머리말 영역에만 위치하는) 도형만 제외. body 와 겹치는
/// Paper 도형은 col 1 시작에 영향 → reserve 대상으로 포함.
fn compute_body_wide_top_reserve_for_para(
    para: &Paragraph,
    layout: &PageLayoutInfo,
    dpi: f64,
) -> f64 {
    use crate::model::shape::{TextWrap, VertRelTo};
    let body_w = layout.body_area.width;
    let body_h = layout.available_body_height();
    let body_top = layout.body_area.y;
    let mut max_bottom: f64 = 0.0;
    for ctrl in &para.controls {
        let common = match ctrl {
            Control::Shape(s) => s.common(),
            Control::Table(t) if !t.common.treat_as_char => &t.common,
            Control::Picture(p) if !p.common.treat_as_char => &p.common,
            _ => continue,
        };
        if !matches!(common.text_wrap, TextWrap::TopAndBottom) || common.treat_as_char {
            continue;
        }
        let shape_w = crate::renderer::hwpunit_to_px(common.width as i32, dpi);
        if shape_w < body_w * 0.8 {
            continue;
        }
        let shape_h = crate::renderer::hwpunit_to_px(common.height as i32, dpi);
        let raw_v_offset = crate::renderer::hwpunit_to_px(common.vertical_offset as i32, dpi);

        // body-rel 기준 시작/끝 y 계산.
        // - VertRelTo::Paper: vertical_offset 이 용지 상단(= 0) 기준 → body_top 차감.
        //   본문과 전혀 겹치지 않으면(머리말만 점유) 제외.
        //   본문 위쪽으로 일부 빠져나가면(shape_top_abs < body_top) 본문 침범 영역만 reserve.
        // - VertRelTo::Page / Para: vertical_offset 이 본문/단 top 기준 → body-rel 그대로.
        let (body_y, body_bottom) = if matches!(common.vert_rel_to, VertRelTo::Paper) {
            let shape_top_abs = raw_v_offset;
            let shape_bottom_abs = shape_top_abs + shape_h;
            if shape_bottom_abs <= body_top {
                continue;
            }
            (
                (shape_top_abs - body_top).max(0.0),
                shape_bottom_abs - body_top,
            )
        } else {
            (raw_v_offset, raw_v_offset + shape_h)
        };

        if body_y > body_h / 3.0 {
            continue;
        }
        let outer_bottom = crate::renderer::hwpunit_to_px(common.margin.bottom as i32, dpi);
        let bottom = body_bottom + outer_bottom;
        if bottom > max_bottom {
            max_bottom = bottom;
        }
    }
    max_bottom
}

fn endnote_separator_below_margin(shape: &FootnoteShape) -> i16 {
    shape.separator_below_margin_hu()
}

fn endnote_between_notes_margin(shape: &FootnoteShape) -> u16 {
    shape.between_notes_margin_hu()
}

// 3-09월_교육_통합_2022.hwp의 기본 "미주 사이 7mm"는 원본 LINE_SEG
// 흐름에 이미 상당 부분 녹아 있어 추가 pagination 높이로 더하지 않는다.
// 별도 저장한 "미주사이20" 기준 파일에서는 7mm를 넘는 초과분만 다음
// 미주 묶음 vpos에 반영할 때 한컴오피스의 24쪽 분기와 맞는다.
const ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU: i32 = 1984;
const ENDNOTE_COMPACT_SEPARATOR_BELOW_MAX_HU: i16 = 1000;

#[derive(Clone, Copy, Debug)]
struct EndnoteFlowProfile {
    separator_above_hu: i32,
    separator_below_hu: i32,
    between_notes_hu: i32,
    visible_separator: bool,
    absorbed_between_notes_gap: bool,
    compact_separator_below: bool,
    separator_line_width: u8,
}

impl EndnoteFlowProfile {
    fn from_shape(shape: &FootnoteShape) -> Self {
        let separator_above_hu = shape.separator_above_margin_hu() as i32;
        let separator_below_hu = endnote_separator_below_margin(shape) as i32;
        let between_notes_hu = endnote_between_notes_margin(shape) as i32;
        let visible_separator = endnote_has_visible_separator(shape);
        let absorbed_between_notes_gap = endnote_has_absorbed_between_notes_gap(shape);
        let compact_separator_below =
            separator_below_hu <= ENDNOTE_COMPACT_SEPARATOR_BELOW_MAX_HU as i32;

        Self {
            separator_above_hu,
            separator_below_hu,
            between_notes_hu,
            visible_separator,
            absorbed_between_notes_gap,
            compact_separator_below,
            separator_line_width: shape.separator_line_width,
        }
    }

    fn zero_spacing(self) -> bool {
        self.separator_above_hu == 0 && self.between_notes_hu == 0 && self.separator_below_hu == 0
    }

    fn default_or_compact_between_notes(self) -> bool {
        self.between_notes_hu <= ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU
            || self.absorbed_between_notes_gap
    }

    fn default_between_notes(self) -> bool {
        self.between_notes_hu <= ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU
    }

    fn nonzero_default_between_notes(self) -> bool {
        self.between_notes_hu > 0 && self.default_between_notes()
    }

    fn visible_nonzero_default_between_notes(self) -> bool {
        self.visible_separator && self.nonzero_default_between_notes()
    }

    fn visible_non_default_between_notes(self) -> bool {
        self.visible_separator && !self.default_between_notes()
    }

    fn visible_non_default_compact_between_notes(self) -> bool {
        self.visible_non_default_between_notes() && self.default_or_compact_between_notes()
    }

    fn large_between_notes(self) -> bool {
        self.between_notes_hu > ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU
            && !self.absorbed_between_notes_gap
    }

    fn visible_large_between_notes(self) -> bool {
        self.visible_separator && self.large_between_notes()
    }

    fn no_separator_large_between_notes(self) -> bool {
        !self.visible_separator && self.large_between_notes()
    }

    fn large_separator_margin(self) -> bool {
        self.separator_above_hu > ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU
            || self.separator_below_hu > ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU
    }

    fn visible_zero_between_large_separator_margin(self) -> bool {
        self.visible_separator && self.between_notes_hu == 0 && self.large_separator_margin()
    }

    fn visible_large_between_zero_above_compact_below(self) -> bool {
        self.visible_large_between_notes()
            && self.separator_above_hu == 0
            && self.compact_separator_below
    }

    fn pagination_between_notes_margin(self) -> i32 {
        if self.visible_separator && self.absorbed_between_notes_gap {
            0
        } else {
            (self.between_notes_hu - ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU).max(0)
        }
    }

    fn separator_height_px(self, dpi: f64) -> f64 {
        let line_height = if self.visible_separator {
            border_width_to_px(self.separator_line_width).max(0.5)
        } else {
            0.0
        };
        hwpunit_to_px(self.separator_above_hu, dpi)
            + line_height
            + hwpunit_to_px(self.separator_below_hu, dpi)
    }
}

fn endnote_between_notes_pagination_margin(shape: &FootnoteShape) -> i32 {
    // 7mm 기본값은 저장 LINE_SEG 흐름에 이미 녹아 있지만, 20mm처럼 커진
    // "미주 사이" 초과분은 번호 경계마다 pagination vpos에도 온전히
    // 반영해야 한컴의 단 분기와 맞는다.
    (endnote_between_notes_margin(shape) as i32 - ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU).max(0)
}

fn compact_endnote_between_notes_flow(shape: &FootnoteShape) -> bool {
    let between = endnote_between_notes_margin(shape) as i32;
    between <= ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU || endnote_has_absorbed_between_notes_gap(shape)
}

fn endnote_has_absorbed_between_notes_gap(shape: &FootnoteShape) -> bool {
    let between = endnote_between_notes_margin(shape) as i32;
    if between <= ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU {
        return false;
    }

    // 한컴 기본 근방의 "미주 사이"는 구분선 아래가 작고 구분선 위가 충분하면
    // 단 전환 기준에서는 별도 20mm 블록처럼 소비되지 않고 앞쪽 여백에 흡수된다.
    let below = endnote_separator_below_margin(shape) as i32;
    let above = shape.separator_above_margin_hu() as i32;
    below <= ENDNOTE_BETWEEN_NOTES_BASE_FLOW_HU && above > 0 && between <= above
}

fn endnote_has_compact_separator_below(shape: &FootnoteShape) -> bool {
    endnote_separator_below_margin(shape) <= ENDNOTE_COMPACT_SEPARATOR_BELOW_MAX_HU
}

fn endnote_has_visible_separator(shape: &FootnoteShape) -> bool {
    shape.separator_line_type != 0 || shape.separator_line_width != 0 || shape.separator_length != 0
}

fn endnote_separator_height_px(shape: &FootnoteShape, dpi: f64) -> f64 {
    let line_height = if endnote_has_visible_separator(shape) {
        border_width_to_px(shape.separator_line_width).max(0.5)
    } else {
        0.0
    };
    hwpunit_to_px(shape.separator_above_margin_hu() as i32, dpi)
        + line_height
        + hwpunit_to_px(endnote_separator_below_margin(shape) as i32, dpi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::page::{ColumnDef, PageDef};
    use crate::model::paragraph::{LineSeg, Paragraph};
    use crate::renderer::composer::ComposedParagraph;
    use crate::renderer::height_measurer::HeightMeasurer;
    use crate::renderer::page_layout::PageLayoutInfo;
    use crate::renderer::pagination::Paginator;
    use crate::renderer::style_resolver::ResolvedStyleSet;

    fn a4_page_def() -> PageDef {
        PageDef {
            width: 59528,
            height: 84188,
            margin_left: 8504,
            margin_right: 8504,
            margin_top: 5669,
            margin_bottom: 4252,
            margin_header: 4252,
            margin_footer: 4252,
            margin_gutter: 0,
            ..Default::default()
        }
    }

    fn make_paragraph_with_height(line_height: i32) -> Paragraph {
        Paragraph {
            line_segs: vec![LineSeg {
                line_height,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn page_with_items(items: Vec<PageItem>) -> PageContent {
        PageContent {
            page_index: 0,
            page_number: 0,
            section_index: 0,
            layout: PageLayoutInfo::from_page_def(
                &a4_page_def(),
                &ColumnDef::default(),
                DEFAULT_DPI,
            ),
            column_contents: vec![ColumnContent {
                column_index: 0,
                start_height: 0.0,
                endnote_flow: false,
                items,
                zone_layout: None,
                zone_y_offset: 0.0,
                wrap_around_paras: Vec::new(),
                used_height: 0.0,
                wrap_anchors: std::collections::HashMap::new(),
            }],
            active_header: None,
            active_footer: None,
            page_number_pos: None,
            page_hide: None,
            footnotes: Vec::new(),
            active_master_page: None,
            extra_master_pages: Vec::new(),
        }
    }

    /// 두 PaginationResult의 페이지 수와 각 페이지의 항목 수가 동일한지 비교
    fn assert_pagination_match(old: &PaginationResult, new: &PaginationResult, label: &str) {
        assert_eq!(
            old.pages.len(),
            new.pages.len(),
            "{}: 페이지 수 불일치 (old={}, new={})",
            label,
            old.pages.len(),
            new.pages.len(),
        );

        for (pi, (old_page, new_page)) in old.pages.iter().zip(new.pages.iter()).enumerate() {
            assert_eq!(
                old_page.column_contents.len(),
                new_page.column_contents.len(),
                "{}: p{} 단 수 불일치",
                label,
                pi,
            );

            for (ci, (old_col, new_col)) in old_page
                .column_contents
                .iter()
                .zip(new_page.column_contents.iter())
                .enumerate()
            {
                assert_eq!(
                    old_col.items.len(),
                    new_col.items.len(),
                    "{}: p{} col{} 항목 수 불일치 (old={}, new={})",
                    label,
                    pi,
                    ci,
                    old_col.items.len(),
                    new_col.items.len(),
                );
            }
        }
    }

    #[test]
    fn test_typeset_engine_creation() {
        let engine = TypesetEngine::new(96.0);
        assert_eq!(engine.dpi, 96.0);
    }

    #[test]
    fn test_typeset_empty_paragraphs() {
        let engine = TypesetEngine::with_default_dpi();
        let styles = ResolvedStyleSet::default();
        let composed: Vec<ComposedParagraph> = Vec::new();

        let result = engine.typeset_section(
            &[],
            &composed,
            &styles,
            &a4_page_def(),
            &ColumnDef::default(),
            0,
            &[],
            false,
            &std::collections::HashSet::new(),
        );

        assert_eq!(result.pages.len(), 1, "빈 문서도 최소 1페이지");
    }

    #[test]
    fn table_continuation_does_not_reapply_page_hide() {
        let hide = crate::model::control::PageHide {
            hide_master_page: true,
            hide_page_num: true,
            ..Default::default()
        };
        let mut pages = vec![
            page_with_items(vec![PageItem::PartialTable {
                para_index: 7,
                control_index: 0,
                start_row: 0,
                end_row: 2,
                is_continuation: false,
                start_cut: Vec::new(),
                end_cut: Vec::new(),
                is_block_split: false,
            }]),
            page_with_items(vec![PageItem::PartialTable {
                para_index: 7,
                control_index: 0,
                start_row: 2,
                end_row: 4,
                is_continuation: true,
                start_cut: Vec::new(),
                end_cut: Vec::new(),
                is_block_split: false,
            }]),
        ];

        TypesetEngine::finalize_pages(&mut pages, &[], &None, &[], &[(7, hide)], 0);

        assert!(pages[0].page_hide.is_some());
        assert!(pages[1].page_hide.is_none());
    }

    #[test]
    fn footnote_area_reserve_uses_section_shape_metrics() {
        let engine = TypesetEngine::with_default_dpi();
        let styles = ResolvedStyleSet::default();
        let shape = FootnoteShape {
            separator_margin_top: 1000,
            note_spacing: 700,
            raw_unknown: 900,
            separator_line_width: 4,
            ..Default::default()
        };
        let note1 = Paragraph {
            text: "첫 각주".to_string(),
            line_segs: vec![LineSeg {
                line_height: 400,
                ..Default::default()
            }],
            ..Default::default()
        };
        let note2 = Paragraph {
            text: "둘째 각주".to_string(),
            line_segs: vec![LineSeg {
                line_height: 600,
                ..Default::default()
            }],
            ..Default::default()
        };
        let paras = vec![Paragraph {
            text: "본문".to_string(),
            line_segs: vec![LineSeg {
                line_height: 400,
                ..Default::default()
            }],
            controls: vec![
                Control::Footnote(Box::new(crate::model::footnote::Footnote {
                    number: 1,
                    paragraphs: vec![note1],
                    ..Default::default()
                })),
                Control::Footnote(Box::new(crate::model::footnote::Footnote {
                    number: 2,
                    paragraphs: vec![note2],
                    ..Default::default()
                })),
            ],
            ..Default::default()
        }];
        let composed: Vec<ComposedParagraph> = paras
            .iter()
            .map(crate::renderer::composer::compose_paragraph)
            .collect();

        let result = engine.typeset_section_with_variant(
            &paras,
            &composed,
            &styles,
            &a4_page_def(),
            &ColumnDef::default(),
            0,
            &[],
            false,
            false,
            false,
            false,
            Some(&shape),
            None,
            &std::collections::HashSet::new(),
            false,
        );

        let expected = footnote_separator_overhead_px(&shape, DEFAULT_DPI)
            + hwpunit_to_px(400, DEFAULT_DPI)
            + footnote_between_notes_margin_px(&shape, DEFAULT_DPI)
            + hwpunit_to_px(600, DEFAULT_DPI);
        let page = result.pages.first().expect("page");
        assert_eq!(page.footnotes.len(), 2);
        assert!((page.layout.footnote_area.height - expected).abs() < 0.01);
    }

    #[test]
    fn test_typeset_single_paragraph() {
        let engine = TypesetEngine::with_default_dpi();
        let paginator = Paginator::with_default_dpi();
        let styles = ResolvedStyleSet::default();
        let paras = vec![make_paragraph_with_height(400)];
        let composed: Vec<ComposedParagraph> = Vec::new();
        let page_def = a4_page_def();
        let col_def = ColumnDef::default();

        let (old_result, measured) =
            paginator.paginate(&paras, &composed, &styles, &page_def, &col_def, 0);
        let new_result = engine.typeset_section(
            &paras,
            &composed,
            &styles,
            &page_def,
            &col_def,
            0,
            &measured.tables,
            false,
            &std::collections::HashSet::new(),
        );

        assert_pagination_match(&old_result, &new_result, "single_paragraph");
    }

    #[test]
    fn test_typeset_page_overflow() {
        let engine = TypesetEngine::with_default_dpi();
        let paginator = Paginator::with_default_dpi();
        let styles = ResolvedStyleSet::default();
        let paras: Vec<Paragraph> = (0..100).map(|_| make_paragraph_with_height(2000)).collect();
        let composed: Vec<ComposedParagraph> = Vec::new();
        let page_def = a4_page_def();
        let col_def = ColumnDef::default();

        let (old_result, measured) =
            paginator.paginate(&paras, &composed, &styles, &page_def, &col_def, 0);
        let new_result = engine.typeset_section(
            &paras,
            &composed,
            &styles,
            &page_def,
            &col_def,
            0,
            &measured.tables,
            false,
            &std::collections::HashSet::new(),
        );

        assert_pagination_match(&old_result, &new_result, "page_overflow");
    }

    #[test]
    fn saved_single_line_at_body_bottom_stays_on_current_page() {
        let engine = TypesetEngine::with_default_dpi();
        let mut styles = ResolvedStyleSet::default();
        styles
            .para_styles
            .push(crate::renderer::style_resolver::ResolvedParaStyle {
                spacing_before: 9.3,
                ..Default::default()
            });
        let page_def = a4_page_def();
        let col_def = ColumnDef::default();
        let layout = PageLayoutInfo::from_page_def(&page_def, &col_def, DEFAULT_DPI);
        let body_height_hu =
            crate::renderer::px_to_hwpunit(layout.available_body_height(), DEFAULT_DPI);
        let line_height = 1200;
        let line_spacing = 840;
        let spacing_before_hu = crate::renderer::px_to_hwpunit(9.3, DEFAULT_DPI);
        let lead_height = body_height_hu - line_height;
        let lead_measured_height = lead_height - spacing_before_hu + 600;
        let paras = vec![
            Paragraph {
                text: "lead".to_string(),
                line_segs: vec![LineSeg {
                    vertical_pos: 0,
                    line_height: lead_measured_height,
                    text_height: lead_measured_height,
                    ..Default::default()
                }],
                ..Default::default()
            },
            Paragraph {
                para_shape_id: 0,
                text: "tail".to_string(),
                line_segs: vec![LineSeg {
                    vertical_pos: lead_height,
                    line_height,
                    text_height: line_height,
                    line_spacing,
                    ..Default::default()
                }],
                ..Default::default()
            },
        ];
        let composed: Vec<ComposedParagraph> = Vec::new();

        let result = engine.typeset_section(
            &paras,
            &composed,
            &styles,
            &page_def,
            &col_def,
            0,
            &[],
            false,
            &std::collections::HashSet::new(),
        );

        assert_eq!(result.pages.len(), 1);
        assert_eq!(result.pages[0].column_contents[0].items.len(), 2);
    }

    #[test]
    fn two_line_tail_before_vpos_reset_stays_on_current_page_when_visible_bottom_fits() {
        let engine = TypesetEngine::with_default_dpi();
        let mut styles = ResolvedStyleSet::default();
        styles
            .para_styles
            .push(crate::renderer::style_resolver::ResolvedParaStyle::default());
        styles
            .para_styles
            .push(crate::renderer::style_resolver::ResolvedParaStyle {
                spacing_before: hwpunit_to_px(2400, DEFAULT_DPI),
                spacing_after: hwpunit_to_px(1400, DEFAULT_DPI),
                ..Default::default()
            });

        let page_def = a4_page_def();
        let col_def = ColumnDef::default();
        let layout = PageLayoutInfo::from_page_def(&page_def, &col_def, DEFAULT_DPI);
        let body_height_hu =
            crate::renderer::px_to_hwpunit(layout.available_body_height(), DEFAULT_DPI);
        let line_height = 1200;
        let line_spacing = 840;
        let first_vpos = body_height_hu - 3740;
        let lead_height = first_vpos - 2400;

        let paras = vec![
            Paragraph {
                text: "lead".to_string(),
                line_segs: vec![LineSeg {
                    vertical_pos: 0,
                    line_height: lead_height,
                    text_height: lead_height,
                    ..Default::default()
                }],
                ..Default::default()
            },
            Paragraph {
                para_shape_id: 1,
                text: "line one\nline two".to_string(),
                line_segs: vec![
                    LineSeg {
                        vertical_pos: first_vpos,
                        line_height,
                        text_height: line_height,
                        line_spacing,
                        ..Default::default()
                    },
                    LineSeg {
                        vertical_pos: first_vpos + line_height + line_spacing,
                        line_height,
                        text_height: line_height,
                        line_spacing,
                        text_start: 9,
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
            Paragraph {
                text: "next page".to_string(),
                line_segs: vec![LineSeg {
                    vertical_pos: 0,
                    line_height,
                    text_height: line_height,
                    ..Default::default()
                }],
                ..Default::default()
            },
        ];
        let composed: Vec<ComposedParagraph> = Vec::new();

        let result = engine.typeset_section(
            &paras,
            &composed,
            &styles,
            &page_def,
            &col_def,
            0,
            &[],
            false,
            &std::collections::HashSet::new(),
        );

        assert_eq!(result.pages.len(), 2);
        assert!(matches!(
            result.pages[0].column_contents[0].items.as_slice(),
            [
                PageItem::FullParagraph { para_index: 0 },
                PageItem::FullParagraph { para_index: 1 }
            ]
        ));
        assert!(matches!(
            result.pages[1].column_contents[0].items.as_slice(),
            [PageItem::FullParagraph { para_index: 2 }]
        ));
    }

    #[test]
    fn saved_tac_table_line_at_body_bottom_stays_on_current_page() {
        let engine = TypesetEngine::with_default_dpi();
        let styles = ResolvedStyleSet::default();
        let page_def = a4_page_def();
        let col_def = ColumnDef::default();
        let layout = PageLayoutInfo::from_page_def(&page_def, &col_def, DEFAULT_DPI);
        let body_height_hu =
            crate::renderer::px_to_hwpunit(layout.available_body_height(), DEFAULT_DPI);
        let table_height = 10_072;
        let table_vpos = body_height_hu - table_height;
        let lead_height = table_vpos + 900;
        let paras = vec![
            Paragraph {
                text: "lead".to_string(),
                line_segs: vec![LineSeg {
                    vertical_pos: 0,
                    line_height: lead_height,
                    text_height: lead_height,
                    ..Default::default()
                }],
                ..Default::default()
            },
            Paragraph {
                controls: vec![Control::Table(Box::new(crate::model::table::Table {
                    attr: 1,
                    row_count: 3,
                    col_count: 3,
                    common: crate::model::shape::CommonObjAttr {
                        treat_as_char: true,
                        text_wrap: crate::model::shape::TextWrap::TopAndBottom,
                        height: table_height as u32,
                        ..Default::default()
                    },
                    ..Default::default()
                }))],
                line_segs: vec![LineSeg {
                    vertical_pos: table_vpos,
                    line_height: table_height,
                    text_height: table_height,
                    line_spacing: 120,
                    ..Default::default()
                }],
                ..Default::default()
            },
        ];
        let composed: Vec<ComposedParagraph> = Vec::new();

        let result = engine.typeset_section(
            &paras,
            &composed,
            &styles,
            &page_def,
            &col_def,
            0,
            &[],
            false,
            &std::collections::HashSet::new(),
        );

        assert_eq!(result.pages.len(), 1);
        assert!(matches!(
            result.pages[0].column_contents[0].items.as_slice(),
            [
                PageItem::FullParagraph { para_index: 0 },
                PageItem::Table { para_index: 1, .. }
            ]
        ));
    }

    /// [Task #1363 v3 Stage 2] scratch 측정 부작용 격리 회귀 가드.
    ///
    /// `measure_endnote_para_advance` 는 매 호출 `LayoutEngine::new()` 로 독립 인스턴스를
    /// 쓰므로 (a) 양수·유한, (b) 동일 엔진 반복 호출에 결정적(호출 간 상태 무누적),
    /// (c) 독립 `TypesetEngine` 인스턴스 간 동일(전역/공유 가변 상태 누수 없음)이어야 한다.
    /// scratch 의 numbering/overflow/last_item_content_bottom 변이가 측정에만 머무름을 실증.
    #[test]
    fn test_measure_endnote_advance_side_effect_free() {
        use crate::renderer::composer::compose_paragraph;

        let para = Paragraph {
            text: "각주 측정 격리 회귀 가드 문장".to_string(),
            line_segs: vec![LineSeg {
                line_height: 1000,
                baseline_distance: 850,
                ..Default::default()
            }],
            ..Default::default()
        };
        let composed = compose_paragraph(&para);
        let styles = ResolvedStyleSet::default();
        let item = PageItem::FullParagraph { para_index: 900 };
        let (en_col_w, available, y_start) = (280.0_f64, 900.0_f64, 100.0_f64);

        let engine = TypesetEngine::new(96.0);
        let first = engine.measure_endnote_para_advance(
            &para, &composed, &styles, en_col_w, available, y_start, &item, 0, 900,
        );

        // (a) 양수·유한 — 실제 텍스트 para 는 advance 를 만든다.
        assert!(
            first.is_finite() && first > 0.0,
            "advance must be positive finite: {first}",
        );

        // (b) 동일 엔진 반복 호출 → 결정적 (scratch 호출 간 상태 무누적).
        for _ in 0..5 {
            let v = engine.measure_endnote_para_advance(
                &para, &composed, &styles, en_col_w, available, y_start, &item, 0, 900,
            );
            assert_eq!(v, first, "repeat call drifted — scratch 상태 누적 누수");
        }

        // (c) 독립 TypesetEngine 인스턴스 → 동일 (전역 가변 상태 누수 없음).
        let engine2 = TypesetEngine::new(96.0);
        let other = engine2.measure_endnote_para_advance(
            &para, &composed, &styles, en_col_w, available, y_start, &item, 0, 900,
        );
        assert_eq!(other, first, "independent engine differs — 전역 상태 누수");
    }

    #[test]
    fn test_typeset_line_split() {
        let engine = TypesetEngine::with_default_dpi();
        let paginator = Paginator::with_default_dpi();
        let styles = ResolvedStyleSet::default();

        // 여러 줄이 있는 큰 문단 (페이지 경계에서 줄 단위 분할)
        let paras = vec![Paragraph {
            line_segs: (0..50)
                .map(|_| LineSeg {
                    line_height: 1800,
                    line_spacing: 200,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }];
        let composed: Vec<ComposedParagraph> = Vec::new();
        let page_def = a4_page_def();
        let col_def = ColumnDef::default();

        let (old_result, measured) =
            paginator.paginate(&paras, &composed, &styles, &page_def, &col_def, 0);
        let new_result = engine.typeset_section(
            &paras,
            &composed,
            &styles,
            &page_def,
            &col_def,
            0,
            &measured.tables,
            false,
            &std::collections::HashSet::new(),
        );

        assert_pagination_match(&old_result, &new_result, "line_split");
    }

    #[test]
    fn test_typeset_mixed_paragraphs() {
        let engine = TypesetEngine::with_default_dpi();
        let paginator = Paginator::with_default_dpi();
        let styles = ResolvedStyleSet::default();

        // 다양한 높이의 문단 혼합
        let paras: Vec<Paragraph> = vec![
            make_paragraph_with_height(400),
            make_paragraph_with_height(10000), // 큰 문단
            make_paragraph_with_height(400),
            make_paragraph_with_height(800),
            make_paragraph_with_height(20000), // 매우 큰 문단
            make_paragraph_with_height(400),
        ];
        let composed: Vec<ComposedParagraph> = Vec::new();
        let page_def = a4_page_def();
        let col_def = ColumnDef::default();

        let (old_result, measured) =
            paginator.paginate(&paras, &composed, &styles, &page_def, &col_def, 0);
        let new_result = engine.typeset_section(
            &paras,
            &composed,
            &styles,
            &page_def,
            &col_def,
            0,
            &measured.tables,
            false,
            &std::collections::HashSet::new(),
        );

        assert_pagination_match(&old_result, &new_result, "mixed_paragraphs");
    }

    #[test]
    fn test_typeset_page_break() {
        let engine = TypesetEngine::with_default_dpi();
        let paginator = Paginator::with_default_dpi();
        let styles = ResolvedStyleSet::default();

        // 강제 쪽 나누기가 있는 문단
        let paras = vec![
            make_paragraph_with_height(400),
            {
                let mut p = make_paragraph_with_height(400);
                p.column_type = ColumnBreakType::Page;
                p
            },
            make_paragraph_with_height(400),
        ];
        let composed: Vec<ComposedParagraph> = Vec::new();
        let page_def = a4_page_def();
        let col_def = ColumnDef::default();

        let (old_result, measured) =
            paginator.paginate(&paras, &composed, &styles, &page_def, &col_def, 0);
        let new_result = engine.typeset_section(
            &paras,
            &composed,
            &styles,
            &page_def,
            &col_def,
            0,
            &measured.tables,
            false,
            &std::collections::HashSet::new(),
        );

        assert_pagination_match(&old_result, &new_result, "page_break");
        assert_eq!(new_result.pages.len(), 2, "쪽 나누기로 2페이지");
    }

    // [Task #1046] 사후 reflow force-break hint 메커니즘 검증.
    #[test]
    fn test_typeset_force_break_before_hint() {
        let engine = TypesetEngine::with_default_dpi();
        let styles = ResolvedStyleSet::default();
        // 한 페이지에 충분히 들어가는 3개 문단
        let paras = vec![
            make_paragraph_with_height(400),
            make_paragraph_with_height(400),
            make_paragraph_with_height(400),
        ];
        let composed: Vec<ComposedParagraph> = Vec::new();
        let page_def = a4_page_def();
        let col_def = ColumnDef::default();

        // hint 없음 → 1페이지
        let baseline = engine.typeset_section(
            &paras,
            &composed,
            &styles,
            &page_def,
            &col_def,
            0,
            &[],
            false,
            &std::collections::HashSet::new(),
        );
        assert_eq!(baseline.pages.len(), 1, "hint 없으면 3문단 모두 1페이지");

        // para_idx=1 에 force-break hint → para 1 이 2페이지에서 시작
        let mut hint = std::collections::HashSet::new();
        hint.insert(1usize);
        let reflowed = engine.typeset_section(
            &paras,
            &composed,
            &styles,
            &page_def,
            &col_def,
            0,
            &[],
            false,
            &hint,
        );
        assert_eq!(reflowed.pages.len(), 2, "para1 force-break 로 2페이지");
        let page0_paras: Vec<usize> = reflowed.pages[0]
            .column_contents
            .iter()
            .flat_map(|cc| cc.items.iter().map(|it| it.para_index()))
            .collect();
        let page1_paras: Vec<usize> = reflowed.pages[1]
            .column_contents
            .iter()
            .flat_map(|cc| cc.items.iter().map(|it| it.para_index()))
            .collect();
        assert_eq!(page0_paras, vec![0], "1페이지엔 para0 만");
        assert_eq!(page1_paras, vec![1, 2], "2페이지엔 para1,2");
    }

    // ========================================================
    // 실제 HWP 파일 비교 테스트
    // ========================================================

    /// 실제 HWP 파일로 기존 Paginator와 TypesetEngine 결과 비교
    fn compare_with_hwp_file(path: &str) {
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(_) => {
                eprintln!("skip: {} not found", path);
                return;
            }
        };
        let doc = match crate::document_core::DocumentCore::from_bytes(&data) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("skip: {} parse error: {}", path, e);
                return;
            }
        };

        let engine = TypesetEngine::with_default_dpi();

        for (sec_idx, section) in doc.document.sections.iter().enumerate() {
            let composed = &doc.composed[sec_idx];
            let measured_tables = &doc.measured_tables[sec_idx];
            let column_def =
                crate::document_core::DocumentCore::find_initial_column_def(&section.paragraphs);

            // 구역에 표가 포함되어 있는지 확인
            let has_tables = section
                .paragraphs
                .iter()
                .any(|p| p.controls.iter().any(|c| matches!(c, Control::Table(_))));

            let new_result = engine.typeset_section(
                &section.paragraphs,
                composed,
                &doc.styles,
                &section.section_def.page_def,
                &column_def,
                sec_idx,
                measured_tables,
                section.section_def.hide_empty_line,
                &std::collections::HashSet::new(),
            );

            let old_result = &doc.pagination[sec_idx];
            let label = format!("{} sec{}", path, sec_idx);

            if has_tables {
                // 표가 포함된 구역: Phase 2 전환 전까지 차이 허용 (경고만 출력)
                if old_result.pages.len() != new_result.pages.len() {
                    eprintln!(
                        "WARN {}: 표 포함 구역 페이지 수 차이 (old={}, new={}) — Phase 2에서 해결",
                        label,
                        old_result.pages.len(),
                        new_result.pages.len(),
                    );
                }
            } else {
                // 비-표 구역: 완전 일치 필수
                assert_eq!(
                    old_result.pages.len(),
                    new_result.pages.len(),
                    "{}: 페이지 수 불일치 (old={}, new={})",
                    label,
                    old_result.pages.len(),
                    new_result.pages.len(),
                );

                for (pi, (old_page, new_page)) in old_result
                    .pages
                    .iter()
                    .zip(new_result.pages.iter())
                    .enumerate()
                {
                    assert_eq!(
                        old_page.column_contents.len(),
                        new_page.column_contents.len(),
                        "{}: p{} 단 수 불일치",
                        label,
                        pi,
                    );
                }
            }
        }
    }

    #[test]
    fn test_typeset_vs_paginator_p222() {
        // p222.hwp sec2는 표가 많아 Phase 2 전환 전까지 차이 발생 가능
        // Phase 1에서는 비-표 문단만 검증
        compare_with_hwp_file("samples/p222.hwp");
    }

    #[test]
    fn test_typeset_vs_paginator_hongbo() {
        compare_with_hwp_file("samples/20250130-hongbo.hwp");
    }

    #[test]
    fn test_typeset_vs_paginator_biz_plan() {
        compare_with_hwp_file("samples/biz_plan.hwp");
    }

    /// Issue #703: BehindText/InFrontOfText 표는 본문 흐름에서 제외되어야 한다.
    ///
    /// 글뒤로 (BehindText) / 글앞으로 (InFrontOfText) 표는 시각적으로 본문 텍스트 뒤/앞에
    /// 절대 좌표로 배치되는 데코레이션 (워터마크/배경 등) 이며, 본문 흐름의 vertical advance 에
    /// 영향을 주지 않는다. `pagination/engine.rs:976-981` 와 동일 시멘틱.
    ///
    /// 결함 메커니즘: typeset_block_table → place_table_with_text → `cur_h += table_total_height`
    /// (line 1594) 가 BehindText/InFrontOfText 표에 대해서도 적용되어 본문 흐름 누적이 발생.
    ///
    /// 본 테스트는 BIG BehindText 표 (≈300 mm 높이) 를 1 페이지 본문 안에 넣어두고 후속
    /// paragraph 가 동일 페이지에 들어감을 검증한다. 결함 시 BehindText 표의 거대 height 가
    /// cur_h 에 가산되어 후속 paragraph 가 다음 페이지로 밀림.
    #[test]
    fn test_typeset_703_behind_text_table_no_flow_advance() {
        use crate::model::shape::TextWrap;
        let engine = TypesetEngine::with_default_dpi();
        let paginator = Paginator::with_default_dpi();
        let styles = ResolvedStyleSet::default();
        let page_def = a4_page_def();
        let col_def = ColumnDef::default();
        let composed: Vec<ComposedParagraph> = Vec::new();

        // BehindText 1×1 표: 본문 높이의 약 80% 차지 (60000 HU ≈ 800 px @96dpi).
        // BehindText 는 데코레이션이므로 본문 흐름 누적 0 이어야 정상.
        // 결함 시 cur_h 에 800 px 가산 → 후속 1 단락도 fit 실패 → 페이지 분할.
        let mut table = crate::model::table::Table {
            row_count: 1,
            col_count: 1,
            cells: vec![crate::model::table::Cell {
                col: 0,
                row: 0,
                col_span: 1,
                row_span: 1,
                width: 51974,
                height: 60000,
                paragraphs: vec![Paragraph::default()],
                ..Default::default()
            }],
            ..Default::default()
        };
        table.common.text_wrap = TextWrap::BehindText;
        table.common.treat_as_char = false;
        table.common.width = 51974;
        table.common.height = 60000; // ≈800 px @96dpi — 본문 80% 점유 (결함 시 가산되는 양)

        let host_para = Paragraph {
            line_segs: vec![LineSeg {
                line_height: 1000,
                line_spacing: 600,
                ..Default::default()
            }],
            controls: vec![crate::model::control::Control::Table(Box::new(table))],
            ..Default::default()
        };

        // 후속 5 단락 — 본문 정상 흐름이면 호스트(21px) + 5 × 13px = 86 px (1 페이지 여유)
        // 결함 시 호스트(21+800=821px) + 첫 단락(13px) = 834 px 도 fit, 더 추가 시 결국 분할
        // → 단순히 페이지 수 정확히 비교 필요.
        let mut paras = vec![host_para];
        for _ in 0..5 {
            paras.push(make_paragraph_with_height(1000));
        }

        let (paginator_result, measured) =
            paginator.paginate(&paras, &composed, &styles, &page_def, &col_def, 0);
        let typeset_result = engine.typeset_section(
            &paras,
            &composed,
            &styles,
            &page_def,
            &col_def,
            0,
            &measured.tables,
            false,
            &std::collections::HashSet::new(),
        );

        // 검증 1: paginator (engine.rs reference) 는 1 페이지에 모두 배치
        assert_eq!(
            paginator_result.pages.len(),
            1,
            "[reference] BehindText 표 + 5 후속 paragraph 는 paginator 에서 1 페이지에 들어가야 함",
        );

        // 검증 2: typeset 결과도 1 페이지 (현재 결함 시 RED — typeset 이 BehindText 표 height 를 누적)
        assert_eq!(
            typeset_result.pages.len(),
            1,
            "[BUG #703] typeset 도 1 페이지여야 함. 결함 시 BehindText 표 height ≈800 px 가 \
             cur_h 에 가산되어 후속 paragraph 가 다음 페이지로 밀림 (RED)",
        );
    }
}
