use std::collections::HashMap;
use std::sync::OnceLock;

use fontdue::{Font, FontSettings, Metrics};
use serde_json::Value;
use svgtypes::{SimplePathSegment, SimplifyingPathParser};
use tiny_skia::{
    Color, FillRule, FilterQuality, GradientStop, IntSize, LineCap, LineJoin, LinearGradient, Mask,
    Paint, Path, PathBuilder, Pixmap, PixmapMut, PixmapPaint, Point, Rect, SpreadMode, Stroke,
    Transform,
};

const REGULAR: &[u8] = include_bytes!("../../../public/assets/Ubuntu-R.ttf");
const BOLD: &[u8] = include_bytes!("../../../public/assets/Ubuntu-B.ttf");
const FALLBACK: [u8; 4] = [52, 32, 72, 255];

static FONTS: OnceLock<[Font; 2]> = OnceLock::new();

fn font(bold: bool) -> &'static Font {
    let fonts = FONTS.get_or_init(|| {
        [REGULAR, BOLD].map(|source| {
            Font::from_bytes(source, FontSettings::default()).expect("bundled Ubuntu TTF")
        })
    });
    &fonts[usize::from(bold)]
}

pub fn measure_text(text: &str, size: f32, bold: bool, letter_spacing: f32) -> f32 {
    let font = font(bold);
    let mut width = 0.0;
    let mut previous = None;
    for letter in text.chars() {
        if let Some(left) = previous {
            width += font.horizontal_kern(left, letter, size).unwrap_or_default();
        }
        width += font.metrics(letter, size).advance_width + letter_spacing;
        previous = Some(letter);
    }
    width.max(0.0)
}

struct Glyph {
    metrics: Metrics,
    coverage: Vec<u8>,
}

type GlyphKey = (bool, char, u32);

#[derive(Default)]
pub struct Renderer {
    images: HashMap<String, Option<Pixmap>>,
    coin_samples: HashMap<(u32, u32), Pixmap>,
    glyphs: HashMap<GlyphKey, Glyph>,
    frame_size: Option<(u32, u32, u32)>,
}

#[derive(Clone, Copy)]
struct BoxArea {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl BoxArea {
    fn read(command: &Value) -> Self {
        Self {
            x: number(command, "x", 0.0),
            y: number(command, "y", 0.0),
            w: number(command, "w", 0.0),
            h: number(command, "h", 0.0),
        }
    }

    fn inflate(self, delta: f32) -> Self {
        Self {
            x: self.x - delta,
            y: self.y - delta,
            w: self.w + delta * 2.0,
            h: self.h + delta * 2.0,
        }
    }

    fn translated(self, x: f32, y: f32) -> Self {
        Self {
            x: self.x + x,
            y: self.y + y,
            ..self
        }
    }
}

impl Renderer {
    pub fn paint(
        &mut self,
        frame: &Value,
        target: &mut PixmapMut<'_>,
        density: f32,
        mut load_asset: impl FnMut(&str) -> Option<Vec<u8>>,
    ) {
        if !density.is_finite() || density <= 0.0 {
            return;
        }
        let key = (target.width(), target.height(), density.to_bits());
        if self.frame_size != Some(key) {
            self.coin_samples.clear();
        }
        if self.frame_size != Some(key) || self.glyphs.len() > 2048 {
            self.glyphs.clear();
            self.frame_size = Some(key);
        }
        target.fill(sk_color(color(frame, "background", FALLBACK)));
        let Some(commands) = frame.get("commands").and_then(Value::as_array) else {
            return;
        };
        self.paint_commands(commands, target, density, &mut load_asset);
    }

