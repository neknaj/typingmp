use ab_glyph::FontVec;

use crate::backend::BackendError;
use crate::font::{FontBundle, Fonts};
use crate::io::{embedded_alegreya_font_bytes, AssetProvider, BundledFont};

pub fn load_desktop_fonts(asset_provider: &impl AssetProvider) -> Result<Fonts, BackendError> {
    let ui_font = load_embedded_alegreya_font()?;
    let japanese_font =
        load_bundled_or_alegreya(asset_provider, BundledFont::YujiSyukuRegular, "Yuji Syuku")?;
    let japanese_ruby_font =
        load_bundled_or_alegreya(asset_provider, BundledFont::YujiSyukuRegular, "Yuji Syuku")?;
    let japanese_unconfirmed_font =
        load_bundled_or_alegreya(asset_provider, BundledFont::YujiSyukuRegular, "Yuji Syuku")?;
    let simplified_chinese_font =
        load_bundled_or_alegreya(asset_provider, BundledFont::LongCangRegular, "Long Cang")?;
    let simplified_chinese_ruby_font = load_embedded_alegreya_font()?;
    let simplified_chinese_unconfirmed_font = load_embedded_alegreya_font()?;
    let traditional_chinese_font =
        load_bundled_or_alegreya(asset_provider, BundledFont::LongCangRegular, "Long Cang")?;
    let traditional_chinese_ruby_font = load_embedded_alegreya_font()?;
    let traditional_chinese_unconfirmed_font = load_embedded_alegreya_font()?;
    let english_font =
        load_bundled_or_alegreya(asset_provider, BundledFont::KalamRegular, "Kalam")?;

    Ok(Fonts::new(FontBundle {
        ui: ui_font,
        japanese: japanese_font,
        japanese_ruby: japanese_ruby_font,
        japanese_unconfirmed: japanese_unconfirmed_font,
        chinese_simplified: simplified_chinese_font,
        chinese_simplified_ruby: simplified_chinese_ruby_font,
        chinese_simplified_unconfirmed: simplified_chinese_unconfirmed_font,
        traditional_chinese: traditional_chinese_font,
        traditional_chinese_ruby: traditional_chinese_ruby_font,
        traditional_chinese_unconfirmed: traditional_chinese_unconfirmed_font,
        english: english_font,
    }))
}

pub fn load_embedded_alegreya_font() -> Result<FontVec, BackendError> {
    FontVec::try_from_vec(embedded_alegreya_font_bytes().to_vec())
        .map_err(|_| BackendError::asset("failed to parse embedded Alegreya font"))
}

fn load_bundled_or_alegreya(
    asset_provider: &impl AssetProvider,
    font: BundledFont,
    label: &str,
) -> Result<FontVec, BackendError> {
    if font == BundledFont::AlegreyaRegular {
        return load_embedded_alegreya_font();
    }

    match asset_provider.load_bundled_font(font) {
        Ok(bytes) => match FontVec::try_from_vec(bytes) {
            Ok(font) => Ok(font),
            Err(_) => load_embedded_alegreya_font(),
        },
        Err(_) => load_embedded_alegreya_font().map_err(|error| {
            BackendError::asset(format!(
                "failed to load {label} font and embedded Alegreya fallback: {error}"
            ))
        }),
    }
}
