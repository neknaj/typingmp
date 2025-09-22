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

/// Anchorからのオフセット（余白）を定義する構造体
#[derive(Clone, Copy)]
pub struct Margin {
    pub x: i32,
    pub y: i32,
}

/// 画面に描画すべき要素の種類とレイアウト情報を定義するenum
pub enum Renderable<'a> {
    /// 通常のフォントサイズで描画されるテキスト
    Text {
        text: &'a str,
        anchor: Anchor,
        margin: Margin,
    },
    /// 大きなフォントサイズで描画されるテキスト
    BigText {
        text: &'a str,
        anchor: Anchor,
        margin: Margin,
    },
}

/// Appの状態を受け取り、描画リスト（UIレイアウト）を構築する
pub fn build_ui<'a>(app: &'a App) -> Vec<Renderable<'a>> {
    vec![
        // 画面中央に表示する大きなテキスト
        Renderable::BigText {
            text: &app.input_text,
            anchor: Anchor::CenterLeft,
            margin: Margin { x: 0, y: 0 },
        },
        // 画面左下に表示するステータステキスト
        Renderable::Text {
            text: &app.status_text,
            anchor: Anchor::BottomLeft,
            margin: Margin { x: 5, y: -5 },
        },
    ]
}

/// AnchorとMarginから、具体的な描画開始座標(x, y)を計算する
pub fn calculate_position(anchor: Anchor, margin: Margin, width: usize, height: usize) -> (i32, i32) {
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
    (base_pos.0 + margin.x, base_pos.1 + margin.y)
}