    fn paint_commands(
        &mut self,
        mut commands: &[Value],
        target: &mut PixmapMut<'_>,
        density: f32,
        load_asset: &mut impl FnMut(&str) -> Option<Vec<u8>>,
    ) {
        let transform = Transform::from_scale(density, density);
        let mut clips: Vec<Mask> = Vec::new();
        while let Some((command, rest)) = commands.split_first() {
            commands = rest;
            let area = BoxArea::read(command);
            let radius = number(command, "radius", 0.0);
            let clip = clips.last();
            let rgba = color(command, "color", [255; 4]);
            match command.get("kind").and_then(Value::as_str) {
                Some("fill") => {
                    if let Some(bounds) =
                        Rect::from_xywh(0.0, 0.0, target.width() as f32, target.height() as f32)
                    {
                        target.fill_rect(bounds, &solid(rgba), Transform::identity(), clip);
                    }
                }
                Some("rect" | "stroke_rect") => {
                    let Some(path) = rounded_path(area, radius) else {
                        continue;
                    };
                    if command.get("kind").and_then(Value::as_str) == Some("stroke_rect") {
                        target.stroke_path(
                            &path,
                            &solid(rgba),
                            &stroke(number(command, "stroke", 1.0)),
                            transform,
                            clip,
                        );
                        continue;
                    }
                    if number(command, "backdropBlur", 0.0) > 0.0 {
                        backdrop_blur(
                            target,
                            area,
                            &path,
                            number(command, "backdropBlur", 0.0),
                            density,
                            clip,
                        );
                    }
                    if let Some(list) = command.get("shadows").and_then(Value::as_array) {
                        for item in list.iter().rev().filter(|item| !flag(item, "inset")) {
                            shadow(target, area, radius, item, density, clip, false);
                        }
                    }
                    let blur_sigma = number(command, "blur", 0.0);
                    if blur_sigma > 0.0 {
                        filtered_fill(target, area, &path, rgba, blur_sigma, density, clip);
                    } else {
                        target.fill_path(&path, &solid(rgba), FillRule::Winding, transform, clip);
                    }
                    if let Some(list) = command.get("shadows").and_then(Value::as_array) {
                        for item in list.iter().rev().filter(|item| flag(item, "inset")) {
                            shadow(target, area, radius, item, density, clip, true);
                        }
                    }
                    if let Some(border) = command.get("border") {
                        let weight = number(border, "width", 0.0);
                        if weight > 0.0 {
                            if let Some(path) =
                                rounded_path(area.inflate(-weight * 0.5), radius - weight * 0.5)
                            {
                                target.stroke_path(
                                    &path,
                                    &solid(color(border, "color", [255; 4])),
                                    &stroke(weight),
                                    transform,
                                    clip,
                                );
                            }
                        }
                    }
                }
                Some("line") => {
                    let mut b = PathBuilder::new();
                    b.move_to(number(command, "x1", 0.0), number(command, "y1", 0.0));
                    b.line_to(number(command, "x2", 0.0), number(command, "y2", 0.0));
                    if let Some(path) = b.finish() {
                        target.stroke_path(
                            &path,
                            &solid(rgba),
                            &stroke(number(command, "stroke", 1.0)),
                            transform,
                            clip,
                        );
                    }
                }
                Some("circle") => {
                    let r = number(command, "r", 0.0);
                    if let Some(path) = PathBuilder::from_circle(area.x, area.y, r) {
                        target.fill_path(&path, &solid(rgba), FillRule::Winding, transform, clip);
                    }
                }
                Some("clip") => {
                    if let Some(path) = rounded_path(area, radius) {
                        if let Some(mask) = intersect(target, clip, &path, transform) {
                            clips.push(mask);
                        }
                    }
                }
                Some("restore") => {
                    clips.pop();
                }
                Some("text") => self.draw_text(target, command, density, clip),
                Some("image") => self.draw_image(target, command, area, density, clip, load_asset),
                Some("linear_gradient") => gradient(target, command, area, radius, transform, clip),
                Some("path") => svg_path(target, command, area, density, clip),
                Some("transform") => {
                    let mut nesting = 1;
                    let end = commands
                        .iter()
                        .position(|item| {
                            match item.get("kind").and_then(Value::as_str) {
                                Some("clip" | "transform") => nesting += 1,
                                Some("restore") => nesting -= 1,
                                _ => {}
                            }
                            nesting == 0
                        })
                        .unwrap_or(commands.len());
                    let (group, after) = commands.split_at(end);
                    if let Some(mut layer) = Pixmap::new(target.width(), target.height()) {
                        self.paint_commands(group, &mut layer.as_mut(), density, load_asset);
                        let alpha = number(command, "alpha", 1.0).clamp(0.0, 1.0);
                        let paint = PixmapPaint {
                            opacity: alpha,
                            quality: FilterQuality::Bilinear,
                            ..PixmapPaint::default()
                        };
                        target.draw_pixmap(
                            0,
                            0,
                            layer.as_ref(),
                            &paint,
                            group_transform(command, density),
                            clip,
                        );
                    }
                    commands = after.get(1..).unwrap_or_default();
                }
                _ => {}
            }
        }
    }

    fn draw_text(
        &mut self,
        target: &mut PixmapMut<'_>,
        command: &Value,
        density: f32,
        clip: Option<&Mask>,
    ) {
        let text = command.get("text").and_then(Value::as_str).unwrap_or("");
        let size = (number(command, "size", 16.0) * density).clamp(0.5, 512.0);
        let bold = flag(command, "bold");
        let font = font(bold);
        let letter_spacing = number(command, "letterSpacing", 0.0) * density;
        let width = measure_text(text, size, bold, letter_spacing);
        let align = match command.get("align").and_then(Value::as_str) {
            Some("center") => 0.5,
            Some("right") => 1.0,
            _ => 0.0,
        };
        let mut x = number(command, "x", 0.0) * density - width * align;
        let y = number(command, "y", 0.0) * density;
        let rgba = color(command, "color", [255; 4]);
        let mut previous = None;
        let mut run = Vec::new();
        for letter in text.chars() {
            if let Some(left) = previous {
                x += font.horizontal_kern(left, letter, size).unwrap_or_default();
            }
            let key = (bold, letter, size.to_bits());
            let glyph = self.glyphs.entry(key).or_insert_with(|| {
                let (metrics, coverage) = font.rasterize(letter, size);
                Glyph { metrics, coverage }
            });
            let left = x.round() as i32 + glyph.metrics.xmin;
            let top = y.round() as i32 - glyph.metrics.ymin - glyph.metrics.height as i32;
            run.push((key, left, top));
            x += glyph.metrics.advance_width + letter_spacing;
            previous = Some(letter);
        }
        if let Some(shadows) = command.get("shadows").and_then(Value::as_array) {
            for effect in shadows.iter().rev() {
                text_shadow(target, &self.glyphs, &run, effect, density, clip);
            }
        }
        for (key, left, top) in run {
            let glyph = &self.glyphs[&key];
            blend_mask(
                target,
                &glyph.coverage,
                glyph.metrics.width,
                left,
                top,
                rgba,
                clip,
            );
        }
    }

