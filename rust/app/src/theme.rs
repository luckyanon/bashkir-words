//! Drawing values from the 2.10.5 stylesheet. The native screen layout supplies
//! the same CSS-pixel boxes and the raster backend composites these operations.

use std::cell::Cell;

use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Color(u8, u8, u8, u8);

impl Color {
    pub(crate) const fn hex(rgb: u32) -> Self {
        Self::rgba((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255)
    }

    pub(crate) const fn css(rgba: u32) -> Self {
        Self::rgba(
            (rgba >> 24) as u8,
            (rgba >> 16) as u8,
            (rgba >> 8) as u8,
            rgba as u8,
        )
    }

    pub(crate) const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self(red, green, blue, alpha)
    }

    pub(crate) const fn opacity(self, opacity: f32) -> Self {
        Self(
            self.0,
            self.1,
            self.2,
            (self.3 as f32 * opacity + 0.5) as u8,
        )
    }

    pub(crate) fn argb(self) -> String {
        format!("#{:02x}{:02x}{:02x}{:02x}", self.3, self.0, self.1, self.2)
    }
}

pub(crate) const TRANSPARENT: Color = Color::hex(0).opacity(0.0);
pub(crate) const WHITE: Color = Color::hex(0xffffff);
pub(crate) const INK: Color = Color::hex(0x533763);
pub(crate) const DEEP: Color = Color::hex(0x432455);
pub(crate) const VIOLET: Color = Color::hex(0x78469e);
pub(crate) const MUTED: Color = Color::hex(0xe7d4e8);
pub(crate) const EYEBROW: Color = Color::hex(0xf7d6e9);
pub(crate) const LINE: Color = WHITE.opacity(0.24);
pub(crate) const PAGE_BACKDROP: Color = Color::hex(0x453265);
pub(crate) const CROSSWORD_LETTER: Color = Color::hex(0x624078);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Shadow {
    pub(crate) dx: f32,
    pub(crate) dy: f32,
    pub(crate) blur: f32,
    pub(crate) spread: f32,
    pub(crate) color: Color,
    pub(crate) inset: bool,
}

impl Shadow {
    pub(crate) const fn new(dx: f32, dy: f32, blur: f32, spread: f32, color: Color) -> Self {
        Self {
            dx,
            dy,
            blur,
            spread,
            color,
            inset: false,
        }
    }

    pub(crate) const fn inset(mut self) -> Self {
        self.inset = true;
        self
    }

