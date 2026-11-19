use super::Point;
use crate::theme::{Bezier, Color, Shadow};
use serde_json::{Value, json};

pub(super) const SCREEN_IN: f32 = 262.5;
pub(super) const SCREEN_OUT: f32 = 180.0;
pub(super) const DIALOG_IN: f32 = 210.0;
pub(super) const DIALOG_OUT: f32 = 158.0;
pub(super) const DIALOG_BACKDROP: f32 = 187.5;
pub(super) const TILE_SHUFFLE: f32 = 360.0;
pub(super) const TILE_LOCKOUT_MS: i64 = 405;
pub(super) const GIFT_FLIGHT: f32 = 638.0;
pub(super) const GIFT_BUMP: f32 = 225.0;
pub(super) const WORD_FOUND: f32 = 675.0;
pub(super) const WORD_REPEAT: f32 = 375.0;
pub(super) const HINT_REVEAL: f32 = 187.5;
pub(super) const INVALID_CELL: f32 = 360.0;
pub(super) const BUTTON_TRANSITION: f32 = 135.0;

pub(super) const EASE: Bezier = Bezier(0.25, 0.1, 0.25, 1.0);
pub(super) const EASE_OUT: Bezier = Bezier::EASE_OUT;
pub(super) const SHUFFLE_CURVE: Bezier = Bezier::SHUFFLE;

pub(super) fn elapsed(now: i64, start: i64) -> f32 {
    now.saturating_sub(start).max(0) as f32
}
pub(super) fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub(super) fn press_scale(elapsed: f32) -> f32 {
    mix(1.0, 0.96, EASE.sample(elapsed / BUTTON_TRANSITION))
}
pub(super) fn point(a: Point, b: Point, t: f32) -> Point {
    Point::new(mix(a.x, b.x, t), mix(a.y, b.y, t))
}

#[derive(Clone, Copy)]
pub(super) struct Transform {
    pub scale: f32,
    pub rotate: f32,
    pub translation: Point,
    pub alpha: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            scale: 1.0,
            rotate: 0.0,
            translation: Point::default(),
            alpha: 1.0,
        }
    }
}

impl Transform {
    pub fn command(self, origin: Point) -> Value {
        json!({"kind":"transform","originX":origin.x,"originY":origin.y,
               "scale":self.scale,"rotate":self.rotate,"translateX":self.translation.x,
               "translateY":self.translation.y,"alpha":self.alpha})
    }
}

pub(super) fn enter(elapsed: f32, duration: f32) -> Transform {
    let p = Bezier::WINDOW.sample(elapsed / duration);
    Transform {
        scale: mix(1.12, 1.0, p),
        alpha: p,
        ..Transform::default()
    }
}

pub(super) fn exit(elapsed: f32, duration: f32) -> Transform {
    let p = Bezier::EASE_IN.sample(elapsed / duration);
    Transform {
        scale: mix(1.0, 0.94, p),
        alpha: 1.0 - p,
        ..Transform::default()
    }
}

pub(super) fn gift(elapsed: f32, origin: Point, target: Point) -> Transform {
    let p = Bezier::GIFT.sample(elapsed / GIFT_FLIGHT);
    let travel = Point::new(target.x - origin.x, target.y - origin.y);
    let peak = Point::new(travel.x * 0.2, travel.y * 0.35 - 45.0);
    let (translation, scale, rotate) = if p <= 0.35 {
        let t = p / 0.35;
        (
            point(Point::default(), peak, t),
            mix(1.0, 1.6, t),
            mix(0.0, -15.0, t),
        )
    } else {
        let t = (p - 0.35) / 0.65;
        (
            point(peak, travel, t),
            mix(1.6, 0.7, t),
            mix(-15.0, 12.0, t),
        )
    };
    Transform {
        translation,
        scale,
        rotate,
        ..Transform::default()
    }
}

pub(super) fn gift_bump(elapsed: f32) -> f32 {
    let p = (elapsed / GIFT_BUMP).clamp(0.0, 1.0);
    if p <= 0.5 {
        mix(1.0, 1.18, p * 2.0)
    } else {
        mix(1.18, 1.0, (p - 0.5) * 2.0)
    }
}

fn interpolate_frames(elapsed: f32, duration: f32, frames: &[(f32, f32, f32)]) -> Transform {
    let p = (elapsed / duration).clamp(0.0, 1.0);
    let [a, b] = frames
        .windows(2)
        .find(|pair| pair[1].0 >= p)
        .unwrap_or(&frames[frames.len() - 2..])
    else {
        unreachable!()
    };
    let t = Bezier::EASE_IN_OUT.sample((p - a.0) / (b.0 - a.0));
    Transform {
        scale: mix(a.1, b.1, t),
        rotate: mix(a.2, b.2, t),
        ..Transform::default()
    }
}

pub(super) fn word_found(elapsed: f32) -> Transform {
    interpolate_frames(
        elapsed,
        WORD_FOUND,
        &[
            (0.0, 1.0, 0.0),
            (0.18, 1.18, -5.0),
            (0.34, 1.18, 5.0),
            (0.50, 1.16, -4.0),
            (0.66, 1.12, 3.0),
            (0.82, 1.08, -1.0),
            (1.0, 1.0, 0.0),
        ],
    )
}