    fn draw_image(
        &mut self,
        target: &mut PixmapMut<'_>,
        command: &Value,
        area: BoxArea,
        density: f32,
        clip: Option<&Mask>,
        load_asset: &mut impl FnMut(&str) -> Option<Vec<u8>>,
    ) {
        let Some(name) = command.get("path").and_then(Value::as_str) else {
            return;
        };
        if !name.starts_with("assets/") || name.contains("..") || name.contains('\\') {
            return;
        }
        let Some(image) = self
            .images
            .entry(name.to_owned())
            .or_insert_with(|| load_asset(name).and_then(|bytes| decode_image(&bytes)))
        else {
            return;
        };
        let mut sx = area.w / image.width() as f32;
        let mut sy = area.h / image.height() as f32;
        match command.get("fit").and_then(Value::as_str) {
            Some("stretch") => {}
            Some("contain") => {
                sx = sx.min(sy);
                sy = sx;
            }
            _ => {
                sx = sx.max(sy);
                sy = sx;
            }
        }
        let x = area.x
            + command.get("offsetX").and_then(Value::as_f64).map_or_else(
                || (area.w - image.width() as f32 * sx) * number(command, "positionX", 0.5),
                |value| value as f32,
            );
        let y = area.y + (area.h - image.height() as f32 * sy) * number(command, "positionY", 0.5);
        let mut scale_x = sx * density;
        let mut scale_y = sy * density;
        let image = if name == "assets/coin.png" && scale_x < 0.5 && scale_y < 0.5 {
            // Bilinear sampling alone skips most of the texture when the large
            // coin artwork is drawn at its UI size. Cache a premultiplied area
            // sample and keep the original placement for subpixel composition.
            let draw_width = scale_x * image.width() as f32;
            let draw_height = scale_y * image.height() as f32;
            let size = (draw_width.round() as u32, draw_height.round() as u32);
            let sample = match self.coin_samples.entry(size) {
                std::collections::hash_map::Entry::Occupied(entry) => Some(entry.into_mut()),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    area_resample(image, size).map(|sample| entry.insert(sample))
                }
            };
            if let Some(sample) = sample {
                scale_x = draw_width / sample.width() as f32;
                scale_y = draw_height / sample.height() as f32;
                sample
            } else {
                image
            }
        } else {
            image
        };
        let transform = Transform::from_row(scale_x, 0.0, 0.0, scale_y, x * density, y * density);
        let paint = PixmapPaint {
            opacity: number(command, "alpha", 1.0).clamp(0.0, 1.0),
            quality: FilterQuality::Bilinear,
            ..PixmapPaint::default()
        };
        let circle = command.get("clipCircle");
        let radius = number(command, "radius", 0.0);
        let path = if let Some(circle) = circle {
            let cx = area.x + area.w * number(circle, "x", 0.5);
            let cy = area.y + area.h * number(circle, "y", 0.5);
            PathBuilder::from_circle(cx, cy, area.w.min(area.h) * number(circle, "radius", 0.5))
        } else {
            rounded_path(area, radius)
        };
        let Some(path) = path else { return };
        let Some(clipped) = intersect(target, clip, &path, Transform::from_scale(density, density))
        else {
            return;
        };
        if let Some(shadows) = command.get("shadows").and_then(Value::as_array) {
            let geometry = ImageShadowGeometry {
                area,
                radius,
                transform,
                density,
            };
            let shadow_clip = if circle.is_some() {
                Some(&clipped)
            } else {
                clip
            };
            for effect in shadows.iter().rev() {
                image_shadow(target, image, &paint, effect, &geometry, shadow_clip);
            }
        }
        target.draw_pixmap(0, 0, image.as_ref(), &paint, transform, Some(&clipped));
    }
}

fn group_transform(command: &Value, density: f32) -> Transform {
    let origin_x = number(command, "originX", 0.0) * density;
    let origin_y = number(command, "originY", 0.0) * density;
    let x = number(command, "translateX", 0.0) * density;
    let y = number(command, "translateY", 0.0) * density;
    let scale = number(command, "scale", 1.0);
    let (sin, cos) = number(command, "rotate", 0.0).to_radians().sin_cos();
    let a = scale * cos;
    let b = scale * sin;
    Transform::from_row(
        a,
        b,
        -b,
        a,
        origin_x + x - a * origin_x + b * origin_y,
        origin_y + y - b * origin_x - a * origin_y,
    )
}

fn number(value: &Value, key: &str, default: f32) -> f32 {
    value
        .get(key)
        .and_then(Value::as_f64)
        .map(|value| value as f32)
        .filter(|value| value.is_finite())
        .unwrap_or(default)
}

fn flag(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn parse_color(text: &str) -> Option<[u8; 4]> {
    let hex = text.strip_prefix('#')?;
    let raw = u32::from_str_radix(hex, 16).ok()?;
    let [a, r, g, b] = raw.to_be_bytes();
    match hex.len() {
        6 => Some([r, g, b, 255]),
        8 => Some([r, g, b, a]),
        _ => None,
    }
}

fn color(value: &Value, key: &str, default: [u8; 4]) -> [u8; 4] {
    value
        .get(key)
        .and_then(Value::as_str)
        .and_then(parse_color)
        .unwrap_or(default)
}

fn sk_color([r, g, b, a]: [u8; 4]) -> Color {
    Color::from_rgba8(r, g, b, a)
}

fn solid(rgba: [u8; 4]) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(sk_color(rgba));
    paint
}