    pub(crate) fn json(self) -> Value {
        json!({
            "dx":self.dx,"dy":self.dy,"blur":self.blur,"spread":self.spread,
            "color":self.color.argb(),"inset":self.inset,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Radius {
    Pixels(f32),
    Fraction(f32),
}

impl Radius {
    fn resolve(self, w: f32, h: f32) -> f32 {
        let side = w.min(h);
        let radius = match self {
            Self::Pixels(px) => px,
            Self::Fraction(fraction) => side * fraction,
        };
        radius.clamp(0.0, side * 0.5)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct BoxStyle<'shadow> {
    pub(crate) color: Color,
    pub(crate) radius: Radius,
    pub(crate) border: Option<(f32, Color)>,
    pub(crate) shadows: &'shadow [Shadow],
    pub(crate) blur: f32,
    pub(crate) backdrop_blur: f32,
}

impl BoxStyle<'_> {
    pub(crate) const fn new(color: Color, radius: f32) -> Self {
        Self {
            color,
            radius: Radius::Pixels(radius),
            border: None,
            shadows: &[],
            blur: 0.0,
            backdrop_blur: 0.0,
        }
    }

    pub(crate) const fn relative_radius(mut self, fraction: f32) -> Self {
        self.radius = Radius::Fraction(fraction);
        self
    }

    pub(crate) const fn border(mut self, width: f32, color: Color) -> Self {
        self.border = Some((width, color));
        self
    }

    pub(crate) const fn shadows(self, shadows: &[Shadow]) -> BoxStyle<'_> {
        BoxStyle { shadows, ..self }
    }

    pub(crate) const fn blur(mut self, sigma: f32) -> Self {
        self.blur = sigma;
        self
    }

    pub(crate) const fn backdrop(mut self, sigma: f32) -> Self {
        self.backdrop_blur = sigma;
        self
    }

    pub(crate) const fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub(crate) fn emit(self, x: f32, y: f32, w: f32, h: f32) -> Value {
        let w = w.max(0.0);
        let h = h.max(0.0);
        let mut command = json!({
            "kind":"rect","x":x,"y":y,"w":w,"h":h,
            "radius":self.radius.resolve(w,h),"color":self.color.argb(),
        });
        if let Some((width, color)) = self.border {
            command["border"] = json!({"width":width,"color":color.argb()});
        }
        if !self.shadows.is_empty() {
            command["shadows"] = json!(self.shadows.iter().map(|s| s.json()).collect::<Vec<_>>());
        }
        if self.blur != 0.0 {
            command["blur"] = json!(self.blur);
        }
        if self.backdrop_blur != 0.0 {
            command["backdropBlur"] = json!(self.backdrop_blur);
        }
        command
    }
}

// Rules merged in original stylesheet cascade order.
pub(crate) const PRIMARY: BoxStyle<'static> =
    BoxStyle::new(Color::hex(0xfff6ef), 10.0).shadows(&[Shadow::new(
        0.0,
        5.0,
        0.0,
        0.0,
        Color::hex(0x381f4f).opacity(0.13),
    )]);
pub(crate) const PRIMARY_PURPLE: BoxStyle<'static> = PRIMARY.with_color(VIOLET);
pub(crate) const SECONDARY: BoxStyle<'static> =
    BoxStyle::new(WHITE.opacity(0.15), 10.0).border(1.0, LINE);
pub(crate) const MENU_SECONDARY: BoxStyle<'static> = SECONDARY.with_color(WHITE.opacity(0.2775));
pub(crate) const COIN_BALANCE: BoxStyle<'static> =
    BoxStyle::new(Color::hex(0x291b40).opacity(0.22), 10.0).border(1.0, LINE);
pub(crate) const MENU_RATING: BoxStyle<'static> = COIN_BALANCE;
pub(crate) const ICON_BUTTON: BoxStyle<'static> = BoxStyle::new(WHITE.opacity(0.14), 8.0)
    .border(1.0, LINE)
    .backdrop(12.0);
pub(crate) const FOUND_BUTTON: BoxStyle<'static> =
    BoxStyle::new(Color::hex(0x311c44).opacity(0.27), 10.0).border(1.0, LINE);
pub(crate) const DIALOG: BoxStyle<'static> = BoxStyle::new(Color::hex(0xfff9f6), 20.0)
    .shadows(&[Shadow::new(0.0, 20.0, 70.0, 0.0, Color::css(0x16102655))]);
pub(crate) const DIALOG_BACKDROP: BoxStyle<'static> =
    BoxStyle::new(Color::css(0x24153380), 0.0).backdrop(6.0);
pub(crate) const CLOSE_MODAL: BoxStyle<'static> = BoxStyle::new(Color::hex(0xece0ef), 8.0);
pub(crate) const MODAL_SECONDARY: BoxStyle<'static> = BoxStyle::new(Color::hex(0xeaddeb), 10.0);
pub(crate) const DANGER: BoxStyle<'static> = BoxStyle::new(Color::hex(0xf4dce1), 10.0);
pub(crate) const HINT_SQUARE: BoxStyle<'static> = BoxStyle::new(Color::css(0xfff1e8e8), 8.0)
    .border(1.0, Color::css(0xffffff88))
    .shadows(&[Shadow::new(0.0, 2.0, 7.0, 0.0, Color::css(0x24143526))]);
pub(crate) const HARDCORE_REGENERATE: BoxStyle<'static> =
    BoxStyle::new(Color::css(0x39254885), 8.0).border(1.0, LINE);
pub(crate) const CELL_SHADOWS: &[Shadow] = &[
    Shadow::new(0.0, 2.0, 0.0, 0.0, Color::css(0x44265330)),
    Shadow::new(0.0, 4.0, 6.0, 0.0, Color::css(0x24143526)),
];
pub(crate) const CELL: BoxStyle<'static> = BoxStyle::new(Color::hex(0xfff9f1).opacity(0.97), 0.0)
    .relative_radius(0.18)
    .shadows(CELL_SHADOWS);
pub(crate) const CELL_REVEALED: BoxStyle<'static> = CELL.with_color(Color::hex(0x8551ad));
pub(crate) const CELL_PARTIAL: BoxStyle<'static> = CELL.with_color(Color::hex(0xdfbfe9));
pub(crate) const CELL_REPEATED: BoxStyle<'static> = CELL_REVEALED.shadows(&[
    Shadow::new(0.0, 0.0, 0.0, 2.0, Color::hex(0x96f2ad)).inset(),
    Shadow::new(0.0, 0.0, 0.0, 2.0, Color::hex(0x43cc77)),
    Shadow::new(0.0, 0.0, 12.0, 0.0, Color::css(0x43cc7780)),
]);
pub(crate) const PREVIEW_CELL: BoxStyle<'static> =
    BoxStyle::new(Color::hex(0xfff9f1).opacity(0.97), 6.0).shadows(CELL_SHADOWS);
pub(crate) const WHEEL: BoxStyle<'static> = BoxStyle::new(Color::hex(0xfff9f5).opacity(0.84), 0.0)
    .relative_radius(0.5)
    .shadows(&[Shadow::new(0.0, 10.0, 35.0, 0.0, Color::css(0x17162918))])
    .blur(1.2);
pub(crate) const LETTER: BoxStyle<'static> = BoxStyle::new(Color::css(0xfff9f3c9), 0.0)
    .relative_radius(0.5)
    .border(1.5, Color::css(0x956fa86b))
    .shadows(&[Shadow::new(0.0, 2.0, 4.0, 0.0, Color::css(0x42295317))]);
pub(crate) const LETTER_SELECTED: BoxStyle<'static> = LETTER
    .with_color(Color::hex(0x8550ac))
    .border(1.5, Color::hex(0x8550ac))
    .shadows(&[Shadow::new(0.0, 3.0, 12.0, 0.0, Color::css(0x7945a255))]);
pub(crate) const LETTER_IDLE: BoxStyle<'static> = LETTER
    .with_color(Color::hex(0xf5d16a))
    .border(1.5, Color::hex(0xe6ac25))
    .shadows(&[
        Shadow::new(0.0, 0.0, 0.0, 3.0, Color::hex(0xfff0b6)),
        Shadow::new(0.0, 0.0, 20.0, 0.0, Color::hex(0xf5cf75)),
    ]);
pub(crate) const WORD_ROW: BoxStyle<'static> = BoxStyle::new(Color::hex(0xf6edf5), 12.0);
pub(crate) const VICTORY_WORD_ROW: BoxStyle<'static> =
    WORD_ROW.with_color(Color::hex(0xfffaf5).opacity(0.93));
pub(crate) const WORD_ROW_BUTTON: BoxStyle<'static> = BoxStyle::new(Color::hex(0xe4d3ea), 10.0);
pub(crate) const SWITCH: BoxStyle<'static> = BoxStyle::new(Color::hex(0xd9c5df), 30.0);
pub(crate) const SWITCH_ACTIVE: BoxStyle<'static> = SWITCH.with_color(Color::hex(0x8b51aa));
pub(crate) const SWITCH_KNOB: BoxStyle<'static> = BoxStyle::new(WHITE, 0.0)
    .relative_radius(0.5)
    .shadows(&[Shadow::new(0.0, 2.0, 3.0, 0.0, Color::css(0x553c5922))]);
pub(crate) const BACKGROUND_CARD: BoxStyle<'static> =
    BoxStyle::new(Color::hex(0xeee4ef), 12.0).border(2.0, TRANSPARENT);
pub(crate) const BACKGROUND_CARD_SELECTED: BoxStyle<'static> = BACKGROUND_CARD
    .with_color(Color::hex(0xfff2d9))
    .border(2.0, Color::hex(0xaa7727));
pub(crate) const TOAST: BoxStyle<'static> = BoxStyle::new(Color::hex(0xfff7ed), 12.0)
    .shadows(&[Shadow::new(0.0, 8.0, 30.0, 0.0, Color::css(0x23173830))]);
pub(crate) const VICTORY_BADGE: BoxStyle<'static> = BoxStyle::new(Color::hex(0xffda8e), 0.0)
    .relative_radius(0.5)
    .shadows(&[Shadow::new(0.0, 0.0, 0.0, 12.0, Color::css(0xffe9c322))]);
pub(crate) const VICTORY_BADGE_COMPACT: BoxStyle<'static> =
    VICTORY_BADGE.shadows(&[Shadow::new(0.0, 0.0, 0.0, 6.0, Color::css(0xffe9c322))]);
pub(crate) const REWARD_CHIP: BoxStyle<'static> =
    BoxStyle::new(Color::css(0xffffff20), 12.0).border(1.0, LINE);
pub(crate) const ABOUT_STATS: BoxStyle<'static> = BoxStyle::new(Color::hex(0xeddfef), 12.0);

#[derive(Clone, Copy, Debug)]
pub(crate) enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TextStyle<'shadow> {
    pub(crate) size: f32,
    pub(crate) bold: bool,
    pub(crate) color: Color,
    pub(crate) align: Align,
    pub(crate) letter_spacing: f32,
    pub(crate) shadows: &'shadow [Shadow],
}

