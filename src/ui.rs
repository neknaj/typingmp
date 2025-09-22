// uefi featureが有効な場合、標準のallocクレートをインポート
#[cfg(feature = "uefi")]
extern crate alloc;

// uefi と std で使用する Vec と vec! を切り替える
#[cfg(feature = "uefi")]
use alloc::vec::Vec;
#[cfg(feature = "uefi")]
use alloc::vec;
#[cfg(not(feature = "uefi"))]
use std::vec::Vec;

use crate::app::App;

/// 画面上の描画基準点を定義するenum
#[derive(Clone, Copy)]
pub enum Anchor {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

/// Anchorからのオフセット（移動量）を定義する構造体
#[derive(Clone, Copy)]
pub struct Shift {
    pub x: f32,
    pub y: f32,
}

/// 水平方向の揃え
#[derive(Clone, Copy)]
pub enum HorizontalAlign {
    Left,
    Center,
    Right,
}

/// 垂直方向の揃え
#[derive(Clone, Copy)]
pub enum VerticalAlign {
    Top,
    Center,
    Bottom,
}

/// テキストの揃え方を定義する構造体
#[derive(Clone, Copy)]
pub struct Align {
    pub horizontal: HorizontalAlign,
    pub vertical: VerticalAlign,
}

/// 画面に描画すべき要素の種類とレイアウト情報を定義するenum
pub enum Renderable<'a> {
    /// 通常のフォントサイズで描画されるテキスト
    Text {
        text: &'a str,
        anchor: Anchor,
        shift: Shift,
        align: Align,
    },
    /// 大きなフォントサイズで描画されるテキスト
    BigText {
        text: &'a str,
        anchor: Anchor,
        shift: Shift,
        align: Align,
    },
}

/// Appの状態を受け取り、描画リスト（UIレイアウト）を構築する
pub fn build_ui<'a>(app: &'a App) -> Vec<Renderable<'a>> {
    vec![
        // 画面中央に表示する大きなテキスト
        Renderable::BigText {
            text: &app.input_text,
            anchor: Anchor::CenterLeft,
            shift: Shift { x: 0.02, y: 0.0 }, // 画面幅の2%右にずらす
            align: Align {
                horizontal: HorizontalAlign::Left,
                vertical: VerticalAlign::Center,
            },
        },
        // 画面左下に表示するステータステキスト
        Renderable::Text {
            text: &app.status_text,
            anchor: Anchor::BottomLeft,
            shift: Shift { x: 0.01, y: -0.02 }, // 画面幅の1%右、画面高さの2%上にずらす
            align: Align {
                horizontal: HorizontalAlign::Left,
                vertical: VerticalAlign::Bottom,
            },
        },
    ]
}

/// AnchorとShiftから、基準となる座標(x, y)を計算する
pub fn calculate_anchor_position(anchor: Anchor, shift: Shift, width: usize, height: usize) -> (i32, i32) {
    let (w, h) = (width as i32, height as i32);
    let base_pos = match anchor {
        Anchor::TopLeft => (0, 0),
        Anchor::TopCenter => (w / 2, 0),
        Anchor::TopRight => (w, 0),
        Anchor::CenterLeft => (0, h / 2),
        Anchor::Center => (w / 2, h / 2),
        Anchor::CenterRight => (w, h / 2),
        Anchor::BottomLeft => (0, h),
        Anchor::BottomCenter => (w / 2, h),
        Anchor::BottomRight => (w, h),
    };
    let shift_x = (width as f32 * shift.x) as i32;
    let shift_y = (height as f32 * shift.y) as i32;
    (base_pos.0 + shift_x, base_pos.1 + shift_y)
}

/// 基準点、テキストの寸法、揃え方から、最終的な描画開始座標（左上）を計算する
pub fn calculate_aligned_position(
    anchor_pos: (i32, i32),
    text_width: u32,
    text_height: u32,
    align: Align,
) -> (i32, i32) {
    let (tw, th) = (text_width as i32, text_height as i32);
    let (ax, ay) = anchor_pos;

    let x = match align.horizontal {
        HorizontalAlign::Left => ax,
        HorizontalAlign::Center => ax - tw / 2,
        HorizontalAlign::Right => ax - tw,
    };

    let y = match align.vertical {
        VerticalAlign::Top => ay,
        VerticalAlign::Center => ay - th / 2,
        VerticalAlign::Bottom => ay - th,
    };

    (x, y)
}