fn stroke(width: f32) -> Stroke {
    Stroke {
        width: width.max(0.0),
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Stroke::default()
    }
}

fn rounded_path(area: BoxArea, radius: f32) -> Option<Path> {
    let rect = Rect::from_xywh(area.x, area.y, area.w, area.h)?;
    let r = radius.clamp(0.0, area.w.min(area.h) * 0.5);
    if r < 0.01 {
        return Some(PathBuilder::from_rect(rect));
    }
    let (x, y, right, bottom) = (rect.left(), rect.top(), rect.right(), rect.bottom());
    let k = r * 0.5522848;
    let mut p = PathBuilder::new();
    p.move_to(x + r, y);
    p.line_to(right - r, y);
    p.cubic_to(right - r + k, y, right, y + r - k, right, y + r);
    p.line_to(right, bottom - r);
    p.cubic_to(
        right,
        bottom - r + k,
        right - r + k,
        bottom,
        right - r,
        bottom,
    );
    p.line_to(x + r, bottom);
    p.cubic_to(x + r - k, bottom, x, bottom - r + k, x, bottom - r);
    p.line_to(x, y + r);
    p.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    p.close();
    p.finish()
}

fn intersect(
    target: &PixmapMut<'_>,
    clip: Option<&Mask>,
    path: &Path,
    transform: Transform,
) -> Option<Mask> {
    if let Some(clip) = clip {
        let mut next = clip.clone();
        next.intersect_path(path, FillRule::Winding, true, transform);
        Some(next)
    } else {
        let mut next = Mask::new(target.width(), target.height())?;
        next.fill_path(path, FillRule::Winding, true, transform);
        Some(next)
    }
}

fn gradient(
    target: &mut PixmapMut<'_>,
    command: &Value,
    area: BoxArea,
    radius: f32,
    transform: Transform,
    clip: Option<&Mask>,
) {
    let point = |key: &str, fallback: [f32; 2]| {
        let coordinates = command.get(key).and_then(Value::as_array);
        let axis = |index: usize| {
            coordinates
                .and_then(|items| items.get(index))
                .and_then(Value::as_f64)
                .map_or(fallback[index], |value| value as f32)
        };
        Point::from_xy(area.x + area.w * axis(0), area.y + area.h * axis(1))
    };
    let css_stops: Vec<_> = command
        .get("stops")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|stop| {
            let color = stop.get(1)?;
            let rgba = if let Some(text) = color.as_str() {
                parse_color(text)?.map(|value| f32::from(value) / 255.0)
            } else {
                let red = color.get(0)?.as_f64()? as f32 / 255.0;
                let green = color.get(1)?.as_f64()? as f32 / 255.0;
                let blue = color.get(2)?.as_f64()? as f32 / 255.0;
                let alpha = color.get(3)?.as_f64()? as f32;
                [red, green, blue, alpha]
            };
            Some((stop.get(0)?.as_f64()? as f32, rgba))
        })
        .collect();
    let stops = css_gradient_stops(&css_stops);
    if let (Some(path), Some(shader)) = (
        rounded_path(area, radius),
        LinearGradient::new(
            point("from", [0.0, 0.0]),
            point("to", [0.0, 1.0]),
            stops,
            SpreadMode::Pad,
            Transform::identity(),
        ),
    ) {
        let paint = Paint {
            shader,
            ..Paint::default()
        };
        target.fill_path(&path, &paint, FillRule::Winding, transform, clip);
    }
}

fn css_gradient_stops(source: &[(f32, [f32; 4])]) -> Vec<GradientStop> {
    let mut stops = Vec::new();
    let stop_color = |rgba: [f32; 4]| Color::from_rgba(rgba[0], rgba[1], rgba[2], rgba[3]);
    if let Some(&(offset, rgba)) = source.first() {
        if let Some(color) = stop_color(rgba) {
            stops.push(GradientStop::new(offset, color));
        }
    }
    // CSS interpolates premultiplied colors. tiny-skia interpolates straight
    // colors, so subdivide translucent intervals using their premultiplied line.
    for window in source.windows(2) {
        let (start, from) = window[0];
        let (end, to) = window[1];
        let segments = if from[3] == to[3] || end <= start {
            1
        } else {
            24
        };
        for i in 1..=segments {
            let t = i as f32 / segments as f32;
            let alpha = from[3] * (1.0 - t) + to[3] * t;
            let mut rgba = [0.0, 0.0, 0.0, alpha];
            if alpha > 0.0 {
                for channel in 0..3 {
                    rgba[channel] =
                        ((from[channel] * from[3] * (1.0 - t) + to[channel] * to[3] * t) / alpha)
                            .clamp(0.0, 1.0);
                }
            }
            if let Some(color) = stop_color(rgba) {
                stops.push(GradientStop::new(start + (end - start) * t, color));
            }
        }
    }
    stops
}