impl TextStyle<'_> {
    pub(crate) const fn new(size: f32, color: Color) -> Self {
        Self {
            size,
            bold: false,
            color,
            align: Align::Left,
            letter_spacing: 0.0,
            shadows: &[],
        }
    }

    pub(crate) const fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub(crate) const fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    pub(crate) const fn spacing(mut self, spacing: f32) -> Self {
        self.letter_spacing = spacing;
        self
    }

    pub(crate) const fn shadows(self, shadows: &[Shadow]) -> TextStyle<'_> {
        TextStyle { shadows, ..self }
    }

    pub(crate) fn width(self, value: &str) -> f32 {
        crate::raster::measure_text(value, font_size(self.size), self.bold, self.letter_spacing)
    }
}

pub(crate) fn text(x: f32, baseline: f32, value: &str, style: TextStyle<'_>) -> Value {
    let align = match style.align {
        Align::Left => "left",
        Align::Center => "center",
        Align::Right => "right",
    };
    let mut command = json!({
        "kind":"text","x":x,"y":baseline,"text":value,
        "size":font_size(style.size),"bold":style.bold,"color":style.color.argb(),
        "align":align,"letterSpacing":style.letter_spacing,
    });
    if !style.shadows.is_empty() {
        command["shadows"] = json!(style.shadows.iter().map(|s| s.json()).collect::<Vec<_>>());
    }
    command
}