pub(super) fn word_shadows(elapsed: f32) -> [Shadow; 2] {
    let p = (elapsed / WORD_FOUND).clamp(0.0, 1.0);
    let (a, b, t) = if p <= 0.18 {
        (0, 1, Bezier::EASE_IN_OUT.sample(p / 0.18))
    } else {
        (1, 2, Bezier::EASE_IN_OUT.sample((p - 0.18) / 0.82))
    };
    let rings = [
        (2.0, [255, 223, 145, 255]),
        (3.0, [255, 233, 172, 255]),
        (2.0, [255, 224, 154, 255]),
    ];
    let glow = [
        (8.0, 0.0, [255, 208, 100, 255]),
        (18.0, 4.0, [255, 195, 79, 187]),
        (10.0, 0.0, [255, 197, 87, 128]),
    ];
    let color = |a: [u8; 4], b: [u8; 4]| {
        let [r, g, b, a] = std::array::from_fn(|i| mix(a[i] as f32, b[i] as f32, t).round() as u8);
        Color::rgba(r, g, b, a)
    };
    [
        Shadow::new(
            0.0,
            0.0,
            0.0,
            mix(rings[a].0, rings[b].0, t),
            color(rings[a].1, rings[b].1),
        ),
        Shadow::new(
            0.0,
            0.0,
            mix(glow[a].0, glow[b].0, t),
            mix(glow[a].1, glow[b].1, t),
            color(glow[a].2, glow[b].2),
        ),
    ]
}

pub(super) fn word_repeat(elapsed: f32) -> Transform {
    interpolate_frames(
        elapsed,
        WORD_REPEAT,
        &[
            (0.0, 1.0, 0.0),
            (0.2, 1.0, -8.0),
            (0.4, 1.0, 8.0),
            (0.6, 1.0, -6.0),
            (0.8, 1.0, 4.0),
            (1.0, 1.0, 0.0),
        ],
    )
}

pub(super) fn invalid(elapsed: f32) -> Transform {
    interpolate_frames(
        elapsed,
        INVALID_CELL,
        &[
            (0.0, 1.0, 0.0),
            (0.2, 1.0, -9.0),
            (0.4, 1.0, 9.0),
            (0.6, 1.0, -6.0),
            (0.8, 1.0, 4.0),
            (1.0, 1.0, 0.0),
        ],
    )
}

pub(super) fn hint(elapsed: f32) -> Transform {
    let p = EASE_OUT.sample(elapsed / HINT_REVEAL);
    Transform {
        scale: mix(0.85, 1.0, p),
        alpha: mix(0.5, 1.0, p),
        ..Transform::default()
    }
}

#[derive(Default)]
pub(super) struct Tilt {
    neutral: Option<Point>,
    last: Point,
    settled: bool,
    moved_at: i64,
    last_sample: i64,
    target: Point,
    offset: Point,
    last_frame: Option<i64>,
}

impl Tilt {
    fn delta(value: f32, base: f32) -> f32 {
        (value - base + 540.0).rem_euclid(360.0) - 180.0
    }

    pub fn sample(&mut self, sample: Point, angle: f32, now: i64) {
        let Some(neutral) = &mut self.neutral else {
            self.neutral = Some(sample);
            self.last = sample;
            self.last_sample = now;
            self.moved_at = now;
            return;
        };
        let dt = elapsed(now, self.last_sample).min(100.0);
        self.last_sample = now;
        if Self::delta(sample.x, self.last.x).abs() + Self::delta(sample.y, self.last.y).abs() > 0.5
        {
            self.moved_at = now;
        }
        self.last = sample;
        let x = Self::delta(sample.x, neutral.x);
        let y = Self::delta(sample.y, neutral.y);
        if x.abs() > 22.0 {
            neutral.x += x - x.signum() * 22.0;
            self.settled = true;
        }
        if y.abs() > 22.0 {
            neutral.y += y - y.signum() * 22.0;
            self.settled = true;
        }
        if self.settled && now.saturating_sub(self.moved_at) > 500 {
            let mix = 1.0 - (-dt / 600.0).exp();
            neutral.x += Self::delta(sample.x, neutral.x) * mix;
            neutral.y += Self::delta(sample.y, neutral.y) * mix;
            if Self::delta(sample.x, neutral.x).abs() + Self::delta(sample.y, neutral.y).abs()
                < 0.05
            {
                self.settled = false;
            }
        }
        let x = (Self::delta(sample.x, neutral.x) / 22.0).clamp(-1.0, 1.0);
        let y = (Self::delta(sample.y, neutral.y) / 22.0).clamp(-1.0, 1.0);
        let (sin, cos) = angle.to_radians().sin_cos();
        self.target = Point::new(
            -(x * cos + y * sin).clamp(-1.0, 1.0) * 18.0,
            -(y * cos - x * sin).clamp(-1.0, 1.0) * 18.0,
        );
    }

    pub fn tick(&mut self, now: i64) {
        if self.moving() {
            let dt = self
                .last_frame
                .map_or(16.0, |elapsed_at| elapsed(now, elapsed_at))
                .min(50.0);
            self.last_frame = Some(now);
            let mix = 1.0 - (-dt / 95.0).exp();
            self.offset = point(self.offset, self.target, mix);
        } else {
            self.last_frame = None;
        }
    }

    pub fn moving(&self) -> bool {
        (self.offset.x - self.target.x).abs() + (self.offset.y - self.target.y).abs() > 0.03
    }

    pub fn offset(&self) -> Point {
        let fixed = |value: f32| (value * 100.0).round() / 100.0;
        Point::new(fixed(self.offset.x), fixed(self.offset.y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_keyframes() {
        let first = word_found(0.);
        assert_eq!(first.scale, 1.);
        let top = word_found(WORD_FOUND * 0.18);
        assert!((top.scale - 1.18).abs() < 1e-5 && (top.rotate + 5.).abs() < 1e-5);
        let end = word_found(WORD_FOUND);
        assert_eq!(end.scale, 1.);
        assert_eq!(end.rotate, 0.);
        let end = gift(GIFT_FLIGHT, Point::new(300., 400.), Point::new(280., 45.));
        assert_eq!((end.translation.x, end.translation.y), (-20., -355.));
        assert_eq!(end.scale, 0.7);
    }
}