fn svg_path(
    target: &mut PixmapMut<'_>,
    command: &Value,
    area: BoxArea,
    density: f32,
    clip: Option<&Mask>,
) {
    let Some(data) = command.get("d").and_then(Value::as_str) else {
        return;
    };
    let mut builder = PathBuilder::new();
    for segment in SimplifyingPathParser::from(data) {
        match segment {
            Ok(SimplePathSegment::MoveTo { x, y }) => builder.move_to(x as f32, y as f32),
            Ok(SimplePathSegment::LineTo { x, y }) => builder.line_to(x as f32, y as f32),
            Ok(SimplePathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            }) => builder.cubic_to(
                x1 as f32, y1 as f32, x2 as f32, y2 as f32, x as f32, y as f32,
            ),
            Ok(SimplePathSegment::Quadratic { x1, y1, x, y }) => {
                builder.quad_to(x1 as f32, y1 as f32, x as f32, y as f32)
            }
            Ok(SimplePathSegment::ClosePath) => builder.close(),
            Err(_) => return,
        }
    }
    let Some(path) = builder.finish() else { return };
    let view = |axis: usize| {
        command
            .get("viewBox")
            .and_then(|size| size.get(axis))
            .and_then(Value::as_f64)
            .filter(|value| *value > 0.0)
            .map_or(24.0, |value| value as f32)
    };
    let transform = Transform::from_row(
        area.w * density / view(0),
        0.0,
        0.0,
        area.h * density / view(1),
        area.x * density,
        area.y * density,
    );
    if let Some(fill) = command
        .get("fill")
        .and_then(Value::as_str)
        .and_then(parse_color)
    {
        target.fill_path(&path, &solid(fill), FillRule::Winding, transform, clip);
    }
    let weight = number(command, "stroke", 1.8);
    if weight > 0.0 {
        target.stroke_path(
            &path,
            &solid(color(command, "color", [255; 4])),
            &stroke(weight),
            transform,
            clip,
        );
    }
}

fn decode_image(bytes: &[u8]) -> Option<Pixmap> {
    let image = image::load_from_memory(bytes).ok()?.into_rgba8();
    let size = IntSize::from_wh(image.width(), image.height())?;
    let mut pixels = image.into_raw();
    for sample in pixels.chunks_exact_mut(4) {
        let alpha = u16::from(sample[3]);
        for channel in &mut sample[..3] {
            *channel = ((u16::from(*channel) * alpha + 127) / 255) as u8;
        }
    }
    Pixmap::from_vec(pixels, size)
}

fn area_resample(source: &Pixmap, size: (u32, u32)) -> Option<Pixmap> {
    let mut output = Pixmap::new(size.0, size.1)?;
    let (width, height) = (size.0 as usize, size.1 as usize);
    let (source_width, source_height) = (source.width() as usize, source.height() as usize);
    let source = source.data();
    let weight = source_width as u64 * source_height as u64;
    for (y, row) in output.data_mut().chunks_exact_mut(width * 4).enumerate() {
        let top = y * source_height;
        let bottom = top + source_height;
        for (x, pixel) in row.chunks_exact_mut(4).enumerate() {
            // The source pixel and output sample boundaries are scaled onto
            // the same integer grid, preserving fractional edge coverage.
            let left = x * source_width;
            let right = left + source_width;
            let mut sum = [0u64; 4];
            for sy in top / height..bottom.div_ceil(height) {
                let overlap_y = bottom.min((sy + 1) * height) - top.max(sy * height);
                for sx in left / width..right.div_ceil(width) {
                    let overlap_x = right.min((sx + 1) * width) - left.max(sx * width);
                    let overlap = overlap_x as u64 * overlap_y as u64;
                    let start = (sy * source_width + sx) * 4;
                    for (sum, value) in sum.iter_mut().zip(&source[start..start + 4]) {
                        *sum += u64::from(*value) * overlap;
                    }
                }
            }
            for (channel, sum) in pixel.iter_mut().zip(sum) {
                *channel = ((sum + weight / 2) / weight) as u8;
            }
        }
    }
    Some(output)
}

fn blend_mask(
    target: &mut PixmapMut<'_>,
    mask: &[u8],
    stride: usize,
    x: i32,
    y: i32,
    rgba: [u8; 4],
    clip: Option<&Mask>,
) {
    if stride == 0 || rgba[3] == 0 {
        return;
    }
    let width = target.width() as usize;
    let height = target.height() as i32;
    let bytes = target.data_mut();
    for (line, mask) in mask.chunks_exact(stride).enumerate() {
        let row = y + line as i32;
        if row < 0 || row >= height {
            continue;
        }
        for (index, &coverage) in mask.iter().enumerate() {
            let col = x + index as i32;
            if coverage == 0 || col < 0 || col >= width as i32 {
                continue;
            }
            let offset = row as usize * width + col as usize;
            let clip_alpha = clip.map_or(255, |clip| clip.data()[offset]);
            let alpha = u32::from(coverage) * u32::from(clip_alpha) * u32::from(rgba[3]) / 65_025;
            let out = &mut bytes[offset * 4..offset * 4 + 4];
            for channel in 0..3 {
                out[channel] = ((u32::from(rgba[channel]) * alpha
                    + u32::from(out[channel]) * (255 - alpha)
                    + 127)
                    / 255) as u8;
            }
            out[3] = (alpha + u32::from(out[3]) * (255 - alpha) / 255) as u8;
        }
    }
}