pub(crate) fn text_width(value: &str, size: f32, bold: bool) -> f32 {
    TextStyle {
        bold,
        ..TextStyle::new(size, WHITE)
    }
    .width(value)
}

thread_local! {
    static FONT_DENSITY: Cell<f32> = const { Cell::new(2.0) };
}

pub(crate) fn with_font_density<T>(density: f32, work: impl FnOnce() -> T) -> T {
    struct Restore<'a>(&'a Cell<f32>, f32);

    impl Drop for Restore<'_> {
        fn drop(&mut self) {
            self.0.set(self.1);
        }
    }

    FONT_DENSITY.with(|slot| {
        let density = if density.is_finite() && density > 0.0 {
            density
        } else {
            slot.get()
        };
        let _restore = Restore(slot, slot.replace(density));
        work()
    })
}

fn font_size(size: f32) -> f32 {
    size.max(8.0)
}

fn font_layout_metrics(size: f32) -> (f32, f32) {
    // Android WebView 124 measured the same metrics for both bundled Ubuntu
    // weights. Each little-endian record holds the normal inline box height
    // and its alphabetic baseline in device pixels, from 8 to 512 ppem at
    // quarter-pixel intervals.
    const METRICS: &[u8; 8068] = include_bytes!("theme/ubuntu_android_metrics.bin");
    const FIRST_SAMPLE: usize = 32;
    const LAST_SAMPLE: usize = 2048;

    let density = FONT_DENSITY.get();
    let pixels = font_size(size) * density;
    let sample = (pixels * 4.0).round() as usize;
    if (FIRST_SAMPLE..=LAST_SAMPLE).contains(&sample) {
        let index = (sample - FIRST_SAMPLE) * 4;
        let height = u16::from_le_bytes([METRICS[index], METRICS[index + 1]]);
        let baseline = u16::from_le_bytes([METRICS[index + 2], METRICS[index + 3]]);
        return (f32::from(height) / density, f32::from(baseline) / density);
    }

    // The fonts share ascender, descender, and line-gap values of 932/189/28
    // units per em. Use those metrics at sizes outside the sampled range.
    let ascender = (pixels * 0.932).round();
    let descender = (pixels * 0.189).round();
    let gap = (pixels * 0.028).round();
    (
        (ascender + descender + gap) / density,
        (ascender + (gap / 2.0).floor()) / density,
    )
}

