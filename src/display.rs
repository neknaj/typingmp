#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayAspectRatio {
    Native,
    Ratio16x9,
    Ratio4x3,
    Square1x1,
    Ratio3x4,
    Ratio9x16,
}

impl DisplayAspectRatio {
    pub fn label(self) -> &'static str {
        match self {
            Self::Native => "Native",
            Self::Ratio16x9 => "16:9",
            Self::Ratio4x3 => "4:3",
            Self::Square1x1 => "1:1",
            Self::Ratio3x4 => "3:4",
            Self::Ratio9x16 => "9:16",
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Native => Self::Ratio9x16,
            Self::Ratio16x9 => Self::Native,
            Self::Ratio4x3 => Self::Ratio16x9,
            Self::Square1x1 => Self::Ratio4x3,
            Self::Ratio3x4 => Self::Square1x1,
            Self::Ratio9x16 => Self::Ratio3x4,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Native => Self::Ratio16x9,
            Self::Ratio16x9 => Self::Ratio4x3,
            Self::Ratio4x3 => Self::Square1x1,
            Self::Square1x1 => Self::Ratio3x4,
            Self::Ratio3x4 => Self::Ratio9x16,
            Self::Ratio9x16 => Self::Native,
        }
    }

    fn dimensions(self) -> Option<(u32, u32)> {
        match self {
            Self::Native => None,
            Self::Ratio16x9 => Some((16, 9)),
            Self::Ratio4x3 => Some((4, 3)),
            Self::Square1x1 => Some((1, 1)),
            Self::Ratio3x4 => Some((3, 4)),
            Self::Ratio9x16 => Some((9, 16)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayScale {
    Percent75,
    Percent100,
    Percent125,
    Percent150,
    Percent200,
}

impl DisplayScale {
    pub fn label(self) -> &'static str {
        match self {
            Self::Percent75 => "75%",
            Self::Percent100 => "100%",
            Self::Percent125 => "125%",
            Self::Percent150 => "150%",
            Self::Percent200 => "200%",
        }
    }

    pub fn multiplier(self) -> f32 {
        match self {
            Self::Percent75 => 0.75,
            Self::Percent100 => 1.0,
            Self::Percent125 => 1.25,
            Self::Percent150 => 1.5,
            Self::Percent200 => 2.0,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Percent75 => Self::Percent200,
            Self::Percent100 => Self::Percent75,
            Self::Percent125 => Self::Percent100,
            Self::Percent150 => Self::Percent125,
            Self::Percent200 => Self::Percent150,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Percent75 => Self::Percent100,
            Self::Percent100 => Self::Percent125,
            Self::Percent125 => Self::Percent150,
            Self::Percent150 => Self::Percent200,
            Self::Percent200 => Self::Percent75,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplaySettings {
    pub aspect_ratio: DisplayAspectRatio,
    pub scale: DisplayScale,
}

impl DisplaySettings {
    pub fn viewport(self, frame_width: usize, frame_height: usize) -> DisplayViewport {
        let Some((ratio_w, ratio_h)) = self.aspect_ratio.dimensions() else {
            return DisplayViewport {
                x: 0,
                y: 0,
                width: frame_width,
                height: frame_height,
                scale: self.scale.multiplier(),
            };
        };

        if frame_width == 0 || frame_height == 0 {
            return DisplayViewport {
                x: 0,
                y: 0,
                width: frame_width,
                height: frame_height,
                scale: self.scale.multiplier(),
            };
        }

        let frame_width_u64 = frame_width as u64;
        let ratio_w_u64 = u64::from(ratio_w);
        let ratio_h_u64 = u64::from(ratio_h);

        let aspect_height = frame_width_u64
            .saturating_mul(ratio_h_u64)
            .div_ceil(ratio_w_u64) as usize;
        let width = frame_width;
        let height = frame_height.max(aspect_height.max(1));
        let y = ((frame_height as i64 - height as i64) / 2) as i32;

        DisplayViewport {
            x: 0,
            y,
            width,
            height,
            scale: self.scale.multiplier(),
        }
    }
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            aspect_ratio: DisplayAspectRatio::Native,
            scale: DisplayScale::Percent100,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayViewport {
    pub x: i32,
    pub y: i32,
    pub width: usize,
    pub height: usize,
    pub scale: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_preserves_native_frame() {
        let settings = DisplaySettings::default();
        assert_eq!(
            settings.viewport(1920, 1080),
            DisplayViewport {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
                scale: 1.0
            }
        );
    }

    #[test]
    fn viewport_extends_height_for_portrait_ratio_without_horizontal_trim() {
        let settings = DisplaySettings {
            aspect_ratio: DisplayAspectRatio::Ratio9x16,
            scale: DisplayScale::Percent125,
        };

        assert_eq!(
            settings.viewport(1920, 1080),
            DisplayViewport {
                x: 0,
                y: -1167,
                width: 1920,
                height: 3414,
                scale: 1.25
            }
        );
    }

    #[test]
    fn viewport_keeps_full_frame_when_ratio_would_shrink_height() {
        let settings = DisplaySettings {
            aspect_ratio: DisplayAspectRatio::Ratio16x9,
            scale: DisplayScale::Percent75,
        };

        assert_eq!(
            settings.viewport(1080, 1920),
            DisplayViewport {
                x: 0,
                y: 0,
                width: 1080,
                height: 1920,
                scale: 0.75
            }
        );
    }

    #[test]
    fn settings_options_wrap_around() {
        assert_eq!(
            DisplayAspectRatio::Ratio9x16.next(),
            DisplayAspectRatio::Native
        );
        assert_eq!(
            DisplayAspectRatio::Native.previous(),
            DisplayAspectRatio::Ratio9x16
        );
        assert_eq!(DisplayScale::Percent200.next(), DisplayScale::Percent75);
        assert_eq!(DisplayScale::Percent75.previous(), DisplayScale::Percent200);
    }
}