fn blur_pass(data: &mut [u8], width: usize, height: usize, radius: usize, vertical: bool) {
    if radius == 0 || width == 0 || height == 0 {
        return;
    }
    let input = data.to_vec();
    let (lines, len, step, line_step) = if vertical {
        (width, height, width, 1)
    } else {
        (height, width, 1, width)
    };
    let n = (radius * 2 + 1) as u32;
    for line in 0..lines {
        let start = line * line_step;
        let sample = |index: usize| u32::from(input[start + index.min(len - 1) * step]);
        let mut sum = sample(0) * (radius as u32 + 1);
        for i in 1..=radius {
            sum += sample(i);
        }
        for i in 0..len {
            data[start + i * step] = ((sum + n / 2) / n) as u8;
            sum += sample(i + radius + 1);
            sum -= sample(i.saturating_sub(radius));
        }
    }
}

fn blur(data: &mut [u8], width: usize, height: usize, sigma: f32) {
    let sigma = sigma.clamp(0.0, 192.0);
    if sigma <= 0.15 || width == 0 || height == 0 {
        return;
    }
    if sigma < 1.5 {
        let radius = (sigma * 3.0).ceil() as isize;
        let kernel: Vec<u32> = (-radius..=radius)
            .map(|i| ((-0.5 * (i as f32 / sigma).powi(2)).exp() * 4096.0).round() as u32)
            .collect();
        gaussian_pass(data, width, height, &kernel, false);
        gaussian_pass(data, width, height, &kernel, true);
        return;
    }

    // Three separable boxes with the same combined variance as the Gaussian.
    let variance = 12.0 * sigma * sigma;
    let smaller = (((variance / 3.0 + 1.0).sqrt() as usize).max(1) - 1) | 1;
    let width_f = smaller as f32;
    let small_count = ((variance - 3.0 * width_f * width_f - 12.0 * width_f - 9.0)
        / (-4.0 * width_f - 4.0))
        .round()
        .clamp(0.0, 3.0) as usize;
    for pass in 0..3 {
        let radius = smaller / 2 + usize::from(pass >= small_count);
        blur_pass(data, width, height, radius, false);
        blur_pass(data, width, height, radius, true);
    }
}

fn gaussian_pass(data: &mut [u8], width: usize, height: usize, kernel: &[u32], vertical: bool) {
    let input = data.to_vec();
    let (lines, length, step, line_step) = if vertical {
        (width, height, width, 1)
    } else {
        (height, width, 1, width)
    };
    let radius = (kernel.len() / 2) as isize;
    let scale: u32 = kernel.iter().sum();
    for line in 0..lines {
        let start = line * line_step;
        for pixel in 0..length {
            let sample: u32 = kernel
                .iter()
                .enumerate()
                .map(|(index, weight)| {
                    let offset =
                        (pixel as isize + index as isize - radius).clamp(0, length as isize - 1);
                    u32::from(input[start + offset as usize * step]) * weight
                })
                .sum();
            data[start + pixel * step] = ((sample + scale / 2) / scale) as u8;
        }
    }
}

fn filtered_fill(
    target: &mut PixmapMut<'_>,
    area: BoxArea,
    path: &Path,
    rgba: [u8; 4],
    sigma: f32,
    density: f32,
    clip: Option<&Mask>,
) {
    let sigma = sigma.clamp(0.0, 192.0) * density;
    let (left, top, width, height) = bounded_region(
        target,
        area.inflate((sigma * 3.0).ceil() / density),
        density,
    );
    let Some(mut mask) = Mask::new(width, height) else {
        return;
    };
    let transform = Transform::from_row(density, 0.0, 0.0, density, -(left as f32), -(top as f32));
    mask.fill_path(path, FillRule::Winding, true, transform);
    blur(mask.data_mut(), width as usize, height as usize, sigma);
    blend_mask(target, mask.data(), width as usize, left, top, rgba, clip);
}

fn shadow(
    target: &mut PixmapMut<'_>,
    area: BoxArea,
    radius: f32,
    effect: &Value,
    density: f32,
    clip: Option<&Mask>,
    inset: bool,
) {
    let sigma = number(effect, "blur", 0.0).max(0.0) * density / 2.0;
    let spread = number(effect, "spread", 0.0);
    let dx = number(effect, "dx", 0.0);
    let dy = number(effect, "dy", 0.0);
    let shape = area
        .inflate(if inset { -spread } else { spread })
        .translated(dx, dy);
    let Some(path) = rounded_path(shape, radius + if inset { -spread } else { spread }) else {
        return;
    };
    let region = if inset { area } else { shape }.inflate(
        (sigma * 3.0).ceil() / density
            + if inset {
                dx.abs().max(dy.abs()) + spread.abs()
            } else {
                0.0
            },
    );
    let (left, top, width, height) = bounded_region(target, region, density);
    let Some(mut alpha) = Mask::new(width, height) else {
        return;
    };
    let transform = Transform::from_row(density, 0.0, 0.0, density, -(left as f32), -(top as f32));
    alpha.fill_path(&path, FillRule::Winding, true, transform);
    if inset {
        for value in alpha.data_mut() {
            *value = 255 - *value;
        }
    }
    blur(alpha.data_mut(), width as usize, height as usize, sigma);
    let rgba = color(effect, "color", [0, 0, 0, 0]);
    if let (Some(mut silhouette), Some(path)) =
        (Mask::new(width, height), rounded_path(area, radius))
    {
        silhouette.fill_path(&path, FillRule::Winding, true, transform);
        for (shadow, coverage) in alpha.data_mut().iter_mut().zip(silhouette.data()) {
            let visible = if inset { *coverage } else { 255 - *coverage };
            *shadow = ((u16::from(*shadow) * u16::from(visible) + 127) / 255) as u8;
        }
    }
    blend_mask(target, alpha.data(), width as usize, left, top, rgba, clip);
}