pub(crate) fn line_height(size: f32, _bold: bool) -> f32 {
    font_layout_metrics(size).0
}

pub(crate) fn baseline_offset(size: f32, _bold: bool, line: f32) -> f32 {
    let (height, baseline) = font_layout_metrics(size);
    baseline + (line - height) / 2.0
}

pub(crate) fn wrap(value: &str, width: f32, size: f32, bold: bool) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in value.lines() {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if text_width(&candidate, size, bold) <= width {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            for character in word.chars() {
                line.push(character);
                if line.len() != character.len_utf8() && text_width(&line, size, bold) > width {
                    line.pop();
                    lines.push(std::mem::take(&mut line));
                    line.push(character);
                }
            }
        }
        lines.push(line);
    }
    lines
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Icon {
    Menu,
    Close,
    Shuffle,
    Bulb,
    Book,
    Trophy,
    Regenerate,
}

impl Icon {
    fn path(self) -> &'static str {
        match self {
            Self::Menu => "M4 6h16M4 12h16M4 18h16",
            Self::Close => "m6 6 12 12M18 6 6 18",
            Self::Shuffle => {
                "M3 7h3c5 0 7 10 12 10h3m-4-3 3 3-3 3M3 17h3c2 0 4-3 6-6s4-4 6-4h3m-4-3 3 3-3 3"
            }
            Self::Bulb => "M9 18h6m-5 3h4M8 14c-5-5-2-11 4-11s9 6 4 11l-1 2H9z",
            Self::Book => "M12 5v15M3 4c4-1 6 0 9 2 3-2 5-3 9-2v14c-4-1-6 0-9 2-3-2-5-3-9-2z",
            Self::Trophy => {
                "M8 3h8v7c0 6-8 6-8 0zM8 5H4v3c0 3 3 4 5 4m7-7h4v3c0 3-3 4-5 4M12 15v5m-4 1h8"
            }
            Self::Regenerate => {
                "M20 7v5h-5M4 17v-5h5M6 7a7 7 0 0 1 12-1l2 3M4 15l2 3a7 7 0 0 0 12-1"
            }
        }
    }
}

/// x/y are the top-left of the 24×24 SVG display box.
pub(crate) fn icon(x: f32, y: f32, size: f32, icon: Icon, color: Color) -> Value {
    let stroke = if matches!(icon, Icon::Shuffle) {
        1.7
    } else {
        1.8
    };
    json!({
        "kind":"path","x":x,"y":y,"w":size,"h":size,"d":icon.path(),
        "viewBox":[24,24],"stroke":stroke,"color":color.argb(),
    })
}

pub(crate) fn image(x: f32, y: f32, w: f32, h: f32, path: &str) -> Value {
    json!({
        "kind":"image","x":x,"y":y,"w":w,"h":h,
        "path":path,"fit":"cover","positionX":0.5,"positionY":0.5,"alpha":1,
    })
}

pub(crate) fn coin(x: f32, y: f32, size: f32) -> Value {
    let mut command = image(x, y, size, size, "assets/coin.png");
    command["radius"] = json!(size * 0.5);
    command["clipCircle"] = json!({"radius":0.48,"x":0.50,"y":0.506});
    command["shadows"] = json!([Shadow::new(0.0, 1.0, 1.0, 0.0, Color::css(0x58340055)).json()]);
    command
}

pub(crate) fn background_card(x: f32, y: f32, w: f32, h: f32, path: &str, radius: f32) -> Value {
    let mut command = image(x, y, w, h, path);
    command["positionX"] = json!(0.666667);
    command["radius"] = json!(radius);
    command
}