struct ImageShadowGeometry {
    area: BoxArea,
    radius: f32,
    transform: Transform,
    density: f32,
}

fn image_shadow(
    target: &mut PixmapMut<'_>,
    image: &Pixmap,
    paint: &PixmapPaint,
    effect: &Value,
    shape: &ImageShadowGeometry,
    clip: Option<&Mask>,
) {
    let sigma = number(effect, "blur", 0.0).clamp(0.0, 192.0) * shape.density;
    let dx = (number(effect, "dx", 0.0) * shape.density).round() as i32;
    let dy = (number(effect, "dy", 0.0) * shape.density).round() as i32;
    let expansion = ((sigma * 3.0).ceil() + dx.abs().max(dy.abs()) as f32) / shape.density;
    let (left, top, width, height) =
        bounded_region(target, shape.area.inflate(expansion), shape.density);
    let (Some(mut contents), Some(mut bounds), Some(path)) = (
        Pixmap::new(width, height),
        Mask::new(width, height),
        rounded_path(shape.area, shape.radius),
    ) else {
        return;
    };
    let placement = Transform::from_row(
        shape.density,
        0.0,
        0.0,
        shape.density,
        -(left as f32),
        -(top as f32),
    );
    bounds.fill_path(&path, FillRule::Winding, true, placement);
    let sampling = shape
        .transform
        .post_translate(-(left as f32), -(top as f32));
    contents.draw_pixmap(0, 0, image.as_ref(), paint, sampling, Some(&bounds));
    let mut coverage: Vec<u8> = contents
        .data()
        .chunks_exact(4)
        .map(|pixel| pixel[3])
        .collect();
    blur(&mut coverage, width as usize, height as usize, sigma);
    blend_mask(
        target,
        &coverage,
        width as usize,
        left + dx,
        top + dy,
        color(effect, "color", [0, 0, 0, 0]),
        clip,
    );
}

fn text_shadow(
    target: &mut PixmapMut<'_>,
    glyphs: &HashMap<GlyphKey, Glyph>,
    run: &[(GlyphKey, i32, i32)],
    effect: &Value,
    density: f32,
    clip: Option<&Mask>,
) {
    let rendered = || {
        run.iter().filter_map(|(key, x, y)| {
            let glyph = glyphs.get(key)?;
            (glyph.metrics.width > 0 && glyph.metrics.height > 0).then_some((glyph, *x, *y))
        })
    };
    let mut outer = None;
    for (glyph, x, y) in rendered() {
        let next_right = x + glyph.metrics.width as i32;
        let next_bottom = y + glyph.metrics.height as i32;
        outer = Some(match outer {
            Some((left, top, right, bottom)) => (
                x.min(left),
                y.min(top),
                next_right.max(right),
                next_bottom.max(bottom),
            ),
            None => (x, y, next_right, next_bottom),
        });
    }
    let Some((left, top, right, bottom)) = outer else {
        return;
    };
    let sigma = (number(effect, "blur", 0.0) * density / 2.0).clamp(0.0, 192.0);
    let padding = (sigma * 3.0).ceil() as usize;
    let width = (right - left) as usize + padding * 2;
    let height = (bottom - top) as usize + padding * 2;
    let mut mask = vec![0; width * height];
    for (glyph, x, y) in rendered() {
        for (row, coverage) in glyph.coverage.chunks_exact(glyph.metrics.width).enumerate() {
            let offset =
                (padding + row + (y - top) as usize) * width + (x - left) as usize + padding;
            let visible = &mut mask[offset..offset + coverage.len()];
            for (destination, &source) in visible.iter_mut().zip(coverage) {
                *destination = source
                    + ((u16::from(*destination) * u16::from(255 - source) + 127) / 255) as u8;
            }
        }
    }
    blur(&mut mask, width, height, sigma);
    let dx = (number(effect, "dx", 0.0) * density).round() as i32;
    let dy = (number(effect, "dy", 0.0) * density).round() as i32;
    blend_mask(
        target,
        &mask,
        width,
        left + dx - padding as i32,
        top + dy - padding as i32,
        color(effect, "color", [0, 0, 0, 0]),
        clip,
    );
}

fn bounded_region(target: &PixmapMut<'_>, area: BoxArea, density: f32) -> (i32, i32, u32, u32) {
    let left = ((area.x * density).floor() as i32).clamp(0, target.width() as i32);
    let top = ((area.y * density).floor() as i32).clamp(0, target.height() as i32);
    let right = (((area.x + area.w) * density).ceil() as i32).clamp(left, target.width() as i32);
    let bottom = (((area.y + area.h) * density).ceil() as i32).clamp(top, target.height() as i32);
    (left, top, (right - left) as u32, (bottom - top) as u32)
}