pub(crate) fn wallpaper(width: f32, height: f32, path: &str, dx: f32, dy: f32) -> [Value; 3] {
    let w = width + 48.0;
    let h = height + 48.0;
    // Wallpaper assets retain their native 1440 × 1080 dimensions. This is the
    // JS positionBackground formula, including the overscan of .background-layer.
    let scaled_w = w.max(h * (1440.0 / 1080.0));
    let offset = (w / 2.0 - scaled_w * (2.0 / 3.0)).clamp(w - scaled_w, 0.0);
    let mut background = image(-24.0 + dx, -24.0 + dy, w, h, path);
    background["offsetX"] = json!(offset);
    background["positionY"] = json!(0.65);
    let gradient = json!({
        "kind":"linear_gradient","x":0,"y":0,"w":width,"h":height,"from":[0,0],"to":[0,1],
        "stops":[
            [0.0,[35,19,70,0.48]],
            [0.45,[63,30,70,0.18]],
            [1.0,[18,22,58,0.45]],
        ],
    });
    [
        BoxStyle::new(PAGE_BACKDROP, 0.0).emit(0.0, 0.0, width, height),
        background,
        gradient,
    ]
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Bezier(
    pub(crate) f32,
    pub(crate) f32,
    pub(crate) f32,
    pub(crate) f32,
);

impl Bezier {
    pub(crate) const EASE_IN: Self = Self(0.42, 0.0, 1.0, 1.0);
    pub(crate) const EASE_OUT: Self = Self(0.0, 0.0, 0.58, 1.0);
    pub(crate) const EASE_IN_OUT: Self = Self(0.42, 0.0, 0.58, 1.0);
    pub(crate) const WINDOW: Self = Self(0.2, 0.8, 0.25, 1.0);
    pub(crate) const SHUFFLE: Self = Self(0.22, 0.7, 0.25, 1.0);
    pub(crate) const GIFT: Self = Self(0.25, 0.65, 0.4, 1.0);

    pub(crate) fn sample(self, position: f32) -> f32 {
        let x = position.clamp(0.0, 1.0);
        if x == 0.0 || x == 1.0 {
            return x;
        }
        let sample_axis = |t: f32, first: f32, second: f32| {
            let u = 1.0 - t;
            3.0 * u * u * t * first + 3.0 * u * t * t * second + t * t * t
        };
        let (mut left, mut right) = (0.0, 1.0);
        for _ in 0..22 {
            let midpoint = (left + right) * 0.5;
            if sample_axis(midpoint, self.0, self.2) < x {
                left = midpoint;
            } else {
                right = midpoint;
            }
        }
        sample_axis((left + right) * 0.5, self.1, self.3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_css_alpha_and_gradient_positions() {
        assert_eq!(
            MENU_SECONDARY.emit(0.0, 0.0, 320.0, 46.0)["color"],
            "#47ffffff"
        );
        assert_eq!(LETTER.emit(0.0, 0.0, 42.0, 42.0)["radius"], 21.0);
        assert_eq!(LETTER.emit(0.0, 0.0, 42.0, 42.0)["color"], "#c9fff9f3");
        let [_, image, gradient] = wallpaper(360.0, 524.0, "assets/background-ufa.webp", 0.0, 0.0);
        assert_eq!(image["w"], 408.0);
        assert_eq!(image["x"], -24.0);
        assert!((image["offsetX"].as_f64().unwrap() + 2740.0 / 9.0).abs() < 0.01);
        assert_eq!(gradient["stops"][1], json!([0.45, [63, 30, 70, 0.18]]));
    }

    #[test]
    fn cubic_bezier_easing_resolves_time_axis_and_keeps_endpoints() {
        assert_eq!(Bezier::GIFT.sample(0.0), 0.0);
        assert_eq!(Bezier::GIFT.sample(1.0), 1.0);
        assert!((Bezier::EASE_IN_OUT.sample(0.5) - 0.5).abs() < 0.0001);
        assert!(Bezier::WINDOW.sample(0.5) > 0.8);
        assert!(Bezier::GIFT.sample(0.35) > 0.65);
    }

    #[test]
    fn uses_the_original_android_ubuntu_line_height_and_baseline() {
        assert_eq!(line_height(16.0, false), 18.5);
        assert_eq!(line_height(19.0, true), 21.5);
        assert_eq!(line_height(28.0, true), 32.5);
        assert_eq!(baseline_offset(19.0, true, 23.0), 17.75);
        assert_eq!(line_height(62.0, true), 71.0);
        assert_eq!(baseline_offset(62.0, true, 62.0), 53.5);
        with_font_density(1.5, || {
            assert_eq!(line_height(16.0, false), 28.0 / 1.5);
            assert_eq!(line_height(7.0, true), line_height(8.0, true));
        });
        assert_eq!(line_height(16.0, false), 18.5);
    }
}