fn backdrop_blur(
    target: &mut PixmapMut<'_>,
    area: BoxArea,
    path: &Path,
    radius: f32,
    density: f32,
    clip: Option<&Mask>,
) {
    let (left, top, width, height) = bounded_region(target, area.inflate(radius * 3.0), density);
    let Some(size) = IntSize::from_wh(width, height) else {
        return;
    };
    let stride = target.width() as usize * 4;
    let width = width as usize;
    let height = height as usize;
    let mut channels: [Vec<u8>; 4] = std::array::from_fn(|_| Vec::with_capacity(width * height));
    for y in top as usize..top as usize + height {
        let start = y * stride + left as usize * 4;
        for pixel in target.as_ref().data()[start..start + width * 4].chunks_exact(4) {
            for channel in 0..4 {
                channels[channel].push(pixel[channel]);
            }
        }
    }
    for channel in &mut channels {
        blur(channel, width, height, radius * density);
    }
    let [red, green, blue, alpha] = channels;
    let mut pixels = Vec::with_capacity(width * height * 4);
    for (((red, green), blue), alpha) in red.into_iter().zip(green).zip(blue).zip(alpha) {
        pixels.extend([red, green, blue, alpha]);
    }
    if let (Some(image), Some(mask)) = (
        Pixmap::from_vec(pixels, size),
        intersect(target, clip, path, Transform::from_scale(density, density)),
    ) {
        target.draw_pixmap(
            left,
            top,
            image.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            Some(&mask),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn css_gradients_interpolate_translucent_colors_in_premultiplied_space() {
        let mut renderer = Renderer::default();
        let mut surface = Pixmap::new(101, 4).unwrap();
        renderer.paint(
            &json!({
                "background": "#000000",
                "commands": [{
                    "kind": "linear_gradient", "x":0, "y":0, "w":101, "h":4,
                    "from":[0,0], "to":[1,0],
                    "stops":[[0,[255,0,0,0.0]],[1,[0,0,255,1.0]]]
                }]
            }),
            &mut surface.as_mut(),
            1.0,
            |_| None,
        );
        let pixel = surface.pixel(50, 2).unwrap();
        assert_eq!(pixel.red(), 0);
        assert!((127..=129).contains(&pixel.blue()));
    }

    #[test]
    fn rust_raster_clips_original_style_scene() {
        let mut renderer = Renderer::default();
        let mut canvas = Pixmap::new(120, 80).unwrap();
        renderer.paint(&json!({
            "background":"#342048",
            "commands":[
                {"kind":"clip","x":0,"y":0,"w":30,"h":40},
                {"kind":"rect","x":0,"y":0,"w":60,"h":40,"color":"#ff0055"},
                {"kind":"restore"},
                {"kind":"text","x":47,"y":32,"text":"Ә","size":22.0,"color":"#ffffff","bold":true}
            ]
        }), &mut canvas.as_mut(), 2.0, |_| None);
        assert_eq!(canvas.pixel(3, 3).unwrap().red(), 255);
        assert_eq!(canvas.pixel(61, 3).unwrap().red(), 52);
        assert!(measure_text("Хардкор", 16.0, true, 0.0) > 20.0);
    }

    #[test]
    fn shadows_stay_outside_the_translucent_box_and_filter_softens_its_edge() {
        let mut renderer = Renderer::default();
        let mut canvas = Pixmap::new(64, 64).unwrap();
        renderer.paint(
            &json!({
                "background":"#ffffff",
                "commands":[
                    {"kind":"rect","x":20,"y":10,"w":20,"h":20,"color":"#80ffffff",
                     "shadows":[{"dx":4,"dy":1,"blur":4,"color":"#000000"}]},
                    {"kind":"rect","x":10,"y":42,"w":16,"h":16,"color":"#000000","blur":2}
                ]
            }),
            &mut canvas.as_mut(),
            1.0,
            |_| None,
        );
        assert_eq!(canvas.pixel(30, 20).unwrap().red(), 255);
        assert!(canvas.pixel(41, 20).unwrap().red() < 200);
        assert!((1..250).contains(&canvas.pixel(9, 49).unwrap().red()));
        assert!(canvas.pixel(17, 49).unwrap().red() < 15);
    }

    #[test]
    fn coin_sampling_averages_fractional_premultiplied_coverage() {
        let source = Pixmap::from_vec(
            vec![240, 150, 60, 255, 0, 0, 0, 0, 60, 45, 0, 60],
            IntSize::from_wh(3, 1).unwrap(),
        )
        .unwrap();
        let sample = area_resample(&source, (2, 1)).unwrap();
        assert_eq!(sample.data(), &[160, 100, 40, 170, 40, 30, 0, 40]);
    }

    #[test]
    fn coin_drop_shadow_respects_its_css_clip_circle() {
        let mut renderer = Renderer::default();
        let mut canvas = Pixmap::new(40, 40).unwrap();
        let coin = include_bytes!("../../../public/assets/coin.png");
        renderer.paint(
            &json!({
                "background":"#ffffff",
                "commands":[crate::theme::coin(10.0,10.0,20.0)]
            }),
            &mut canvas.as_mut(),
            1.0,
            |_| Some(coin.to_vec()),
        );
        assert!(renderer.coin_samples.contains_key(&(20, 20)));
        assert_eq!(canvas.pixel(11, 11).unwrap().red(), 255);
        assert_ne!(canvas.pixel(20, 20).unwrap().blue(), 255);
    }
}
