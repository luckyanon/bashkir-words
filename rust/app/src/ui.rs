use std::collections::HashSet;
use std::f32::consts::TAU;

use serde_json::{Value, json};

use crate::game::{ActionResult, Game, Mode};
use crate::theme::{self, Align, BoxStyle, Color, TextStyle};

#[path = "ui/animation.rs"]
mod animation;
#[path = "ui/layout.rs"]
mod layout;

const RESET_DESCRIPTION: &str = "Игра начнётся с первого уровня и 90 монет. Найденные слова, полученные награды и открытые фоны будут сброшены. Отменить сброс нельзя.";
const GALLERY_HEADING_SIZE: f32 = 17.0;
const GALLERY_HEADING_MARGIN: f32 = 12.0;
const GALLERY_HEIGHT: f32 = 181.0;
const GALLERY_SCROLLBAR: f32 = 5.0;
const POS_HEIGHT: f32 = 16.0;
const POS_MARGIN: f32 = 12.0;

struct Background {
    id: &'static str,
    title: &'static str,
    price: u64,
}

const BACKGROUNDS: [Background; 5] = [
    Background {
        id: "ufa",
        title: "Уфа",
        price: 0,
    },
    Background {
        id: "salavat",
        title: "Салават",
        price: 1000,
    },
    Background {
        id: "baymak",
        title: "Баймак",
        price: 1600,
    },
    Background {
        id: "village",
        title: "Деревня",
        price: 2400,
    },
    Background {
        id: "olasay",
        title: "У бабушки",
        price: 3000,
    },
];

#[derive(Clone, Copy, Debug, Default)]
struct Point {
    x: f32,
    y: f32,
}

impl Point {
    fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    fn distance(self, other: Self) -> f32 {
        (self.x - other.x).hypot(self.y - other.y)
    }
    fn midpoint(self, other: Self) -> Self {
        Self::new((self.x + other.x) * 0.5, (self.y + other.y) * 0.5)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Rect {
    fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    fn right(self) -> f32 {
        self.x + self.w
    }
    fn bottom(self) -> f32 {
        self.y + self.h
    }
    fn center(self) -> Point {
        Point::new(self.x + 0.5 * self.w, self.y + 0.5 * self.h)
    }
    fn inset(self, pad: f32) -> Self {
        Self::new(
            self.x + pad,
            self.y + pad,
            (self.w - 2.0 * pad).max(0.0),
            (self.h - 2.0 * pad).max(0.0),
        )
    }
    fn contains(self, p: Point) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.bottom()
    }
    fn intersection(self, other: Self) -> Option<Self> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let w = self.right().min(other.right()) - x;
        let h = self.bottom().min(other.bottom()) - y;
        (w > 0.0 && h > 0.0).then(|| Self::new(x, y, w, h))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Screen {
    #[default]
    Menu,
    Play,
    Victory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Setting {
    Sound,
    Music,
    Animations,
    Tilt,
}

#[derive(Clone, Debug)]
enum Panel {
    Found,
    Translation { word: String, from_found: bool },
    Settings,
    About,
    Attribution,
    Rating,
    Feedback,
    Reset,
    Regenerate,
    Purchase(usize),
}

impl Panel {
    fn title(&self) -> &'static str {
        match self {
            Self::Found => "Найденные слова",
            Self::Translation { .. } => "Перевод и значения",
            Self::Settings => "Настройки",
            Self::About => "Об игре",
            Self::Attribution => "Источники и лицензия",
            Self::Rating => "Рейтинг",
            Self::Feedback => "Обратная связь",
            Self::Reset => "Сбросить прогресс?",
            Self::Regenerate => "Пересобрать уровень?",
            Self::Purchase(_) => "Купить фон?",
        }
    }
}

#[derive(Clone, Debug)]
enum Action {
    Navigate(Screen),
    Mode(Mode),
    Panel(Panel),
    Close,
    Claim,
    Shuffle,
    Hint,
    Next,
    Regenerate,
    Reset,
    Background(usize),
    Buy(usize),
    Toggle(Setting),
    ToggleSources,
    Select(usize),
    Submit,
    EditFeedback,
    SubmitFeedback,
    OpenLink(String),
    Nothing,
}

fn button_action(action: &Action) -> bool {
    !matches!(
        action,
        Action::OpenLink(_) | Action::EditFeedback | Action::ToggleSources | Action::Nothing
    )
}

struct Control {
    rect: Rect,
    drawn_rect: Rect,
    drawn_end: usize,
    label: String,
    action: Action,
}

#[derive(Clone, Copy)]
struct ScrollArea {
    bounds: Rect,
    max: f32,
    kind: ScrollKind,
}

#[derive(Clone, Copy)]
enum ScrollKind {
    Main,
    Gallery,
    Meanings,
}

#[derive(Default)]
struct Paint {
    commands: Vec<Value>,
    controls: Vec<Control>,
    semantics: Vec<Value>,
    clips: Vec<Rect>,
    scroll: Option<ScrollArea>,
    inner_scroll: Option<ScrollArea>,
}

impl Paint {
    fn styled(&mut self, r: Rect, style: BoxStyle<'_>) {
        self.commands.push(style.emit(r.x, r.y, r.w, r.h));
    }

    fn line_text(&mut self, x: f32, top: f32, line: f32, label: &str, style: TextStyle<'_>) {
        let baseline = top + theme::baseline_offset(style.size, style.bold, line);
        self.commands.push(theme::text(x, baseline, label, style));
    }

    fn styled_center(&mut self, r: Rect, label: &str, style: TextStyle<'_>) {
        self.line_text(r.center().x, r.y, r.h, label, style.align(Align::Center));
    }

    fn glyph(&mut self, center: Point, icon: theme::Icon, size: f32, color: Color) {
        self.commands.push(theme::icon(
            center.x - size * 0.5,
            center.y - size * 0.5,
            size,
            icon,
            color,
        ));
    }

    fn coin(&mut self, x: f32, y: f32, size: f32) {
        self.commands.push(theme::coin(x, y, size));
    }

    fn transform(&mut self, origin: Point, effect: animation::Transform) {
        self.commands.push(effect.command(origin));
    }

    fn restore(&mut self) {
        self.commands.push(json!({"kind":"restore"}));
    }

    fn rect(&mut self, r: Rect, radius: f32, color: &str) {
        self.commands.push(
            json!({"kind":"rect","x":r.x,"y":r.y,"w":r.w,"h":r.h,"radius":radius,"color":color}),
        );
    }

    fn line(&mut self, a: Point, b: Point, stroke: f32, color: &str) {
        self.commands.push(json!({"kind":"line","x1":a.x,"y1":a.y,"x2":b.x,"y2":b.y,"stroke":stroke,"color":color}));
    }

    fn clip(&mut self, r: Rect) {
        self.commands
            .push(json!({"kind":"clip","x":r.x,"y":r.y,"w":r.w,"h":r.h}));
        self.clips.push(r);
    }

    fn unclip(&mut self) {
        self.commands.push(json!({"kind":"restore"}));
        self.clips.pop();
    }

    fn visible_rect(&self, mut rect: Rect) -> Option<Rect> {
        for clip in &self.clips {
            rect = rect.intersection(*clip)?;
        }
        Some(rect)
    }

    fn control(&mut self, rect: Rect, label: &str, action: Action) {
        if let Some(visible) = self.visible_rect(rect) {
            self.controls.push(Control {
                rect: visible,
                drawn_rect: rect,
                drawn_end: self.commands.len(),
                label: label.to_string(),
                action,
            });
        }
    }

    fn scale_control(&mut self, active: Rect, scale: f32) {
        if (scale - 1.0).abs() < f32::EPSILON {
            return;
        }
        let Some(control) = self
            .controls
            .iter()
            .rev()
            .find(|control| control.rect == active)
        else {
            return;
        };
        let end = control.drawn_end;
        let start = (0..end)
            .rev()
            .take_while(|&index| graphic_inside(&self.commands[index], control.drawn_rect))
            .last();
        if let Some(start) = start {
            self.commands.insert(end, json!({"kind":"restore"}));
            self.commands.insert(
                start,
                animation::Transform {
                    scale,
                    ..animation::Transform::default()
                }
                .command(control.drawn_rect.center()),
            );
        }
    }

    fn semantic(&mut self, id: usize, rect: Rect, label: &str) {
        if let Some(rect) = self.visible_rect(rect) {
            self.semantics
                .push(json!({"id":10_000+id,"label":label,"x":rect.x,"y":rect.y,
                "w":rect.w,"h":rect.h,"readOnly":true}));
        }
    }
}

fn graphic_inside(command: &Value, bounds: Rect) -> bool {
    let value = |key: &str| command[key].as_f64().unwrap_or_default() as f32;
    let contains =
        |x, y, w, h| bounds.contains(Point::new(x, y)) && bounds.contains(Point::new(x + w, y + h));
    let (x, y) = (value("x"), value("y"));
    match command["kind"].as_str() {
        Some("rect" | "stroke_rect" | "image" | "path") => contains(x, y, value("w"), value("h")),
        Some("circle") => {
            let r = value("r");
            contains(x - r, y - r, 2.0 * r, 2.0 * r)
        }
        Some("line") => {
            bounds.contains(Point::new(value("x1"), value("y1")))
                && bounds.contains(Point::new(value("x2"), value("y2")))
        }
        Some("text") => {
            let size = value("size");
            let width = crate::raster::measure_text(
                command["text"].as_str().unwrap_or(""),
                size,
                command["bold"].as_bool().unwrap_or(false),
                value("letterSpacing"),
            );
            let left = match command["align"].as_str() {
                Some("center") => x - width * 0.5,
                Some("right") => x - width,
                _ => x,
            };
            contains(left, y - size * 0.8, width, size)
        }
        _ => false,
    }
}

struct Geometry {
    board: Rect,
    viewport: Rect,
    hardcore_focus: Rect,
    header: Rect,
    wheel: Rect,
    found: Rect,
    preview: Rect,
    menu_size: f32,
    tile_max: f32,
}

impl Geometry {
    fn new(w: f32, h: f32) -> Self {
        layout::game(w, h)
    }

    fn tiles(&self, n: usize) -> Vec<Point> {
        let center = self.wheel.center();
        let radius = self.wheel.w * 0.36;
        (0..n)
            .map(|i| {
                let angle = (i as f32 / n as f32 - 0.25) * TAU;
                Point::new(
                    center.x + radius * angle.cos(),
                    center.y + radius * angle.sin(),
                )
            })
            .collect()
    }

    fn tile_radius(&self, n: usize) -> f32 {
        (self.wheel.w.floor() * 0.68 * (std::f32::consts::PI / n.max(2) as f32).sin())
            .floor()
            .min(self.tile_max)
            * 0.5
    }
}

#[derive(Clone, Copy)]
struct Camera {
    zoom: f32,
    offset: Point,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            offset: Point::default(),
        }
    }
}

impl Camera {
    fn cell(self, game: &Game, board: Rect) -> f32 {
        if game.is_hardcore() {
            36.0 * self.zoom
        } else {
            let cols = game.level().cols.max(1) as f32;
            let rows = game.level().rows.max(1) as f32;
            ((board.w.round() - 4.0 - (cols - 1.0) * 3.0) / cols)
                .floor()
                .min(((board.h.round() - 16.0 - (rows - 1.0) * 3.0) / rows).floor())
                .clamp(12.0, 44.0)
        }
    }

    fn gap(self, game: &Game) -> f32 {
        if game.is_hardcore() {
            3.0 * self.zoom
        } else {
            3.0
        }
    }

    fn size(self, game: &Game, board: Rect) -> Point {
        let gap = self.gap(game);
        let step = self.cell(game, board) + gap;
        Point::new(
            step * game.level().cols as f32 - gap,
            step * game.level().rows as f32 - gap,
        )
    }

    fn fit(game: &Game, board: Rect) -> f32 {
        ((board.w - 24.0) / (39.0 * game.level().cols.max(1) as f32 - 3.0))
            .min((board.h - 24.0) / (39.0 * game.level().rows.max(1) as f32 - 3.0))
            .min(1.0)
    }

    fn origin(self, game: &Game, board: Rect) -> Point {
        let size = self.size(game, board);
        let center = board.center();
        Point::new(
            center.x + self.offset.x - size.x * 0.5,
            if game.is_hardcore() {
                center.y + self.offset.y - size.y * 0.5
            } else {
                (center.y - size.y * 0.5).max(board.y + 8.0)
            },
        )
    }

    fn constrain(&mut self, game: &Game, board: Rect) {
        let size = self.size(game, board);
        let max_x = ((board.w + size.x) * 0.5 - 48.0).max(0.0);
        let max_y = ((board.h + size.y) * 0.5 - 48.0).max(0.0);
        self.offset.x = self.offset.x.clamp(-max_x, max_x);
        self.offset.y = self.offset.y.clamp(-max_y, max_y);
    }

    fn scale(&mut self, game: &Game, board: Rect, factor: f32, anchor: Point) {
        let new = (self.zoom * factor).clamp((Self::fit(game, board) * 0.7).max(0.03), 2.5);
        let center = board.center();
        let ratio = new / self.zoom;
        self.offset.x = anchor.x - center.x - (anchor.x - center.x - self.offset.x) * ratio;
        self.offset.y = anchor.y - center.y - (anchor.y - center.y - self.offset.y) * ratio;
        self.zoom = new;
        self.constrain(game, board);
    }

    fn pan(&mut self, game: &Game, board: Rect, x: f32, y: f32) {
        self.offset.x += x;
        self.offset.y += y;
        self.constrain(game, board);
    }
}

struct Pointer {
    id: i64,
    at: Point,
}

enum Gesture {
    Wheel {
        id: i64,
        point: Point,
    },
    Press {
        id: i64,
        start: Point,
        current: Point,
        rect: Rect,
        action: Action,
        scrolled: bool,
        scroll: Option<ScrollArea>,
    },
    Board(Vec<Pointer>),
}

struct TimedText {
    text: String,
    until: i64,
}

struct Idle {
    word: String,
    marks_tiles: bool,
    until: i64,
}

struct GiftFlight {
    began: i64,
    origin: Point,
}

struct Shuffling {
    began: i64,
    previous: Vec<usize>,
}

struct CellEffect {
    key: String,
    began: i64,
    found: bool,
}

struct Transition {
    began: i64,
    action: Action,
    screen: bool,
}

struct ReleasedPress {
    rect: Rect,
    scale: f32,
    began: i64,
}

pub struct Ui {
    width: f32,
    height: f32,
    density: f32,
    screen: Screen,
    panel: Option<Panel>,
    scroll: f32,
    found_scroll: f32,
    victory_scroll: f32,
    menu_scroll: f32,
    gallery_scroll: f32,
    meanings_scroll: f32,
    attribution_parent_scroll: f32,
    sources_open: bool,
    feedback_text: String,
    camera: Camera,
    gesture: Option<Gesture>,
    press_began: i64,
    released_press: Option<ReleasedPress>,
    selection: Vec<usize>,
    tilt: animation::Tilt,
    effect: Value,
    toast: Option<TimedText>,
    toast_began: i64,
    repeat: Option<TimedText>,
    invalid: Option<TimedText>,
    cell_effects: Vec<CellEffect>,
    shuffle: Option<Shuffling>,
    gift_flight: Option<GiftFlight>,
    screen_enter: i64,
    panel_enter: i64,
    leaving: Option<Transition>,
    victory_after: Option<(i64, u64)>,
    reward: u64,
    started: i64,
    idle: Option<Idle>,
    idle_letters_next: bool,
    paused: bool,
    reduced_motion: bool,
    next_midnight: Option<i64>,
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            width: 390.0,
            height: 760.0,
            density: 2.0,
            screen: Screen::Menu,
            panel: None,
            scroll: 0.0,
            found_scroll: 0.0,
            victory_scroll: 0.0,
            camera: Camera::default(),
            menu_scroll: 0.0,
            gallery_scroll: 0.0,
            meanings_scroll: 0.0,
            attribution_parent_scroll: 0.0,
            sources_open: false,
            feedback_text: String::new(),
            gesture: None,
            press_began: 0,
            released_press: None,
            selection: Vec::new(),
            tilt: animation::Tilt::default(),
            effect: json!({}),
            toast: None,
            toast_began: 0,
            repeat: None,
            invalid: None,
            cell_effects: Vec::new(),
            shuffle: None,
            gift_flight: None,
            screen_enter: 0,
            panel_enter: 0,
            leaving: None,
            victory_after: None,
            reward: 0,
            started: 0,
            idle: None,
            idle_letters_next: false,
            paused: false,
            reduced_motion: false,
            next_midnight: None,
        }
    }
}

impl Ui {
    pub fn frame(&self, game: &Game) -> Value {
        theme::with_font_density(self.density, || self.frame_inner(game))
    }

    fn frame_inner(&self, game: &Game) -> Value {
        let mut paint = self.paint(game);
        let now = game.now_millis;
        let motion = self.motion(game);
        let mut press_transition = false;
        if let Some(Gesture::Press {
            rect,
            current,
            scrolled: false,
            action,
            ..
        }) = &self.gesture
        {
            if rect.contains(*current) && button_action(action) {
                let elapsed = animation::elapsed(now, self.press_began);
                let scale = if motion {
                    animation::press_scale(elapsed)
                } else {
                    0.96
                };
                paint.scale_control(*rect, scale);
                press_transition = elapsed < animation::BUTTON_TRANSITION;
            }
        } else if let Some(released) = &self.released_press {
            let elapsed = animation::elapsed(now, released.began);
            if motion && elapsed < animation::BUTTON_TRANSITION {
                let progress = animation::EASE.sample(elapsed / animation::BUTTON_TRANSITION);
                paint.scale_control(released.rect, animation::mix(released.scale, 1.0, progress));
                press_transition = true;
            }
        }
        let controls: Vec<Value> = paint.controls.into_iter().enumerate().map(|(id, item)| {
            json!({"id":id,"label":item.label,"x":item.rect.x,"y":item.rect.y,"w":item.rect.w,"h":item.rect.h})
        }).chain(paint.semantics).collect();
        let toast_transition = self.toast.as_ref().is_some_and(|toast| {
            animation::elapsed(now, self.toast_began) < 200.0 || now >= toast.until
        });
        let animating = motion
            && (self.gift_flight.is_some()
                || press_transition
                || self.shuffle.is_some()
                || self.leaving.is_some()
                || game.state.settings.background_motion && !self.paused && self.tilt.moving()
                || toast_transition
                || animation::elapsed(now, self.screen_enter) < animation::SCREEN_IN
                || self.panel.is_some()
                    && animation::elapsed(now, self.panel_enter) < animation::DIALOG_IN
                || !self.cell_effects.is_empty()
                || self.repeat.is_some()
                || self.invalid.is_some());
        json!({"commands":paint.commands,"controls":controls,"animate":animating})
    }

    fn geometry(&self) -> Geometry {
        Geometry::new(self.width, self.height)
    }

    fn motion(&self, game: &Game) -> bool {
        game.state.settings.animations && !self.reduced_motion
    }

    fn outgoing_screen(&self, game: &Game) -> Option<f32> {
        self.leaving
            .as_ref()
            .filter(|transition| transition.screen)
            .map(|transition| animation::elapsed(game.now_millis, transition.began))
    }

    fn paint(&self, game: &Game) -> Paint {
        let mut paint = Paint::default();
        let name = game
            .state
            .settings
            .background
            .as_deref()
            .filter(|name| BACKGROUNDS.iter().any(|background| background.id == *name))
            .unwrap_or("ufa");
        let image = format!("assets/background-{name}.webp");
        let tilt = if game.state.settings.background_motion && self.motion(game) {
            self.tilt.offset()
        } else {
            Point::default()
        };
        paint.commands.extend(theme::wallpaper(
            self.width,
            self.height,
            &image,
            tilt.x,
            tilt.y,
        ));
        let elapsed = animation::elapsed(game.now_millis, self.screen_enter);
        let outgoing = self.outgoing_screen(game);
        let whole_screen = self.screen != Screen::Play || !game.is_hardcore();
        let screen_transform = if self.motion(game) && whole_screen {
            if let Some(outgoing) = outgoing {
                Some(animation::exit(outgoing, animation::SCREEN_OUT))
            } else if elapsed < animation::SCREEN_IN {
                Some(animation::enter(elapsed, animation::SCREEN_IN))
            } else {
                None
            }
        } else {
            None
        };
        if let Some(transform) = screen_transform {
            let center = if self.screen == Screen::Menu {
                layout::menu(self.width, self.height, self.menu_scroll)
                    .stage
                    .center()
            } else {
                Point::new(self.width * 0.5, self.height * 0.5)
            };
            paint.transform(center, transform);
        }
        match self.screen {
            Screen::Menu => self.paint_menu(game, &mut paint),
            Screen::Play => self.paint_game(game, &mut paint),
            Screen::Victory => self.paint_victory(game, &mut paint),
        }
        if screen_transform.is_some() {
            paint.restore();
        }
        if let Some(panel) = &self.panel {
            if matches!(panel, Panel::Attribution) {
                self.paint_panel(
                    game,
                    &mut paint,
                    &Panel::About,
                    Some(self.attribution_parent_scroll),
                );
            }
            paint.controls.clear();
            paint.semantics.clear();
            paint.scroll = None;
            paint.inner_scroll = None;
            self.paint_panel(game, &mut paint, panel, None);
        }
        if let Some(flight) = &self.gift_flight {
            let elapsed = animation::elapsed(game.now_millis, flight.began);
            if elapsed < animation::GIFT_FLIGHT && self.screen == Screen::Menu {
                let menu = layout::menu(self.width, self.height, self.menu_scroll);
                let target = Self::balance_box(
                    menu.stage.right() - 24.0,
                    menu.stage.y + 20.0,
                    game.state.coins,
                    true,
                );
                let coin_target = Point::new(target.x + 28.5, target.center().y);
                paint.transform(
                    flight.origin,
                    animation::gift(elapsed, flight.origin, coin_target),
                );
                paint.coin(flight.origin.x - 16.0, flight.origin.y - 16.0, 32.0);
                paint.restore();
            }
        }
        if let Some(text) = &self.toast {
            let lines = theme::wrap(&text.text, self.width - 76.0, 15.0, true);
            let line_h = theme::line_height(15.0, true);
            let w = lines
                .iter()
                .map(|line| theme::text_width(line, 15.0, true))
                .fold(0.0, f32::max)
                + 44.0;
            let rect = Rect::new(
                (self.width - w) * 0.5,
                16.0,
                w,
                26.0 + line_h * lines.len() as f32,
            );
            let fade = if self.motion(game) {
                if game.now_millis >= text.until {
                    1.0 - animation::EASE
                        .sample(animation::elapsed(game.now_millis, text.until) / 200.0)
                } else {
                    animation::EASE
                        .sample(animation::elapsed(game.now_millis, self.toast_began) / 200.0)
                }
            } else {
                1.0
            };
            if fade < 1.0 {
                paint.transform(
                    rect.center(),
                    animation::Transform {
                        alpha: fade,
                        translation: Point::new(0.0, -20.0 * (1.0 - fade)),
                        ..animation::Transform::default()
                    },
                );
            }
            paint.styled(rect, theme::TOAST);
            for (i, line) in lines.iter().enumerate() {
                paint.line_text(
                    rect.center().x,
                    rect.y + 13.0 + i as f32 * line_h,
                    line_h,
                    line,
                    TextStyle::new(15.0, Color::hex(0x633c75))
                        .bold()
                        .align(Align::Center),
                );
            }
            if fade < 1.0 {
                paint.restore();
            }
        }
        paint
    }

    fn balance_box(x_right: f32, y: f32, coins: u64, menu: bool) -> Rect {
        let (padding, gap, font) = if menu {
            (16.0, 7.0, 16.0)
        } else {
            (11.0, 8.0, 14.0)
        };
        let w = 25.0 + gap + theme::text_width(&coins.to_string(), font, true) + padding * 2.0;
        Rect::new(x_right - w, y, w, 47.0)
    }

    fn balance(&self, paint: &mut Paint, bounds: Rect, coins: u64, menu: bool) {
        let (padding, gap, font) = if menu {
            (16.0, 7.0, 16.0)
        } else {
            (11.0, 8.0, 14.0)
        };
        paint.styled(bounds, theme::COIN_BALANCE);
        paint.coin(bounds.x + padding, bounds.y + 11.0, 25.0);
        paint.line_text(
            bounds.x + padding + 25.0 + gap,
            bounds.y,
            bounds.h,
            &coins.to_string(),
            TextStyle::new(font, theme::WHITE).bold(),
        );
        paint.semantic(0, bounds, &format!("{coins} монет"));
    }

    fn paint_menu(&self, game: &Game, paint: &mut Paint) {
        let menu = layout::menu(self.width, self.height, self.menu_scroll);
        let center = menu.stage.center().x;
        let view = Rect::new(0.0, 0.0, self.width, self.height);
        paint.scroll = Some(ScrollArea {
            bounds: view,
            max: menu.stage.h - self.height,
            kind: ScrollKind::Main,
        });
        paint.clip(view);
        let balance = Self::balance_box(
            menu.stage.right() - 24.0,
            menu.stage.y + 20.0,
            game.state.coins,
            true,
        );
        let bump = self.gift_flight.as_ref().map_or(1.0, |f| {
            let elapsed = animation::elapsed(game.now_millis, f.began) - animation::GIFT_FLIGHT;
            animation::gift_bump(elapsed)
        });
        if bump != 1.0 {
            paint.transform(
                balance.center(),
                animation::Transform {
                    scale: bump,
                    ..animation::Transform::default()
                },
            );
        }
        self.balance(paint, balance, game.state.coins, true);
        if bump != 1.0 {
            paint.restore();
        }
        let rating = Rect::new(
            menu.stage.x + 25.0,
            menu.stage.y + 24.0,
            theme::text_width("Рейтинг", 15.0, true) + 30.0,
            44.0,
        );
        paint.styled(rating, theme::MENU_RATING);
        paint.styled_center(rating, "Рейтинг", TextStyle::new(15.0, theme::WHITE).bold());
        paint.control(rating, "Рейтинг", Action::Panel(Panel::Rating));
        paint.line_text(
            center,
            menu.eyebrow_y,
            theme::line_height(13.0, false),
            "БАШҠОРТСА",
            TextStyle::new(13.0, theme::EYEBROW)
                .align(Align::Center)
                .spacing(13.0 * 0.18),
        );
        if let Some(y) = menu.letters_y {
            let origin = Point::new(center, y + 28.0);
            paint.transform(
                origin,
                animation::Transform {
                    rotate: -5.0,
                    ..animation::Transform::default()
                },
            );
            for (i, letter) in ["Ә", "Ө", "Ү"].into_iter().enumerate() {
                let box_rect = Rect::new(center - 87.5 + (i as f32) * 61.0, y, 53.0, 56.0);
                if i == 1 {
                    paint.transform(
                        box_rect.center(),
                        animation::Transform {
                            rotate: 9.0,
                            translation: Point::new(0.0, -8.0),
                            ..animation::Transform::default()
                        },
                    );
                }
                let bg = if i == 1 {
                    Color::hex(0x7e4ba7)
                } else {
                    Color::hex(0xfff5ef)
                };
                paint.styled(
                    box_rect,
                    BoxStyle::new(bg, 12.0).shadows(&[theme::Shadow::new(
                        0.0,
                        5.0,
                        0.0,
                        0.0,
                        Color::css(0x32234025),
                    )]),
                );
                let color = if i == 1 {
                    theme::WHITE
                } else {
                    Color::hex(0x71439a)
                };
                paint.styled_center(box_rect, letter, TextStyle::new(32.0, color).bold());
                if i == 1 {
                    paint.restore();
                }
            }
            paint.restore();
        }
        paint.line_text(
            center,
            menu.title_y,
            menu.title_size,
            "Слова",
            TextStyle::new(menu.title_size, theme::WHITE)
                .bold()
                .align(Align::Center)
                .spacing(menu.title_size * -0.06)
                .shadows(&[theme::Shadow::new(
                    0.0,
                    4.0,
                    22.0,
                    0.0,
                    Color::css(0x36245133),
                )]),
        );
        let title_width =
            theme::text_width("Слова", menu.title_size, true) - 5.0 * menu.title_size * 0.06;
        paint.semantic(
            1,
            Rect::new(
                center - title_width * 0.5,
                menu.title_y,
                title_width,
                menu.title_size,
            ),
            "Слова",
        );
        for (i, line) in menu.subtitle_lines.iter().enumerate() {
            paint.line_text(
                center,
                menu.subtitle_y + (i as f32) * 27.0,
                27.0,
                line,
                TextStyle::new(18.0, theme::WHITE).align(Align::Center),
            );
        }
        let buttons = [
            ("Классика", Action::Mode(Mode::Classic)),
            ("Хардкор", Action::Mode(Mode::Hardcore)),
            ("Подарок", Action::Claim),
            ("Настройки", Action::Panel(Panel::Settings)),
            ("Об игре", Action::Panel(Panel::About)),
        ];
        for (i, (label, action)) in buttons.into_iter().enumerate() {
            let bounds = menu.actions[i];
            let box_style = if i == 0 {
                theme::PRIMARY
            } else {
                theme::MENU_SECONDARY
            };
            let color = if i == 0 { theme::DEEP } else { theme::WHITE };
            if matches!(action, Action::Claim) {
                let available = game
                    .state
                    .daily_gift
                    .last_claim_day
                    .is_none_or(|day| day < game.local_day);
                if !available {
                    paint.transform(
                        bounds.center(),
                        animation::Transform {
                            alpha: 0.65,
                            ..animation::Transform::default()
                        },
                    );
                }
                paint.styled(bounds, box_style);
                paint.styled_center(bounds, label, TextStyle::new(16.0, color).bold());
                let right = if self.width <= 360.0 { 13.0 } else { 17.0 };
                if available {
                    paint.coin(
                        bounds.right() - right - 25.0,
                        bounds.center().y - 12.5,
                        25.0,
                    );
                    paint.control(bounds, label, action);
                } else {
                    let remain = daily_remaining(game, self.next_midnight);
                    let size = if self.width <= 360.0 { 12.0 } else { 14.0 };
                    paint.line_text(
                        bounds.right() - right,
                        bounds.y,
                        bounds.h,
                        &remain,
                        TextStyle::new(size, Color::hex(0xffe1a2))
                            .bold()
                            .align(Align::Right),
                    );
                    paint.restore();
                }
            } else {
                paint.styled(bounds, box_style);
                paint.styled_center(bounds, label, TextStyle::new(16.0, color).bold());
                paint.control(bounds, label, action);
            }
        }
        for (i, line) in menu.note_lines.iter().enumerate() {
            paint.line_text(
                center,
                menu.note_y + (i as f32) * 19.5,
                19.5,
                line,
                TextStyle::new(13.0, theme::WHITE.opacity(0.8)).align(Align::Center),
            );
        }
        paint.unclip();
    }

    fn menu_button_bounds(&self, index: usize) -> Rect {
        layout::menu(self.width, self.height, self.menu_scroll).actions[index]
    }

    fn paint_game(&self, game: &Game, paint: &mut Paint) {
        let g = self.geometry();
        let header = g.header;
        let hardcore = game.is_hardcore();
        let area = if hardcore { g.viewport } else { g.board };
        let enter = animation::elapsed(game.now_millis, self.screen_enter);
        let outgoing = self.outgoing_screen(game);
        let motion = if self.motion(game) && hardcore {
            if let Some(outgoing) = outgoing {
                Some((
                    animation::exit(outgoing, animation::SCREEN_OUT),
                    1.0 - (outgoing / animation::SCREEN_OUT).clamp(0.0, 1.0),
                ))
            } else if enter < animation::SCREEN_IN {
                Some((
                    animation::enter(enter, animation::SCREEN_IN),
                    animation::EASE_OUT.sample(enter / animation::SCREEN_IN),
                ))
            } else {
                None
            }
        } else {
            None
        };
        if let Some((_, alpha)) = motion {
            paint.transform(
                area.center(),
                animation::Transform {
                    alpha,
                    ..animation::Transform::default()
                },
            );
        }
        self.paint_board(game, paint, area);
        if let Some((transform, _)) = motion {
            paint.restore();
            paint.transform(header.center(), transform);
        }
        let menu = Rect::new(
            header.x,
            header.center().y - g.menu_size * 0.5,
            g.menu_size,
            g.menu_size,
        );
        paint.styled(menu, theme::ICON_BUTTON);
        paint.glyph(menu.center(), theme::Icon::Menu, 24.0, theme::WHITE);
        paint.control(menu, "В меню", Action::Navigate(Screen::Menu));
        let balance = Self::balance_box(
            header.right(),
            header.center().y - 23.5,
            game.state.coins,
            false,
        );
        self.balance(paint, balance, game.state.coins, false);
        let label = if hardcore {
            "Хардкор".to_owned()
        } else {
            format!("Уровень {}", game.level().id)
        };
        let count_height = theme::line_height(13.0, false);
        let group_height = if hardcore {
            23.0 + 5.0 + count_height
        } else {
            23.0
        };
        let label_top = header.center().y - group_height * 0.5;
        paint.line_text(
            header.center().x,
            label_top,
            23.0,
            &label,
            TextStyle::new(20.0, theme::WHITE)
                .bold()
                .align(Align::Center),
        );
        let label_width = theme::text_width(&label, 20.0, true);
        paint.semantic(
            1,
            Rect::new(
                header.center().x - label_width * 0.5,
                label_top,
                label_width,
                23.0,
            ),
            &label,
        );
        if hardcore {
            let solved = game.progress().solved.len();
            let total = game.level().words.len();
            let count = format!("{solved}/{total}");
            paint.line_text(
                header.center().x,
                label_top + 28.0,
                count_height,
                &count,
                TextStyle::new(13.0, Color::hex(0xf2dbe9))
                    .align(Align::Center)
                    .spacing(0.52),
            );
            let count_width = theme::text_width(&count, 13.0, false) + count.len() as f32 * 0.52;
            paint.semantic(
                2,
                Rect::new(
                    header.center().x - count_width * 0.5,
                    label_top + 28.0,
                    count_width,
                    count_height,
                ),
                &format!("Найдено {solved} из {total} слов на поле"),
            );
            let regenerate = Rect::new(balance.right() - 39.0, balance.bottom() + 5.0, 32.0, 32.0);
            paint.styled(regenerate, theme::HARDCORE_REGENERATE);
            paint.glyph(
                regenerate.center(),
                theme::Icon::Regenerate,
                19.0,
                Color::hex(0xfff5ed),
            );
            paint.control(
                regenerate,
                "Перегенерировать уровень",
                Action::Panel(Panel::Regenerate),
            );
        } else {
            let short = self.height <= 620.0;
            let hint = Rect::new(
                menu.x,
                menu.bottom() + if short { 16.0 } else { 18.0 },
                g.menu_size,
                if short { 68.0 } else { 71.0 },
            );
            let enabled =
                game.state.coins >= 20 && game.cells().len() > game.progress().revealed.len();
            let square = Rect::new(hint.center().x - 23.0, hint.y, 46.0, hint.h - 25.0);
            if !enabled {
                paint.transform(
                    hint.center(),
                    animation::Transform {
                        alpha: 0.45,
                        ..animation::Transform::default()
                    },
                );
            }
            paint.styled(square, theme::HINT_SQUARE);
            paint.glyph(
                square.center(),
                theme::Icon::Bulb,
                27.0,
                Color::hex(0x74448f),
            );
            let cost_width = theme::text_width("20", 12.0, true);
            let cost_x = hint.center().x - (cost_width + 22.0) * 0.5;
            let cost_y = square.bottom() + 5.0;
            paint.line_text(
                cost_x,
                cost_y,
                20.0,
                "20",
                TextStyle::new(12.0, Color::hex(0xfff3da))
                    .bold()
                    .shadows(&[theme::Shadow::new(0.0, 1.0, 3.0, 0.0, Color::hex(0x241435))]),
            );
            paint.coin(cost_x + cost_width + 2.0, cost_y, 20.0);
            if !enabled {
                paint.restore();
            }
            if enabled {
                paint.control(hint, "Подсказка, 20 монет", Action::Hint);
            }
        }
        if let Some((transform, _)) = motion {
            paint.restore();
            let wheel_area = Rect::new(
                g.preview.x,
                g.preview.y,
                g.preview.w,
                g.wheel.bottom() - g.preview.y,
            );
            paint.transform(wheel_area.center(), transform);
        }
        self.paint_wheel(game, paint, &g);
        if motion.is_some() {
            paint.restore();
        }
        if let Some(hint) = &self.idle {
            if !hint.marks_tiles {
                if let Some(meaning) = game.meanings(&hint.word).first() {
                    let text = format!("Подсказка: {meaning}");
                    let max_width = header.w
                        + (header.x - (self.width - self.width.min(540.0)) * 0.5) * 2.0
                        - 120.0;
                    let lines = theme::wrap(&text, max_width - 32.0, 15.0, true);
                    let width = lines
                        .iter()
                        .map(|line| theme::text_width(line, 15.0, true))
                        .fold(0.0, f32::max)
                        + 32.0;
                    let line_height = theme::line_height(15.0, true);
                    let rect = Rect::new(
                        header.center().x - width * 0.5,
                        120.0,
                        width,
                        24.0 + lines.len() as f32 * line_height,
                    );
                    paint.styled(
                        rect,
                        BoxStyle::new(Color::hex(0xfff7ed), 12.0).shadows(&[theme::Shadow::new(
                            0.0,
                            6.0,
                            24.0,
                            0.0,
                            Color::css(0x24143544),
                        )]),
                    );
                    for (index, line) in lines.iter().enumerate() {
                        paint.line_text(
                            rect.center().x,
                            rect.y + 12.0 + index as f32 * line_height,
                            line_height,
                            line,
                            TextStyle::new(15.0, Color::hex(0x633c75))
                                .bold()
                                .align(Align::Center),
                        );
                    }
                }
            }
        }
        if let Some((transform, _)) = motion {
            let center = Point::new(
                g.found.center().x,
                g.found.center().y - if self.height <= 760.0 { 4.0 } else { 6.0 },
            );
            paint.transform(center, transform);
        }
        let inset = if self.width <= 360.0 { 9.0 } else { 13.0 };
        let font_size = if self.width <= 360.0 { 12.0 } else { 14.0 };
        paint.styled(g.found, theme::FOUND_BUTTON);
        paint.glyph(
            Point::new(g.found.x + inset + 10.0, g.found.center().y),
            theme::Icon::Book,
            20.0,
            theme::WHITE,
        );
        paint.line_text(
            g.found.x + inset + 28.0,
            g.found.y,
            g.found.h,
            "Найденные слова",
            TextStyle::new(font_size, theme::WHITE),
        );
        paint.control(g.found, "Найденные слова", Action::Panel(Panel::Found));
        if motion.is_some() {
            paint.restore();
        }
    }

    fn paint_board(&self, game: &Game, paint: &mut Paint, area: Rect) {
        let size = self.camera.cell(game, area);
        let step = size + self.camera.gap(game);
        let origin = self.camera.origin(game, area);
        let progress = game.progress();
        let revealed: HashSet<_> = progress.revealed.iter().map(String::as_str).collect();
        let solved: HashSet<_> = progress.solved.iter().map(String::as_str).collect();
        let mut foremost = Vec::new();
        paint.clip(area);
        for (index, cell) in game.cells().iter().enumerate() {
            let rect = Rect::new(
                origin.x + cell.col as f32 * step,
                origin.y + cell.row as f32 * step,
                size,
                size,
            );
            if rect.intersection(area).is_none() {
                continue;
            }
            let shown = revealed.contains(cell.key.as_str());
            if shown {
                paint.semantic(100 + index, rect, &cell.letter.to_string());
            }
            let complete = shown && cell.words.iter().any(|word| solved.contains(word.as_str()));
            let style = if complete {
                theme::CELL_REVEALED
            } else if shown {
                theme::CELL_PARTIAL
            } else {
                theme::CELL
            };
            let color = if complete {
                theme::WHITE
            } else {
                Color::hex(0x553467)
            };
            let effect = self
                .cell_effects
                .iter()
                .find(|effect| effect.key == cell.key);
            let repeat = self
                .repeat
                .as_ref()
                .filter(|word| cell.words.contains(&word.text));
            if repeat.is_some() || effect.is_some_and(|effect| effect.found) {
                foremost.push((rect, shown, complete, cell, style, color, effect, repeat));
                continue;
            }
            let framed = if self.motion(game) {
                effect.map(|effect| {
                    animation::hint(animation::elapsed(game.now_millis, effect.began))
                })
            } else {
                None
            };
            if let Some(transform) = framed {
                paint.transform(rect.center(), transform);
            }
            paint.styled(rect, style);
            if shown {
                paint.styled_center(
                    rect,
                    &cell.letter.to_string(),
                    TextStyle::new(size * 0.62, color).bold(),
                );
            }
            if framed.is_some() {
                paint.restore();
            }
        }
        for (rect, shown, _, cell, style, color, effect, repeat) in foremost {
            let now = game.now_millis;
            let elapsed = effect.map_or(0.0, |effect| animation::elapsed(now, effect.began));
            let found_shadows = if self.motion(game) {
                animation::word_shadows(elapsed)
            } else {
                [
                    theme::Shadow::new(0.0, 0.0, 0.0, 2.0, Color::hex(0xffe09a)),
                    theme::Shadow::new(0.0, 0.0, 15.0, 3.0, Color::css(0xffc557b3)),
                ]
            };
            let (style, transform) = if let Some(repeat) = repeat {
                (
                    style.shadows(theme::CELL_REPEATED.shadows),
                    animation::word_repeat(animation::elapsed(now, repeat.until - 1000)),
                )
            } else {
                (
                    style
                        .with_color(Color::hex(0x9459b5))
                        .shadows(&found_shadows),
                    animation::word_found(elapsed),
                )
            };
            if self.motion(game) {
                paint.transform(rect.center(), transform);
            }
            paint.styled(rect, style);
            if shown {
                paint.styled_center(
                    rect,
                    &cell.letter.to_string(),
                    TextStyle::new(size * 0.62, color).bold(),
                );
            }
            if self.motion(game) {
                paint.restore();
            }
        }
        paint.unclip();
    }

    fn paint_wheel(&self, game: &Game, paint: &mut Paint, g: &Geometry) {
        let center = g.wheel.center();
        let positions = g.tiles(game.rack.len());
        let radius = g.tile_radius(game.rack.len());
        let tile_size = radius * 2.0;
        let wheel_font = (tile_size * 0.65).floor().min(34.0);
        paint.styled(g.wheel, theme::WHEEL);
        for neighbors in self.selection.windows(2) {
            if let (Some(&a), Some(&b)) = (positions.get(neighbors[0]), positions.get(neighbors[1]))
            {
                paint.line(a, b, g.wheel.w * 0.038, "#9457b3");
            }
        }
        let mut hinted: Vec<_> = self
            .idle
            .as_ref()
            .filter(|hint| hint.marks_tiles)
            .map(|hint| hint.word.chars().collect())
            .unwrap_or_default();
        let shuffle_factor = self.shuffle.as_ref().map_or(1.0, |shuffle| {
            animation::SHUFFLE_CURVE.sample(
                animation::elapsed(game.now_millis, shuffle.began) / animation::TILE_SHUFFLE,
            )
        });
        for (index, (&letter, &slot)) in game.rack.iter().zip(&positions).enumerate() {
            let at = self
                .shuffle
                .as_ref()
                .and_then(|shuffle| shuffle.previous.get(index))
                .and_then(|&slot| positions.get(slot))
                .map_or(slot, |&old| animation::point(old, slot, shuffle_factor));
            let selected = self.selection.contains(&index);
            let mark = hinted
                .iter()
                .position(|&candidate| candidate == letter)
                .is_some_and(|index| {
                    hinted.remove(index);
                    true
                });
            let style = if selected {
                theme::LETTER_SELECTED
            } else if mark {
                theme::LETTER_IDLE
            } else {
                theme::LETTER
            };
            let r = Rect::new(at.x - radius, at.y - radius, tile_size, tile_size);
            paint.styled(r, style);
            let color = if selected {
                theme::WHITE
            } else if mark {
                Color::hex(0x593861)
            } else {
                theme::CROSSWORD_LETTER
            };
            let label = letter.to_string();
            paint.styled_center(r, &label, TextStyle::new(wheel_font, color).bold());
            if self.shuffle.is_none() {
                paint.control(
                    r,
                    &format!("{label}; буква {}", index + 1),
                    Action::Select(index),
                );
            }
        }
        paint.glyph(center, theme::Icon::Shuffle, 25.0, Color::hex(0x947aa4));
        let shuffle = Rect::new(center.x - 24.5, center.y - 24.5, 49.0, 49.0);
        paint.control(shuffle, "Перемешать буквы", Action::Shuffle);
        let word = self
            .invalid
            .as_ref()
            .map_or_else(|| self.selected_word(game), |invalid| invalid.text.clone());
        if !word.is_empty() {
            let count = word.chars().count() as f32;
            let size = ((g.preview.w.round() - (count - 1.0) * 4.0) / count)
                .floor()
                .min(36.0);
            let step = size + 4.0;
            let left = g.preview.center().x - (count * step - 4.0) * 0.5;
            let invalid = self
                .motion(game)
                .then_some(self.invalid.as_ref())
                .flatten()
                .map(|invalid| {
                    animation::invalid(animation::elapsed(game.now_millis, invalid.until - 420))
                });
            for (index, letter) in word.chars().enumerate() {
                let rect = Rect::new(
                    left + index as f32 * step,
                    g.preview.center().y - size * 0.5,
                    size,
                    size,
                );
                if let Some(transform) = invalid {
                    paint.transform(rect.center(), transform);
                }
                paint.styled(rect, theme::PREVIEW_CELL);
                paint.styled_center(
                    rect,
                    &letter.to_string(),
                    TextStyle::new(size * 0.62, theme::CROSSWORD_LETTER).bold(),
                );
                if invalid.is_some() {
                    paint.restore();
                }
            }
            if self.gesture.is_none() && self.invalid.is_none() {
                paint.control(g.preview, "Проверить слово", Action::Submit);
            }
        }
    }
}

fn daily_remaining(game: &Game, next_midnight: Option<i64>) -> String {
    let date_end = next_midnight.unwrap_or((game.local_day + 1) * 86_400_000);
    let secs = ((date_end - game.now_millis + 999) / 1000).clamp(0, 86_400);
    format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}

const ABOUT_LICENSE: &str = "Игра использует базу башкирских существительных. В подробных записях сохранены значения и источники. Условия использования отдельных источников приведены в полных сведениях.";
const ABOUT_ATTRIBUTION: &str = "Полные сведения об источниках и лицензии";
const ABOUT_REVIEW: &str = "База прошла автоматическую проверку; полная лингвистическая вычитка не проводилась. Русские толкования показаны в исходной формулировке.";
const ABOUT_FEEDBACK: &str =
    "В словарной базе могут отсутствовать некоторые слова. Нашли такие? Сообщите нам.";
const ABOUT_CONTACT_QUESTION: &str = "Задать вопрос:";
const ABOUT_BODY_LINE: f32 = 25.59375;
const ABOUT_NOTE_LINE: f32 = 22.390625;

struct AboutLayout {
    endless: Vec<String>,
    license: Vec<String>,
    attribution: Vec<String>,
    review: Vec<String>,
    feedback: Vec<String>,
}

impl AboutLayout {
    fn new(width: f32) -> Self {
        let statistic_width = (width - 12.0) * 0.5 - 16.0;
        Self {
            endless: theme::wrap(
                "Неограниченное количество уровней",
                statistic_width,
                13.0,
                false,
            ),
            license: theme::wrap(ABOUT_LICENSE, width, 16.0, false),
            attribution: theme::wrap(ABOUT_ATTRIBUTION, width, 14.0, false),
            review: theme::wrap(ABOUT_REVIEW, width, 14.0, false),
            feedback: theme::wrap(ABOUT_FEEDBACK, width, 14.0, false),
        }
    }

    fn stats_height(&self) -> f32 {
        30.0 + theme::line_height(26.0, true)
            + self.endless.len().max(1) as f32 * theme::line_height(16.0, false)
    }

    fn height(&self) -> f32 {
        self.stats_height()
            + 18.0
            + self.license.len() as f32 * ABOUT_BODY_LINE
            + 13.0
            + self.attribution.len() as f32 * ABOUT_NOTE_LINE
            + 13.0
            + self.review.len() as f32 * ABOUT_NOTE_LINE
            + 8.0
            + self.feedback.len() as f32 * ABOUT_NOTE_LINE
            + 18.0
            + ABOUT_BODY_LINE
            + 13.0
            + ABOUT_NOTE_LINE
    }
}

impl Ui {
    fn paint_victory(&self, game: &Game, paint: &mut Paint) {
        let width = self.width.min(500.0) - 48.0;
        let x = (self.width - width) * 0.5;
        let cx = self.width * 0.5;
        let compact = self.height <= 650.0;
        let header_gap = if compact { 7.0 } else { 12.0 };
        let stage_gap = if compact { 10.0 } else { 16.0 };
        let badge_size = if compact { 50.0 } else { 96.0 };
        let mut y = if compact { 12.0 } else { 22.0 };
        let badge = Rect::new(cx - badge_size * 0.5, y, badge_size, badge_size);
        let animation = animation::elapsed(game.now_millis, self.screen_enter);
        let popping = self.motion(game) && animation < 337.5;
        if popping {
            let p = animation::EASE_OUT.sample(animation / 337.5);
            paint.transform(
                badge.center(),
                animation::Transform {
                    scale: animation::mix(0.65, 1.0, p),
                    ..animation::Transform::default()
                },
            );
        }
        paint.styled(
            badge,
            if compact {
                theme::VICTORY_BADGE_COMPACT
            } else {
                theme::VICTORY_BADGE
            },
        );
        paint.glyph(
            badge.center(),
            theme::Icon::Trophy,
            if compact { 30.0 } else { 46.0 },
            Color::hex(0x83528c),
        );
        if popping {
            paint.restore();
        }
        y += badge_size + header_gap;
        let stage = if game.is_hardcore() {
            "ХАРДКОР".to_owned()
        } else {
            format!("УРОВЕНЬ {}", game.level().id)
        };
        paint.line_text(
            cx,
            y,
            14.0,
            &stage,
            TextStyle::new(13.0, theme::EYEBROW)
                .align(Align::Center)
                .spacing(13.0 * 0.18),
        );
        y += 14.0 + header_gap;
        let title_size = if compact { 27.0 } else { 34.0 };
        let title_lines = theme::wrap("Уровень пройден!", width, title_size, true);
        let title_line = if compact { 31.0 } else { 39.0 };
        for line in title_lines {
            paint.line_text(
                cx,
                y,
                title_line,
                &line,
                TextStyle::new(title_size, theme::WHITE)
                    .bold()
                    .align(Align::Center),
            );
            y += title_line;
        }
        y += header_gap;
        let note_size = if compact { 13.0 } else { 16.0 };
        let note_line = theme::line_height(note_size, false);
        for line in theme::wrap(
            "Все слова найдены. Узнайте, что они означают.",
            width,
            note_size,
            false,
        ) {
            paint.line_text(
                cx,
                y,
                note_line,
                &line,
                TextStyle::new(note_size, theme::MUTED).align(Align::Center),
            );
            y += note_line;
        }
        y += header_gap;
        let reward = if self.reward > 0 {
            format!("Награда: +{}", self.reward)
        } else {
            "Награда за уровень уже получена".to_owned()
        };
        let pad_x = if compact { 17.0 } else { 21.0 };
        let pad_y = if compact { 7.0 } else { 11.0 };
        let reward_w = (2.0 * pad_x + 34.0 + theme::text_width(&reward, 16.0, false)).min(width);
        let reward_lines = theme::wrap(
            &reward,
            (reward_w - 34.0 - 2.0 * pad_x).max(1.0) + 0.05,
            16.0,
            false,
        );
        let text_height = reward_lines.len() as f32 * 18.0;
        let h = 2.0 * pad_y + 25.0_f32.max(text_height);
        let chip = Rect::new(cx - reward_w * 0.5, y, reward_w, h);
        paint.styled(chip, theme::REWARD_CHIP);
        paint.coin(chip.x + pad_x, chip.center().y - 12.5, 25.0);
        for (index, line) in reward_lines.iter().enumerate() {
            paint.line_text(
                chip.x + pad_x + 34.0,
                chip.center().y - text_height * 0.5 + index as f32 * 18.0,
                18.0,
                line,
                TextStyle::new(16.0, theme::WHITE).align(Align::Center),
            );
            // The reward span is horizontally centered within its wrapping slot.
            if let Some(last) = paint.commands.last_mut() {
                last["x"] = json!(chip.x + pad_x + 34.0 + (chip.w - 34.0 - 2.0 * pad_x) * 0.5);
            }
        }
        y += h + stage_gap;
        let button_gap = if compact { 8.0 } else { 12.0 };
        let primary_h = if compact { 40.0 } else { 52.0 };
        let secondary_h = if compact { 42.0 } else { 50.0 };
        let bottom = self.height - if compact { 12.0 } else { 20.0 };
        let menu = Rect::new(x, bottom - secondary_h, width, secondary_h);
        let next = Rect::new(x, menu.y - button_gap - primary_h, width, primary_h);
        self.word_list(
            game,
            paint,
            Rect::new(x, y, width, (next.y - stage_gap - y).max(0.0)),
            self.victory_scroll,
            false,
        );
        paint.styled(next, theme::PRIMARY);
        paint.styled_center(
            next,
            "Следующий уровень",
            TextStyle::new(16.0, theme::DEEP).bold(),
        );
        paint.control(next, "Следующий уровень", Action::Next);
        paint.styled(menu, theme::SECONDARY);
        paint.styled_center(menu, "В меню", TextStyle::new(16.0, theme::WHITE).bold());
        paint.control(menu, "В меню", Action::Navigate(Screen::Menu));
    }

    fn dialog_width(&self, panel: &Panel) -> f32 {
        let max = if matches!(panel, Panel::Attribution) {
            560.0
        } else {
            430.0
        };
        (self.width - 38.0).min(max).max(16.0)
    }

    fn dialog_padding(&self) -> (f32, f32) {
        if self.width <= 360.0 {
            (20.0, 24.0)
        } else {
            (25.0, 27.0)
        }
    }

    fn dialog_heading(&self, panel: &Panel, content_width: f32) -> (Vec<String>, f32) {
        let text = if matches!(panel, Panel::Feedback) {
            ""
        } else {
            panel.title()
        };
        let lines = theme::wrap(text, (content_width - 51.0).max(1.0), 22.0, true);
        let h = (lines.len() as f32 * 28.59375).max(36.0);
        (lines, h)
    }

    fn body_height(&self, game: &Game, panel: &Panel, w: f32) -> f32 {
        match panel {
            Panel::Found => {
                let n = game.progress().found_order.len();
                if n == 0 {
                    let p = theme::wrap(
                        "Здесь появятся разгаданные и бонусные слова.",
                        w - 8.0,
                        16.0,
                        false,
                    );
                    30.0 + p.len() as f32 * 25.6
                } else {
                    (n * 65 + (n - 1) * 10 + 4) as f32
                }
            }
            Panel::Settings => {
                let p = theme::wrap("Настройки сохраняются на этом устройстве.", w, 14.0, false);
                284.0
                    + 22.0
                    + theme::line_height(GALLERY_HEADING_SIZE, true)
                    + GALLERY_HEADING_MARGIN
                    + GALLERY_HEIGHT
                    + GALLERY_SCROLLBAR
                    + 13.0
                    + p.len() as f32 * 22.390625
                    + 20.0
                    + 65.0
            }
            Panel::Translation { word, .. } => {
                let entry = game.dictionary_entry(word);
                let lines = Self::meaning_lines(game, word, w);
                let heights = lines
                    .iter()
                    .map(|rows| rows.len() as f32 * 24.0 + 4.0)
                    .sum::<f32>();
                let word_height = theme::line_height(28.0, true);
                let positions = Self::positions(game, word);
                let details = if entry.is_some() { 46.0 } else { 0.0 };
                let more = if self.sources_open && entry.is_some() {
                    let footer = theme::wrap(
                        "Значения из локального словаря. Просмотр бесплатный.",
                        w,
                        12.0,
                        false,
                    );
                    let id = entry
                        .map(|entry| format!("Запись: {}", entry.id))
                        .unwrap_or_default();
                    let lines = footer.len() + theme::wrap(&id, w, 12.0, false).len();
                    lines as f32 * 18.0
                        + 16.0
                        + if Self::translation_links(game, word).is_empty() {
                            0.0
                        } else {
                            25.5
                        }
                } else {
                    0.0
                };
                word_height
                    + 8.0
                    + if positions.is_empty() {
                        0.0
                    } else {
                        POS_HEIGHT
                    }
                    + if lines.is_empty() {
                        13.0 + theme::wrap("В этой базе пока нет русского перевода", w, 16.0, false)
                            .len() as f32
                            * 25.6
                    } else {
                        heights.min(144.0)
                            + if positions.is_empty() {
                                0.0
                            } else {
                                POS_MARGIN
                            }
                    }
                    + details
                    + more
            }
            Panel::Rating => 45.59375,
            Panel::Regenerate => 48.0,
            Panel::Reset => {
                let text = theme::wrap(RESET_DESCRIPTION, w, 16.0, false);
                text.len() as f32 * 25.59375 + 24.0 + 44.0
            }
            Panel::Purchase(index) => {
                let warning = game.state.coins < BACKGROUNDS[*index].price;
                170.0
                    + 12.0
                    + 23.0
                    + 12.0
                    + 22.0
                    + 24.0
                    + 44.0
                    + if warning { 22.390625 + 13.0 } else { 0.0 }
            }
            Panel::Feedback => {
                12.0 + 120.0
                    + 13.0
                    + 22.390625
                    + 13.0
                    + 8.0
                    + 34.0
                    + 2.0 * theme::line_height(16.0, false)
            }
            Panel::About => AboutLayout::new(w).height(),
            Panel::Attribution => self.height,
        }
    }

    fn panel_rect(&self, game: &Game, panel: &Panel) -> Rect {
        let width = self.dialog_width(panel);
        let (px, py) = self.dialog_padding();
        let inner_w = (width - px * 2.0).max(16.0);
        let (_, header) = self.dialog_heading(panel, inner_w);
        let max_height = if matches!(panel, Panel::Found) {
            (self.height * 0.5).min(self.height - 38.0)
        } else {
            self.height - 38.0
        };
        let height =
            (2.0 * py + header + 20.0 + self.body_height(game, panel, inner_w)).min(max_height);
        Rect::new(
            (self.width - width) * 0.5,
            (self.height - height) * 0.5,
            width,
            height,
        )
    }

    fn paint_panel(
        &self,
        game: &Game,
        paint: &mut Paint,
        panel: &Panel,
        underlay_scroll: Option<f32>,
    ) {
        let card = self.panel_rect(game, panel);
        let elapsed = animation::elapsed(game.now_millis, self.panel_enter);
        let animated = underlay_scroll.is_none() && self.motion(game);
        let exiting = self
            .leaving
            .as_ref()
            .filter(|_| underlay_scroll.is_none())
            .map(|transition| animation::elapsed(game.now_millis, transition.began));
        if exiting.is_some_and(|elapsed| elapsed >= animation::DIALOG_OUT) {
            return;
        }
        if animated && elapsed < animation::DIALOG_BACKDROP {
            let alpha = animation::EASE_OUT.sample(elapsed / animation::DIALOG_BACKDROP);
            paint.transform(
                card.center(),
                animation::Transform {
                    alpha,
                    ..animation::Transform::default()
                },
            );
            paint.styled(
                Rect::new(0.0, 0.0, self.width, self.height),
                theme::DIALOG_BACKDROP,
            );
            paint.restore();
        } else {
            paint.styled(
                Rect::new(0.0, 0.0, self.width, self.height),
                theme::DIALOG_BACKDROP,
            );
        }
        let transform = if animated {
            if let Some(exiting) = exiting {
                Some(animation::exit(exiting, animation::DIALOG_OUT))
            } else if elapsed < animation::DIALOG_IN {
                Some(animation::enter(elapsed, animation::DIALOG_IN))
            } else {
                None
            }
        } else {
            None
        };
        if let Some(transform) = transform {
            paint.transform(card.center(), transform);
        }
        paint.styled(card, theme::DIALOG);
        paint.clip(card);
        let (px, py) = self.dialog_padding();
        let w = (card.w - px * 2.0).max(16.0);
        let (lines, header_h) = self.dialog_heading(panel, w);
        let body_h = self.body_height(game, panel, w);
        let x = card.x + px;
        let header_y = card.y + py;
        let outer = matches!(
            panel,
            Panel::Settings
                | Panel::Translation { .. }
                | Panel::About
                | Panel::Purchase(_)
                | Panel::Feedback
        );
        let max = (2.0 * py + header_h + 20.0 + body_h - card.h).max(0.0);
        let scroll = if outer {
            underlay_scroll.unwrap_or(self.scroll).clamp(0.0, max)
        } else {
            0.0
        };
        let close = Rect::new(
            if matches!(panel, Panel::Feedback) {
                x
            } else {
                card.right() - px - 36.0
            },
            header_y + (header_h - 36.0) * 0.5 - scroll,
            36.0,
            36.0,
        );
        let first_y = header_y + (header_h - lines.len() as f32 * 28.59375) * 0.5 - scroll;
        for (i, line) in lines.iter().enumerate() {
            paint.line_text(
                x,
                first_y + i as f32 * 28.59375,
                28.59375,
                line,
                TextStyle::new(22.0, theme::INK).bold(),
            );
        }
        paint.styled(close, theme::CLOSE_MODAL);
        paint.glyph(
            close.center(),
            theme::Icon::Close,
            24.0,
            Color::hex(0x704384),
        );
        paint.control(close, "Закрыть", Action::Close);
        let y = header_y + header_h + 20.0 - scroll;
        let bounds = Rect::new(
            x,
            y,
            w,
            if outer {
                body_h
            } else {
                (card.bottom() - py - y).max(0.0)
            },
        );
        match panel {
            Panel::Found => self.word_list(game, paint, bounds, self.found_scroll, true),
            Panel::Settings => self.paint_settings(game, paint, bounds),
            Panel::Translation { word, .. } => self.paint_translation(game, paint, bounds, word),
            Panel::About => self.paint_about(game, paint, bounds),
            Panel::Attribution => self.paint_attribution(paint, bounds),
            Panel::Rating => paint.line_text(
                bounds.center().x,
                bounds.y,
                25.59375,
                "В разработке",
                TextStyle::new(16.0, theme::INK).align(Align::Center),
            ),
            Panel::Feedback => self.paint_feedback(paint, bounds),
            Panel::Reset => self.confirm(
                paint,
                bounds,
                RESET_DESCRIPTION,
                "Сбросить",
                Action::Reset,
                true,
            ),
            Panel::Regenerate => {
                self.confirm(paint, bounds, "", "Пересобрать", Action::Regenerate, false)
            }
            Panel::Purchase(index) => self.paint_purchase(game, paint, bounds, *index),
        }
        paint.unclip();
        if outer {
            paint.scroll = Some(ScrollArea {
                bounds: card,
                max,
                kind: ScrollKind::Main,
            });
        }
        if transform.is_some() {
            paint.restore();
        }
    }

    fn confirm(
        &self,
        paint: &mut Paint,
        bounds: Rect,
        description: &str,
        label: &str,
        action: Action,
        danger: bool,
    ) {
        let lines = theme::wrap(description, bounds.w, 16.0, false);
        for (i, line) in lines.iter().enumerate() {
            paint.line_text(
                bounds.x,
                bounds.y + i as f32 * 25.59375,
                25.59375,
                line,
                TextStyle::new(16.0, theme::INK),
            );
        }
        let h = 44.0;
        let y = bounds.bottom() - h;
        let w = (bounds.w - 10.0) * 0.5;
        let cancel = Rect::new(bounds.x, y, w, h);
        let confirm = Rect::new(bounds.right() - w, y, w, h);
        paint.styled(cancel, theme::MODAL_SECONDARY);
        paint.styled_center(
            cancel,
            "Отмена",
            TextStyle::new(14.0, Color::hex(0x724886)).bold(),
        );
        paint.control(
            cancel,
            "Отмена",
            if danger {
                Action::Panel(Panel::Settings)
            } else {
                Action::Close
            },
        );
        paint.styled(
            confirm,
            if danger {
                theme::DANGER
            } else {
                theme::PRIMARY_PURPLE
            },
        );
        paint.styled_center(
            confirm,
            label,
            TextStyle::new(
                14.0,
                if danger {
                    Color::hex(0x963a52)
                } else {
                    theme::WHITE
                },
            )
            .bold(),
        );
        paint.control(confirm, label, action);
    }

    fn paint_purchase(&self, game: &Game, paint: &mut Paint, bounds: Rect, index: usize) {
        let bg = &BACKGROUNDS[index];
        let center = bounds.center().x;
        paint.commands.push(theme::background_card(
            center - 75.0,
            bounds.y,
            150.0,
            170.0,
            &format!("assets/background-{}.webp", bg.id),
            12.0,
        ));
        paint.line_text(
            center,
            bounds.y + 182.0,
            23.0,
            bg.title,
            TextStyle::new(20.0, theme::INK).bold().align(Align::Center),
        );
        let text = bg.price.to_string();
        let w = theme::text_width(&text, 18.0, false);
        let left = center - (w + 26.0) * 0.5;
        paint.line_text(
            left,
            bounds.y + 217.0,
            22.0,
            &text,
            TextStyle::new(18.0, theme::INK),
        );
        paint.coin(left + w + 4.0, bounds.y + 217.0, 22.0);
        let poor = game.state.coins < bg.price;
        if poor {
            paint.line_text(
                bounds.x,
                bounds.y + 252.0,
                22.390625,
                "Недостаточно монет.",
                TextStyle::new(14.0, Color::hex(0x8d7196)),
            );
        }
        let button_width = (bounds.w - 10.0) * 0.5;
        let y = bounds.bottom() - 44.0;
        let cancel = Rect::new(bounds.x, y, button_width, 44.0);
        let buy = Rect::new(bounds.right() - button_width, y, button_width, 44.0);
        paint.styled(cancel, theme::MODAL_SECONDARY);
        paint.styled_center(
            cancel,
            "Отмена",
            TextStyle::new(14.0, Color::hex(0x724886)).bold(),
        );
        paint.control(cancel, "Отмена", Action::Panel(Panel::Settings));
        if poor {
            paint.transform(
                buy.center(),
                animation::Transform {
                    alpha: 0.45,
                    ..animation::Transform::default()
                },
            );
        }
        paint.styled(buy, theme::PRIMARY_PURPLE);
        paint.styled_center(buy, "Купить", TextStyle::new(14.0, theme::WHITE).bold());
        if poor {
            paint.restore();
        } else {
            paint.control(buy, "Купить", Action::Buy(index));
        }
    }

    fn paint_feedback(&self, paint: &mut Paint, bounds: Rect) {
        let line_height = theme::line_height(16.0, false);
        paint.line_text(
            bounds.x,
            bounds.y,
            line_height,
            "Каких слов не хватает?",
            TextStyle::new(16.0, theme::INK),
        );
        let text = Rect::new(bounds.x, bounds.y + line_height + 12.0, bounds.w, 120.0);
        paint.styled(
            text,
            BoxStyle::new(Color::hex(0xffffff), 10.0).border(1.0, Color::hex(0xd7c3de)),
        );
        let view = text.inset(13.0);
        let (value, color) = if self.feedback_text.is_empty() {
            ("Напишите слово и его перевод", Color::hex(0x757575))
        } else {
            (self.feedback_text.as_str(), theme::INK)
        };
        let rows: Vec<_> = value
            .split('\n')
            .flat_map(|line| {
                if line.trim().is_empty() {
                    vec![String::new()]
                } else {
                    theme::wrap(line, view.w, 16.0, false)
                }
            })
            .collect();
        let below = (rows.len() as f32 * line_height - view.h).max(0.0);
        paint.clip(view);
        for (index, line) in rows.iter().enumerate() {
            paint.line_text(
                view.x,
                view.y + index as f32 * line_height - below,
                line_height,
                line,
                TextStyle::new(16.0, color),
            );
        }
        paint.unclip();
        paint.control(text, "Каких слов не хватает?", Action::EditFeedback);
        paint.line_text(
            bounds.x,
            text.bottom() + 13.0,
            22.390625,
            "Отправка пока недоступна.",
            TextStyle::new(14.0, Color::hex(0x8d7196)),
        );
        let send = Rect::new(
            bounds.x,
            text.bottom() + 56.390625,
            bounds.w,
            line_height + 34.0,
        );
        paint.styled(send, theme::PRIMARY_PURPLE);
        paint.styled_center(send, "Отправить", TextStyle::new(16.0, theme::WHITE).bold());
        paint.control(send, "Отправить", Action::SubmitFeedback);
    }

    fn word_list(
        &self,
        game: &Game,
        paint: &mut Paint,
        bounds: Rect,
        scroll: f32,
        from_found: bool,
    ) {
        let words = &game.progress().found_order;
        let pad_y = if from_found { 2.0 } else { 3.0 };
        let top = bounds.y + pad_y;
        let left = bounds.x + 4.0;
        let width = (bounds.w - 8.0).max(0.0);
        let solved: HashSet<_> = game.progress().solved.iter().map(String::as_str).collect();
        let button_width = theme::text_width("Перевод", 14.0, true) + 20.0;
        let rows: Vec<_> = words
            .iter()
            .map(|word| {
                let capacity = (width - 30.0 - 12.0 - button_width).max(1.0);
                let lines = if from_found {
                    vec![word.clone()]
                } else {
                    theme::wrap(word, capacity, 19.0, true)
                };
                let height = 26.0 + (lines.len() as f32 * 23.0 + 16.0).max(36.0);
                (word, lines, height)
            })
            .collect();
        let content = rows.iter().map(|(_, _, height)| *height).sum::<f32>()
            + ((rows.len().saturating_sub(1)) as f32) * 10.0
            + pad_y * 2.0;
        let max = (content - bounds.h).max(0.0);
        let scroll = scroll.clamp(0.0, max);
        paint.scroll = Some(ScrollArea {
            bounds,
            max,
            kind: ScrollKind::Main,
        });
        paint.clip(bounds);
        if rows.is_empty() {
            for (i, line) in theme::wrap(
                "Здесь появятся разгаданные и бонусные слова.",
                width,
                16.0,
                false,
            )
            .into_iter()
            .enumerate()
            {
                paint.line_text(
                    left,
                    top + 13.0 + i as f32 * 25.59375,
                    25.59375,
                    &line,
                    TextStyle::new(16.0, if from_found { theme::INK } else { theme::WHITE }),
                );
            }
        }
        let mut y = top - scroll;
        for (word, lines, height) in rows {
            let r = Rect::new(left, y, width, height);
            y += height + 10.0;
            if r.intersection(bounds).is_none() {
                continue;
            }
            paint.styled(
                r,
                if from_found {
                    theme::WORD_ROW
                } else {
                    theme::VICTORY_WORD_ROW
                },
            );
            let ty = r.y + 13.0;
            let text_x = r.x + 15.0;
            for (i, line) in lines.iter().enumerate() {
                paint.line_text(
                    text_x,
                    ty + i as f32 * 23.0,
                    23.0,
                    line,
                    TextStyle::new(19.0, Color::hex(0x523567)).bold(),
                );
            }
            let kind = if solved.contains(word.as_str()) {
                "Слово поля"
            } else {
                "Бонусное слово"
            };
            paint.line_text(
                text_x,
                ty + lines.len() as f32 * 23.0 + 3.0,
                13.0,
                kind,
                TextStyle::new(12.0, Color::hex(0x806885)),
            );
            let translate = Rect::new(
                r.right() - 15.0 - button_width,
                r.center().y - 18.0,
                button_width,
                36.0,
            );
            paint.styled(translate, theme::WORD_ROW_BUTTON);
            paint.styled_center(
                translate,
                "Перевод",
                TextStyle::new(14.0, Color::hex(0x653481)).bold(),
            );
            paint.control(
                translate,
                &format!("Перевод: {word}"),
                Action::Panel(Panel::Translation {
                    word: word.clone(),
                    from_found,
                }),
            );
        }
        paint.unclip();
    }

    fn paint_settings(&self, game: &Game, paint: &mut Paint, bounds: Rect) {
        let settings = &game.state.settings;
        let mut y = bounds.y;
        for (label, setting, active) in [
            ("Звук", Setting::Sound, settings.sound),
            ("Музыка", Setting::Music, settings.music),
            ("Анимации", Setting::Animations, settings.animations),
            ("Движение фона", Setting::Tilt, settings.background_motion),
        ] {
            let row = Rect::new(bounds.x, y, bounds.w, 71.0);
            let switch = Rect::new(row.right() - 54.0, y + 20.0, 54.0, 30.0);
            paint.line_text(row.x, y, 70.0, label, TextStyle::new(18.0, theme::INK));
            paint.styled(
                switch,
                if active {
                    theme::SWITCH_ACTIVE
                } else {
                    theme::SWITCH
                },
            );
            let knob = Rect::new(
                switch.x + 3.0 + if active { 24.0 } else { 0.0 },
                switch.y + 3.0,
                24.0,
                24.0,
            );
            paint.styled(knob, theme::SWITCH_KNOB);
            paint.control(
                switch,
                &format!(
                    "{label}, {}",
                    if active {
                        "включено"
                    } else {
                        "выключено"
                    }
                ),
                Action::Toggle(setting),
            );
            paint.rect(
                Rect::new(row.x, row.bottom() - 1.0, row.w, 1.0),
                0.0,
                "#e8dae7",
            );
            y += 71.0;
        }
        y += 22.0;
        let heading_height = theme::line_height(GALLERY_HEADING_SIZE, true);
        paint.line_text(
            bounds.x,
            y,
            heading_height,
            "Фон игры",
            TextStyle::new(GALLERY_HEADING_SIZE, theme::INK).bold(),
        );
        y += heading_height + GALLERY_HEADING_MARGIN;
        let gallery = Rect::new(bounds.x, y, bounds.w, GALLERY_HEIGHT);
        let max = (611.0 - gallery.w).max(0.0);
        let scroll = self.gallery_scroll.clamp(0.0, max);
        paint.inner_scroll = Some(ScrollArea {
            bounds: gallery,
            max,
            kind: ScrollKind::Gallery,
        });
        paint.clip(gallery);
        for (index, bg) in BACKGROUNDS.iter().enumerate() {
            let owned = bg.price == 0 || game.state.backgrounds.owned.iter().any(|id| id == bg.id);
            let selected = game.state.settings.background.as_deref().unwrap_or("ufa") == bg.id;
            let r = Rect::new(
                gallery.x + index as f32 * 124.0 - scroll,
                y + 3.0,
                112.0,
                166.0,
            );
            if r.intersection(gallery).is_none() {
                continue;
            }
            paint.styled(
                r,
                if selected {
                    theme::BACKGROUND_CARD_SELECTED
                } else {
                    theme::BACKGROUND_CARD
                },
            );
            let image_y = r.y + 8.0 + if owned { 8.5 } else { 0.0 };
            paint.commands.push(theme::background_card(
                r.x + 8.0,
                image_y,
                96.0,
                106.0,
                &format!("assets/background-{}.webp", bg.id),
                4.0,
            ));
            paint.line_text(
                r.center().x,
                image_y + 114.0,
                16.0,
                bg.title,
                TextStyle::new(14.0, theme::INK).bold().align(Align::Center),
            );
            if !owned {
                let price = bg.price.to_string();
                let w = theme::text_width(&price, 11.0, false);
                let origin = r.center().x - (w + 21.0) * 0.5;
                paint.line_text(
                    origin,
                    image_y + 133.0,
                    17.0,
                    &price,
                    TextStyle::new(11.0, theme::INK),
                );
                paint.coin(origin + w + 4.0, image_y + 133.0, 17.0);
            }
            let label = if owned {
                bg.title.to_owned()
            } else {
                format!("{} {} монет", bg.title, bg.price)
            };
            paint.control(r, &label, Action::Background(index));
        }
        paint.unclip();
        if max > 0.0 {
            let thumb_width = gallery.w * gallery.w / (gallery.w + max);
            let left = gallery.x + (gallery.w - thumb_width) * scroll / max;
            paint.rect(
                Rect::new(left, gallery.bottom(), thumb_width, GALLERY_SCROLLBAR),
                2.5,
                "#baa5c7",
            );
        }
        y = gallery.bottom() + GALLERY_SCROLLBAR + 13.0;
        let note = theme::wrap(
            "Настройки сохраняются на этом устройстве.",
            bounds.w,
            14.0,
            false,
        );
        for line in note {
            paint.line_text(
                bounds.x,
                y,
                22.390625,
                &line,
                TextStyle::new(14.0, Color::hex(0x8d7196)),
            );
            y += 22.390625;
        }
        y += 20.0;
        paint.rect(Rect::new(bounds.x, y, bounds.w, 1.0), 0.0, "#e8dae7");
        let reset = Rect::new(bounds.x, y + 19.0, bounds.w, 46.0);
        paint.styled(reset, theme::DANGER);
        paint.styled_center(
            reset,
            "Сбросить прогресс",
            TextStyle::new(16.0, Color::hex(0x963a52)).bold(),
        );
        paint.control(reset, "Сбросить прогресс", Action::Panel(Panel::Reset));
    }

    fn positions(game: &Game, word: &str) -> String {
        let Some(entry) = game.dictionary_entry(word) else {
            return String::new();
        };
        let mut items = Vec::new();
        for pos in &entry.parts_of_speech {
            let text = match pos.as_str() {
                "noun" => "существительное",
                "verb" => "глагол",
                "adj" | "adjective" => "прилагательное",
                "adv" | "adverb" => "наречие",
                "pron" | "pronoun" => "местоимение",
                "num" | "numeral" => "числительное",
                "conj" => "союз",
                "intj" => "междометие",
                "particle" => "частица",
                "prep" => "предлог",
                _ => continue,
            };
            if !items.contains(&text) {
                items.push(text);
            }
        }
        items.join(", ")
    }

    fn meaning_lines(game: &Game, word: &str, w: f32) -> Vec<Vec<String>> {
        game.meanings(word)
            .into_iter()
            .map(|m| theme::wrap(&m, (w - 30.0).max(1.0), 16.0, false))
            .collect()
    }

    fn translation_links(game: &Game, word: &str) -> Vec<String> {
        let mut links = Vec::new();
        if let Some(entry) = game.dictionary_entry(word) {
            for source in &entry.sources {
                if let Some(url) = source
                    .url
                    .as_ref()
                    .or(source.article_url.as_ref())
                    .or(source.page_url.as_ref())
                {
                    if url.starts_with("https://") && !links.contains(url) {
                        links.push(url.clone());
                    }
                    if links.len() == 3 {
                        break;
                    }
                }
            }
        }
        links
    }

    fn paint_translation(&self, game: &Game, paint: &mut Paint, bounds: Rect, word: &str) {
        let mut y = bounds.y;
        let word_height = theme::line_height(28.0, true);
        paint.line_text(
            bounds.x,
            y,
            word_height,
            word,
            TextStyle::new(28.0, Color::hex(0x8045a0))
                .bold()
                .spacing(0.56),
        );
        y += word_height + 8.0;
        let positions = Self::positions(game, word);
        if !positions.is_empty() {
            paint.line_text(
                bounds.x,
                y,
                POS_HEIGHT,
                &positions,
                TextStyle::new(14.0, Color::hex(0x8f709c)),
            );
            y += POS_HEIGHT;
        }
        let meanings = Self::meaning_lines(game, word, bounds.w);
        if meanings.is_empty() {
            y += 13.0;
            for line in theme::wrap(
                "В этой базе пока нет русского перевода",
                bounds.w,
                16.0,
                false,
            ) {
                paint.line_text(
                    bounds.x,
                    y,
                    25.59375,
                    &line,
                    TextStyle::new(16.0, theme::INK),
                );
                y += 25.59375;
            }
        } else {
            if !positions.is_empty() {
                y += POS_MARGIN;
            }
            let total = meanings
                .iter()
                .map(|lines| lines.len() as f32 * 24.0 + 4.0)
                .sum::<f32>();
            let view = Rect::new(bounds.x, y, bounds.w, total.min(144.0));
            let max = (total - view.h).max(0.0);
            paint.inner_scroll = Some(ScrollArea {
                bounds: view,
                max,
                kind: ScrollKind::Meanings,
            });
            paint.clip(view);
            let mut cursor = y - self.meanings_scroll.clamp(0.0, max);
            for (index, lines) in meanings.iter().enumerate() {
                paint.line_text(
                    bounds.x + 21.0,
                    cursor,
                    24.0,
                    &format!("{}.", index + 1),
                    TextStyle::new(16.0, theme::INK).align(Align::Right),
                );
                for line in lines {
                    paint.line_text(
                        bounds.x + 25.0,
                        cursor,
                        24.0,
                        line,
                        TextStyle::new(16.0, theme::INK),
                    );
                    cursor += 24.0;
                }
                cursor += 4.0;
            }
            paint.unclip();
            y += view.h;
        }
        if let Some(entry) = game.dictionary_entry(word) {
            y += 14.0;
            paint.rect(Rect::new(bounds.x, y, bounds.w, 1.0), 0.0, "#e7d9e7");
            y += 11.0;
            let marker = if self.sources_open {
                "M0 0H8L4 7Z"
            } else {
                "M0 0L7 3.5L0 7Z"
            };
            paint
                .commands
                .push(json!({"kind":"path","x":bounds.x+2.0,"y":y+7.0,
                "w":8.0,"h":7.0,"viewBox":[8,7],"d":marker,"fill":"#754590","stroke":0}));
            paint.line_text(
                bounds.x + 14.0,
                y,
                21.0,
                "Источники и запись словаря",
                TextStyle::new(14.0, Color::hex(0x754590)),
            );
            paint.control(
                Rect::new(bounds.x, y, bounds.w, 21.0),
                "Источники и запись словаря",
                Action::ToggleSources,
            );
            y += 21.0;
            if self.sources_open {
                y += 8.0;
                let id = format!("Запись: {}", entry.id);
                for p in ["Значения из локального словаря. Просмотр бесплатный.", &id]
                {
                    for line in theme::wrap(p, bounds.w, 12.0, false) {
                        paint.line_text(
                            bounds.x,
                            y,
                            18.0,
                            &line,
                            TextStyle::new(12.0, Color::hex(0x917799)),
                        );
                        y += 18.0;
                    }
                }
                let mut x = bounds.x;
                for (i, url) in Self::translation_links(game, word).iter().enumerate() {
                    let label = format!("Источник {}", i + 1);
                    let w = theme::text_width(&label, 13.0, false);
                    paint.line_text(
                        x,
                        y + 14.0,
                        19.5,
                        &label,
                        TextStyle::new(13.0, Color::hex(0x754590)),
                    );
                    paint.control(
                        Rect::new(x, y + 14.0, w, 19.5),
                        &label,
                        Action::OpenLink(url.clone()),
                    );
                    x += w + 9.0;
                }
            }
        }
    }

    fn paint_about(&self, game: &Game, paint: &mut Paint, bounds: Rect) {
        let layout = AboutLayout::new(bounds.w);
        let card_width = (bounds.w - 12.0) * 0.5;
        let card_height = layout.stats_height();
        let first_center = bounds.x + card_width * 0.5;
        let second_center = first_center + card_width + 12.0;
        paint.styled(
            Rect::new(bounds.x, bounds.y, card_width, card_height),
            theme::ABOUT_STATS,
        );
        paint.styled(
            Rect::new(
                bounds.x + card_width + 12.0,
                bounds.y,
                card_width,
                card_height,
            ),
            theme::ABOUT_STATS,
        );
        let digits = game.dictionary_size().to_string();
        let mut count = String::new();
        for (index, digit) in digits.chars().enumerate() {
            if index > 0 && (digits.len() - index) % 3 == 0 {
                count.push(' ');
            }
            count.push(digit);
        }
        let numeral = TextStyle::new(26.0, Color::hex(0x784393))
            .bold()
            .align(Align::Center);
        let numeral_height = theme::line_height(26.0, true);
        paint.line_text(
            first_center,
            bounds.y + 15.0,
            numeral_height,
            &count,
            numeral,
        );
        paint.line_text(second_center, bounds.y + 15.0, numeral_height, "∞", numeral);
        let labels_y = bounds.y + 15.0 + numeral_height;
        let label_height = theme::line_height(16.0, false);
        let label = TextStyle::new(13.0, theme::INK).align(Align::Center);
        paint.line_text(first_center, labels_y, label_height, "слов", label);
        for (index, line) in layout.endless.iter().enumerate() {
            paint.line_text(
                second_center,
                labels_y + index as f32 * label_height,
                label_height,
                line,
                label,
            );
        }

        let mut y = bounds.y + card_height + 18.0;
        let prose = TextStyle::new(16.0, theme::INK);
        for line in &layout.license {
            paint.line_text(bounds.x, y, ABOUT_BODY_LINE, line, prose);
            y += ABOUT_BODY_LINE;
        }
        let note_color = Color::hex(0x8d7196);
        let note = TextStyle::new(14.0, note_color);
        let underline = theme::baseline_offset(14.0, false, ABOUT_NOTE_LINE) + 3.0;
        y += 13.0;
        for (lines, label, panel) in [
            (&layout.attribution, ABOUT_ATTRIBUTION, Panel::Attribution),
            (&layout.feedback, ABOUT_FEEDBACK, Panel::Feedback),
        ] {
            if matches!(panel, Panel::Feedback) {
                y += 13.0;
                for line in &layout.review {
                    paint.line_text(bounds.x, y, ABOUT_NOTE_LINE, line, note);
                    y += ABOUT_NOTE_LINE;
                }
                y += 8.0;
            }
            let start = y;
            let mut max_width = 0.0_f32;
            for line in lines {
                let width = theme::text_width(line, 14.0, false);
                max_width = max_width.max(width);
                paint.line_text(bounds.x, y, ABOUT_NOTE_LINE, line, note);
                paint.line(
                    Point::new(bounds.x, y + underline),
                    Point::new(bounds.x + width, y + underline),
                    1.0,
                    &note_color.argb(),
                );
                y += ABOUT_NOTE_LINE;
            }
            paint.control(
                Rect::new(bounds.x, start, max_width.min(bounds.w), y - start),
                label,
                Action::Panel(panel),
            );
        }

        y += 18.0;
        paint.line_text(
            bounds.center().x,
            y,
            ABOUT_BODY_LINE,
            ABOUT_CONTACT_QUESTION,
            TextStyle::new(16.0, note_color).align(Align::Center),
        );
        y += ABOUT_BODY_LINE + 13.0;
        let links = [
            (
                "Telegram",
                "assets/contact-telegram.webp",
                "https://t.me/goinmasss",
            ),
            ("VK", "assets/contact-vk.png", "https://vk.ru/luckyanon"),
        ];
        let links_width = 32.0
            + links
                .iter()
                .map(|&(name, _, _)| 29.0 + theme::text_width(name, 14.0, false))
                .sum::<f32>();
        let mut x = bounds.center().x - links_width * 0.5;
        for (name, icon, url) in links {
            let text_width = theme::text_width(name, 14.0, false);
            let mut image = theme::image(x, y + (ABOUT_NOTE_LINE - 22.0) * 0.5, 22.0, 22.0, icon);
            image["radius"] = json!(11.0);
            paint.commands.push(image);
            paint.line_text(x + 29.0, y, ABOUT_NOTE_LINE, name, note);
            paint.line(
                Point::new(x + 29.0, y + underline),
                Point::new(x + 29.0 + text_width, y + underline),
                1.0,
                &note_color.argb(),
            );
            paint.control(
                Rect::new(x, y, 29.0 + text_width, ABOUT_NOTE_LINE),
                name,
                Action::OpenLink(url.to_string()),
            );
            x += 29.0 + text_width + 32.0;
        }
    }

    fn paint_attribution(&self, paint: &mut Paint, bounds: Rect) {
        paint.clip(bounds);
        let mut y = bounds.y - self.scroll;
        let document = include_str!("../../../public/ATTRIBUTION.md").replace("\r\n", "\n");
        let mut margin: f32 = 0.0;
        let mut first = true;
        for block in document
            .trim()
            .split("\n\n")
            .filter(|block| !block.starts_with("# "))
        {
            let section = Rect::new(bounds.x, bounds.y, bounds.w - 8.0, bounds.h);
            if let Some(heading) = block.strip_prefix("## ") {
                y += if first { 0.0 } else { margin.max(20.0) };
                for line in theme::wrap(heading, section.w, 18.0, true) {
                    paint.line_text(
                        section.x,
                        y,
                        25.2,
                        &line,
                        TextStyle::new(18.0, Color::hex(0x754590)).bold(),
                    );
                    y += 25.2;
                }
                margin = 10.0;
            } else if block.lines().all(|line| line.starts_with("- ")) {
                y += margin.max(12.0);
                for (index, line) in block.lines().enumerate() {
                    if index > 0 {
                        y += 9.0;
                    }
                    paint.line_text(
                        section.x + 7.0,
                        y,
                        24.0,
                        "•",
                        TextStyle::new(15.0, theme::INK),
                    );
                    y = license_paragraph(
                        paint,
                        Rect::new(section.x + 21.0, section.y, section.w - 21.0, section.h),
                        y,
                        &line[2..],
                    );
                }
                margin = 12.0;
            } else {
                y += margin.max(13.0);
                y = license_paragraph(paint, section, y, block);
                margin = 13.0;
            }
            first = false;
        }
        paint.unclip();
        paint.scroll = Some(ScrollArea {
            bounds,
            max: (y + margin + self.scroll - bounds.bottom()).max(0.0),
            kind: ScrollKind::Main,
        });
    }
}

fn license_paragraph(paint: &mut Paint, bounds: Rect, mut y: f32, text: &str) -> f32 {
    let mut x = bounds.x;
    let mut space = false;
    let space_width = theme::text_width(" ", 15.0, false);
    let line = 24.0;
    let link_color = Color::hex(0x754590);
    let link_height = theme::line_height(15.0, false);
    let link_top = (line - link_height) * 0.5;
    for (span, url) in markdown_spans(text) {
        let mut link_lines: Vec<Rect> = Vec::new();
        for part in span.split_inclusive(char::is_whitespace) {
            let word = part.trim_end();
            if !word.is_empty() {
                for fragment in theme::wrap(word, bounds.w, 15.0, false) {
                    let width = theme::text_width(&fragment, 15.0, false);
                    let gap = if space && x > bounds.x {
                        space_width
                    } else {
                        0.0
                    };
                    if x > bounds.x && x + gap + width > bounds.right() {
                        x = bounds.x;
                        y += line;
                    } else {
                        x += gap;
                    }
                    if y + line > bounds.y && y < bounds.bottom() {
                        let color = if url.is_some() {
                            link_color
                        } else {
                            theme::INK
                        };
                        paint.line_text(x, y, line, &fragment, TextStyle::new(15.0, color));
                        if url.is_some() {
                            let top = y + link_top;
                            if let Some(previous) =
                                link_lines.last_mut().filter(|rect| rect.y == top)
                            {
                                previous.w = x + width - previous.x;
                            } else {
                                link_lines.push(Rect::new(x, top, width, link_height));
                            }
                        }
                    }
                    x += width;
                    space = false;
                }
            }
            space |= word.len() < part.len();
        }
        if let Some(url) = url {
            let underline_offset = theme::baseline_offset(15.0, false, line) + 3.0 - link_top;
            let mut target: Option<Rect> = None;
            for row in link_lines {
                let underline = row.y + underline_offset;
                paint.line(
                    Point::new(row.x, underline),
                    Point::new(row.right(), underline),
                    1.0,
                    &link_color.argb(),
                );
                if let Some(row) = paint.visible_rect(row) {
                    target = Some(target.map_or(row, |rect| {
                        let x = rect.x.min(row.x);
                        let y = rect.y.min(row.y);
                        Rect::new(
                            x,
                            y,
                            rect.right().max(row.right()) - x,
                            rect.bottom().max(row.bottom()) - y,
                        )
                    }));
                }
            }
            if let Some(target) = target {
                paint.control(target, span, Action::OpenLink(url.to_owned()));
            }
        }
    }
    y + line
}

fn markdown_spans(text: &str) -> Vec<(&str, Option<&str>)> {
    let mut rest = text;
    let mut spans = Vec::new();
    while let Some(begin) = rest.find('[') {
        let Some((title, after)) = rest[begin + 1..].split_once("](") else {
            break;
        };
        let Some((url, suffix)) = after.split_once(')') else {
            break;
        };
        spans.push((&rest[..begin], None));
        if url.starts_with("https://") {
            spans.push((title, Some(url)));
        } else {
            spans.push((&rest[begin..rest.len() - suffix.len()], None));
        }
        rest = suffix;
    }
    spans.push((rest, None));
    spans
}

impl Ui {
    pub fn take_effects(&mut self) -> Value {
        std::mem::replace(&mut self.effect, json!({}))
    }

    pub fn event(&mut self, game: &mut Game, event: Value) {
        if event["type"] == "resize" {
            if let Some(density) = event["density"]
                .as_f64()
                .filter(|density| (0.5..=8.0).contains(density))
            {
                self.density = density as f32;
            }
        }
        theme::with_font_density(self.density, || self.event_inner(game, event));
    }

    fn event_inner(&mut self, game: &mut Game, event: Value) {
        let now = game.now_millis;
        if let Some(reduced_motion) = event["reducedMotion"].as_bool() {
            self.reduced_motion = reduced_motion;
        }
        if self.leaving.as_ref().is_some_and(|transition| {
            !self.motion(game)
                || animation::elapsed(now, transition.began)
                    >= if transition.screen {
                        animation::SCREEN_OUT
                    } else {
                        animation::DIALOG_OUT
                    }
        }) {
            let next = self.leaving.take().expect("leaving transition exists");
            self.apply_action(game, next.action);
        }
        if self.victory_after.is_some_and(|(until, _)| now >= until) {
            if let Some((_, reward)) = self.victory_after.take() {
                if self.screen == Screen::Play {
                    self.show_game(game, reward);
                }
            }
        }
        let number = |key: &str| event[key].as_f64().unwrap_or(0.0) as f32;
        let id = event["id"].as_i64().unwrap_or(0);
        let point = Point::new(number("x"), number("y"));
        if let Some(midnight) = event["nextMidnightMillis"].as_i64() {
            self.next_midnight = Some(midnight);
        }
        match event["type"].as_str().unwrap_or_default() {
            "resize" => {
                let width = number("width");
                let height = number("height");
                if width >= 80.0 && height >= 160.0 && width <= 10_000.0 && height <= 10_000.0 {
                    let viewport = self.geometry().viewport;
                    if (self.width > self.height) != (width > height) {
                        self.tilt = animation::Tilt::default();
                    }
                    let before = self.camera.zoom;
                    self.width = width;
                    self.height = height;
                    let next = self.geometry().viewport;
                    if self.screen == Screen::Play && game.is_hardcore() {
                        self.camera.zoom =
                            before.clamp((Camera::fit(game, next) * 0.7).max(0.03), 2.5);
                        let ratio = self.camera.zoom / before;
                        self.camera.offset =
                            Point::new(self.camera.offset.x * ratio, self.camera.offset.y * ratio);
                        if viewport.w != next.w || viewport.h != next.h {
                            self.camera.constrain(game, next);
                        }
                    }
                    self.cancel();
                }
            }
            "down" => self.down(game, id, point),
            "move" => self.moved(game, id, point),
            "up" => self.up(game, id, point),
            "cancel" => self.cancel(),
            "activate" => {
                if self.leaving.is_some() {
                    return;
                }
                if let Some(action) = usize::try_from(id)
                    .ok()
                    .and_then(|id| self.paint(game).controls.into_iter().nth(id))
                {
                    self.activate(game, action.action);
                }
            }
            "back" => self.back(game),
            "feedbackText" if matches!(self.panel, Some(Panel::Feedback)) => {
                if let Some(value) = event["text"].as_str() {
                    self.feedback_text = value.chars().take(4_000).collect();
                }
            }
            "scroll" => {
                let g = self.geometry();
                let delta = number("delta").clamp(-10.0, 10.0);
                if self.panel.is_none()
                    && self.screen == Screen::Play
                    && game.is_hardcore()
                    && g.viewport.contains(point)
                {
                    self.camera.scale(
                        game,
                        g.viewport,
                        (-(delta * 16.0).clamp(-300.0, 300.0) * 0.003).exp(),
                        point,
                    );
                } else {
                    let paint = self.paint(game);
                    if let Some(area) = paint
                        .inner_scroll
                        .filter(|area| area.bounds.contains(point) && area.max > 0.0)
                        .or_else(|| paint.scroll.filter(|area| area.bounds.contains(point)))
                    {
                        self.scroll_by(delta * 32.0, area);
                    }
                }
            }
            "pause" => {
                self.paused = true;
                self.cancel();
                self.idle = None;
                self.tilt = animation::Tilt::default();
            }
            "resume" => {
                self.paused = false;
                self.tilt = animation::Tilt::default();
                self.attempt(game.now_millis);
                if self.screen_enter == 0 {
                    self.screen_enter = game.now_millis;
                }
            }
            "sensor"
                if game.state.settings.background_motion && self.motion(game) && !self.paused =>
            {
                self.tilt.sample(point, number("angle"), now);
                self.tilt.tick(now);
            }
            "tick" => {
                self.tilt.tick(now);
                self.tick_idle(game);
            }
            _ => {}
        }
        if self.toast.as_ref().is_some_and(|item| {
            item.until + if self.motion(game) { 200 } else { 0 } <= game.now_millis
        }) {
            self.toast = None;
        }
        if self
            .repeat
            .as_ref()
            .is_some_and(|item| item.until <= game.now_millis)
        {
            self.repeat = None;
        }
        if self.gift_flight.as_ref().is_some_and(|item| {
            animation::elapsed(now, item.began) >= animation::GIFT_FLIGHT + animation::GIFT_BUMP
        }) {
            self.gift_flight = None;
        }
        if self.invalid.as_ref().is_some_and(|item| item.until <= now) {
            self.invalid = None;
        }
        if self
            .shuffle
            .as_ref()
            .is_some_and(|item| now.saturating_sub(item.began) >= animation::TILE_LOCKOUT_MS)
        {
            self.shuffle = None;
        }
        self.cell_effects.retain(|effect| {
            now.saturating_sub(effect.began) < if effect.found { 690 } else { 195 }
        });
    }

    fn down(&mut self, game: &mut Game, id: i64, point: Point) {
        if self.leaving.is_some() {
            return;
        }
        if let Some(Gesture::Board(pointers)) = self.gesture.as_mut() {
            if pointers.len() < 2 && pointers.iter().all(|p| p.id != id) {
                pointers.push(Pointer { id, at: point });
            }
            return;
        }
        if self.gesture.is_some() {
            return;
        }
        let game_visible = self.panel.is_none() && self.screen == Screen::Play;
        if game_visible && self.shuffle.is_none() {
            if let Some(index) = self.tile_at(game, point) {
                self.selection.clear();
                self.pick(game, index);
                self.gesture = Some(Gesture::Wheel { id, point });
                return;
            }
        }
        let paint = self.paint(game);
        let scroll = paint
            .inner_scroll
            .filter(|area| area.bounds.contains(point) && area.max > 0.0)
            .or_else(|| paint.scroll.filter(|area| area.bounds.contains(point)));
        let control = paint
            .controls
            .into_iter()
            .rev()
            .find(|item| item.rect.contains(point));
        if let Some(control) = control {
            self.press_began = game.now_millis;
            self.released_press = None;
            self.gesture = Some(Gesture::Press {
                id,
                start: point,
                current: point,
                action: control.action,
                rect: control.rect,
                scrolled: false,
                scroll,
            });
        } else if game_visible
            && game.is_hardcore()
            && self.geometry().viewport.contains(point)
            && !self.geometry().wheel.contains(point)
        {
            self.gesture = Some(Gesture::Board(vec![Pointer { id, at: point }]));
            self.attempt(game.now_millis);
        } else if let Some(area) = scroll {
            self.gesture = Some(Gesture::Press {
                id,
                start: point,
                current: point,
                action: Action::Nothing,
                rect: area.bounds,
                scrolled: false,
                scroll: Some(area),
            });
        } else if self
            .panel
            .as_ref()
            .is_some_and(|panel| !self.panel_rect(game, panel).contains(point))
        {
            self.gesture = Some(Gesture::Press {
                id,
                start: point,
                current: point,
                action: Action::Close,
                rect: Rect::new(0.0, 0.0, self.width, self.height),
                scrolled: false,
                scroll: None,
            });
        }
    }

    fn moved(&mut self, game: &Game, id: i64, point: Point) {
        let Some(mut gesture) = self.gesture.take() else {
            return;
        };
        match &mut gesture {
            Gesture::Wheel {
                id: pointer,
                point: previous,
            } if *pointer == id => {
                self.trace(game, *previous, point);
                *previous = point;
            }
            Gesture::Press {
                id: pointer,
                start,
                current,
                scrolled,
                scroll,
                ..
            } if *pointer == id => {
                if *scrolled || start.distance(point) > 7.0 {
                    if !*scrolled
                        && scroll.is_some_and(|area| matches!(area.kind, ScrollKind::Gallery))
                        && (point.y - start.y).abs() > (point.x - start.x).abs()
                    {
                        *scroll = self.paint(game).scroll;
                    }
                    if let Some(area) = scroll {
                        *scrolled = true;
                        let delta = if matches!(area.kind, ScrollKind::Gallery) {
                            current.x - point.x
                        } else {
                            current.y - point.y
                        };
                        self.scroll_by(delta, *area);
                    }
                }
                *current = point;
            }
            Gesture::Board(pointers) => self.move_camera(game, pointers, id, point),
            _ => {}
        }
        self.gesture = Some(gesture);
    }

    fn up(&mut self, game: &mut Game, id: i64, point: Point) {
        let Some(gesture) = self.gesture.take() else {
            return;
        };
        match gesture {
            Gesture::Wheel {
                id: pointer,
                point: previous,
            } if pointer == id => {
                self.trace(game, previous, point);
                self.submit_selection(game);
            }
            Gesture::Press {
                id: pointer,
                start,
                rect,
                action,
                scrolled,
                ..
            } if pointer == id => {
                if !scrolled && button_action(&action) && self.motion(game) {
                    self.released_press = Some(ReleasedPress {
                        rect,
                        scale: animation::press_scale(animation::elapsed(
                            game.now_millis,
                            self.press_began,
                        )),
                        began: game.now_millis,
                    });
                }
                if !scrolled && rect.contains(point) && start.distance(point) <= 17.0 {
                    self.activate(game, action);
                }
            }
            Gesture::Board(mut pointers) => {
                pointers.retain(|p| p.id != id);
                if !pointers.is_empty() {
                    self.gesture = Some(Gesture::Board(pointers));
                }
            }
            other => self.gesture = Some(other),
        }
    }

    fn move_camera(&mut self, game: &Game, pointers: &mut [Pointer], id: i64, now: Point) {
        let Some(index) = pointers.iter().position(|p| p.id == id) else {
            return;
        };
        let old = pointers[index].at;
        pointers[index].at = now;
        let board = self.geometry().viewport;
        if pointers.len() == 2 {
            let other = pointers[1 - index].at;
            let old_center = old.midpoint(other);
            let new_center = now.midpoint(other);
            let factor =
                (now.distance(other).max(1.0) / old.distance(other).max(1.0)).clamp(0.25, 4.0);
            self.camera.scale(game, board, factor, old_center);
            self.camera.pan(
                game,
                board,
                new_center.x - old_center.x,
                new_center.y - old_center.y,
            );
        } else {
            self.camera.pan(game, board, now.x - old.x, now.y - old.y);
        }
    }

    fn tile_at(&self, game: &Game, point: Point) -> Option<usize> {
        let g = self.geometry();
        let radius = g.tile_radius(game.rack.len()) * 1.15;
        closest_tile(&g.tiles(game.rack.len()), point, radius)
    }

    fn trace(&mut self, game: &Game, from: Point, to: Point) {
        let g = self.geometry();
        let tiles = g.tiles(game.rack.len());
        let radius = g.tile_radius(game.rack.len()) * 1.15;
        let steps = (from.distance(to) / 8.0).ceil().clamp(1.0, 300.0) as usize;
        for step in 1..=steps {
            let t = step as f32 / steps as f32;
            let point = Point::new(from.x + t * (to.x - from.x), from.y + t * (to.y - from.y));
            if let Some(index) = closest_tile(&tiles, point, radius) {
                self.pick(game, index);
            }
        }
    }

    fn pick(&mut self, game: &Game, index: usize) {
        self.invalid = None;
        let count = self.selection.len();
        if count >= 2 && self.selection[count - 2] == index {
            self.selection.pop();
            self.attempt(game.now_millis);
        } else if !self.selection.contains(&index) && index < game.rack.len() {
            self.selection.push(index);
            self.attempt(game.now_millis);
            if game.state.settings.sound {
                self.effect["sound"] = json!("select");
            }
        }
    }

    fn selected_word(&self, game: &Game) -> String {
        self.selection
            .iter()
            .filter_map(|index| game.rack.get(*index))
            .collect()
    }

    fn submit_selection(&mut self, game: &mut Game) {
        if self.selection.len() >= 2 {
            let word = self.selected_word(game);
            let solved = game.progress().solved.clone();
            let result = game.submit(&word);
            if result.kind == "target" {
                self.cell_changes(game, &solved, None);
            }
            self.feedback(game, result);
        }
        self.selection.clear();
    }

    fn feedback(&mut self, game: &Game, result: ActionResult) {
        if result.completed && (result.kind == "target" || result.kind == "hint") {
            let when = game.now_millis + if self.motion(game) { 750 } else { 263 };
            self.victory_after = Some((when, result.reward));
        } else {
            match result.kind {
                "bonus" => self.message(game, format!("Бонусное слово: {}", result.word)),
                "repeat" => {
                    if game.progress().bonus.contains(&result.word) {
                        self.message(game, format!("Бонусное слово: {}", result.word));
                    } else {
                        self.focus_word(game, &result.word);
                        self.repeat = Some(TimedText {
                            text: result.word,
                            until: game.now_millis + 1000,
                        });
                    }
                }
                "insufficient" => self.message(game, "Недостаточно монет"),
                "excluded-length" => {
                    self.toast_began = game.now_millis;
                    self.toast = Some(TimedText {
                        text: format!(
                            "На данном уровне нет слов из {}х букв",
                            result.word.chars().count()
                        ),
                        until: game.now_millis + 1800,
                    });
                    self.invalid = Some(TimedText {
                        text: result.word,
                        until: game.now_millis + 420,
                    });
                }
                "invalid" | "unbuildable" | "outside" => {
                    self.invalid = Some(TimedText {
                        text: result.word,
                        until: game.now_millis + 420,
                    });
                }
                _ => {}
            }
        }
    }

    fn cell_changes(&mut self, game: &Game, before: &[String], hinted: Option<&str>) {
        let newly_solved: Vec<_> = game
            .progress()
            .solved
            .iter()
            .filter(|word| !before.contains(*word))
            .collect();
        let mut changed = Vec::new();
        for cell in game.cells() {
            let found = cell.words.iter().any(|word| newly_solved.contains(&word));
            if found || hinted.is_some_and(|key| cell.key == key) {
                self.cell_effects.retain(|effect| effect.key != cell.key);
                self.cell_effects.push(CellEffect {
                    key: cell.key.clone(),
                    began: game.now_millis,
                    found,
                });
                changed.push((cell.row, cell.col));
            }
        }
        self.focus_cells(game, &changed);
    }

    fn focus_word(&mut self, game: &Game, word: &str) {
        if !game.is_hardcore() {
            return;
        }
        let Some(item) = game.level().words.iter().find(|item| item.word == word) else {
            return;
        };
        let cells: Vec<_> = item.cells().map(|(row, col, _)| (row, col)).collect();
        self.focus_cells(game, &cells);
    }

    fn focus_cells(&mut self, game: &Game, cells: &[(i32, i32)]) {
        if !game.is_hardcore() || cells.is_empty() {
            return;
        }
        let g = self.geometry();
        let size = self.camera.cell(game, g.viewport);
        let step = size + self.camera.gap(game);
        let origin = self.camera.origin(game, g.viewport);
        let (mut x0, mut y0, mut x1, mut y1) = (
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        );
        for &(row, col) in cells {
            let x = origin.x + col as f32 * step;
            let y = origin.y + row as f32 * step;
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x + size);
            y1 = y1.max(y + size);
        }
        let region = g.hardcore_focus.inset(12.0);
        if x0 < region.x || y0 < region.y || x1 > region.right() || y1 > region.bottom() {
            let center = g.hardcore_focus.center();
            self.camera.pan(
                game,
                g.viewport,
                center.x - (x0 + x1) * 0.5,
                center.y - (y0 + y1) * 0.5,
            );
        }
    }

    fn message(&mut self, game: &Game, text: impl Into<String>) {
        self.toast_began = game.now_millis;
        self.toast = Some(TimedText {
            text: text.into(),
            until: game.now_millis + 2_400,
        });
    }

    fn scroll_by(&mut self, delta: f32, area: ScrollArea) {
        let target = if matches!(area.kind, ScrollKind::Gallery) {
            &mut self.gallery_scroll
        } else if matches!(area.kind, ScrollKind::Meanings) {
            &mut self.meanings_scroll
        } else if matches!(self.panel, Some(Panel::Found)) {
            &mut self.found_scroll
        } else if self.panel.is_some() {
            &mut self.scroll
        } else if self.screen == Screen::Menu {
            &mut self.menu_scroll
        } else {
            &mut self.victory_scroll
        };
        *target = (target.clamp(0.0, area.max) + delta).clamp(0.0, area.max);
    }

    fn attempt(&mut self, now: i64) {
        self.started = now;
        self.idle = None;
    }

    fn tick_idle(&mut self, game: &Game) {
        if self.screen != Screen::Play
            || self.panel.is_some()
            || self.paused
            || game.is_hardcore()
            || !self.selection.is_empty()
        {
            return;
        }
        if self
            .idle
            .as_ref()
            .is_some_and(|item| item.until > game.now_millis)
        {
            return;
        }
        self.idle = None;
        let delay = if self.idle_letters_next {
            60_000
        } else {
            30_000
        };
        if game.now_millis.saturating_sub(self.started) < delay {
            return;
        }
        let solved = &game.progress().solved;
        let remaining: Vec<_> = game
            .level()
            .words
            .iter()
            .filter(|entry| !solved.contains(&entry.word) && !game.meanings(&entry.word).is_empty())
            .collect();
        if !remaining.is_empty() {
            let index = game.now_millis.unsigned_abs() as usize % remaining.len();
            self.idle = Some(Idle {
                word: remaining[index].word.clone(),
                marks_tiles: self.idle_letters_next,
                until: game.now_millis + 7_000,
            });
            self.idle_letters_next = !self.idle_letters_next;
            self.started = game.now_millis - if self.idle_letters_next { 30_000 } else { 0 };
        }
    }

    fn show_game(&mut self, game: &Game, reward: u64) {
        self.panel = None;
        self.reward = reward;
        self.screen = if game.progress().completed {
            Screen::Victory
        } else {
            Screen::Play
        };
        self.victory_scroll = f32::MAX;
        let zoom = if game.is_hardcore() {
            Camera::fit(game, self.geometry().viewport).max(0.70)
        } else {
            1.0
        };
        self.camera = Camera {
            zoom,
            ..Camera::default()
        };
        let g = self.geometry();
        if game.is_hardcore() && !g.hardcore_focus.inset(12.0).contains(g.viewport.center()) {
            let focus = g.hardcore_focus.center();
            let center = g.viewport.center();
            self.camera
                .pan(game, g.viewport, focus.x - center.x, focus.y - center.y);
        }
        self.cancel();
        self.toast = None;
        self.repeat = None;
        self.invalid = None;
        self.shuffle = None;
        self.cell_effects.clear();
        self.victory_after = None;
        self.screen_enter = game.now_millis;
        self.idle_letters_next = false;
        self.attempt(game.now_millis);
    }

    fn show_panel(&mut self, game: &Game, panel: Panel) {
        if matches!(self.panel, Some(Panel::Feedback)) {
            self.effect["closeFeedbackEditor"] = json!(true);
        }
        if matches!(panel, Panel::Feedback) {
            self.feedback_text.clear();
        }
        if matches!(panel, Panel::Settings) {
            self.gallery_scroll = 0.0;
        }
        if matches!(panel, Panel::Attribution) {
            self.attribution_parent_scroll = self.scroll;
        }
        self.scroll = 0.0;
        if matches!(panel, Panel::Translation { .. }) {
            self.sources_open = false;
            self.meanings_scroll = 0.0;
        }
        if matches!(panel, Panel::Found) {
            self.found_scroll = f32::MAX;
        }
        self.cancel();
        self.attempt(game.now_millis);
        self.panel = Some(panel);
        self.panel_enter = game.now_millis;
    }

    fn close_panel(&mut self, game: &Game) {
        self.cancel();
        self.scroll = 0.0;
        self.panel_enter = game.now_millis;
        self.attempt(game.now_millis);
        self.panel = match self.panel.take() {
            Some(Panel::Translation {
                from_found: true, ..
            }) => Some(Panel::Found),
            Some(Panel::Attribution) => {
                self.scroll = self.attribution_parent_scroll;
                self.panel_enter = 0;
                Some(Panel::About)
            }
            Some(Panel::Feedback) => {
                self.effect["closeFeedbackEditor"] = json!(true);
                None
            }
            _ => None,
        };
    }

    fn cancel(&mut self) {
        self.selection.clear();
        self.gesture = None;
    }

    fn back(&mut self, game: &mut Game) {
        if self.panel.is_some() {
            self.activate(game, Action::Close);
        } else if !self.selection.is_empty() {
            self.cancel();
        } else if self.screen != Screen::Menu {
            self.activate(game, Action::Navigate(Screen::Menu));
        } else {
            self.effect["exit"] = json!(true);
        }
    }

    fn activate(&mut self, game: &mut Game, action: Action) {
        if self.leaving.is_some() {
            return;
        }
        let screen = matches!(
            action,
            Action::Navigate(_)
                | Action::Mode(_)
                | Action::Next
                | Action::Regenerate
                | Action::Reset
        );
        let modal = self.panel.is_some()
            && matches!(action, Action::Close | Action::Panel(_))
            && !matches!(action, Action::Panel(Panel::Attribution));
        if self.motion(game) && (screen || modal) {
            self.cancel();
            self.leaving = Some(Transition {
                began: game.now_millis,
                action,
                screen,
            });
        } else {
            self.apply_action(game, action);
        }
    }

    fn apply_action(&mut self, game: &mut Game, action: Action) {
        self.attempt(game.now_millis);
        match action {
            Action::Navigate(screen) => {
                self.screen = screen;
                self.panel = None;
                self.cancel();
                self.victory_after = None;
                self.screen_enter = game.now_millis;
                if screen == Screen::Menu {
                    self.menu_scroll = 0.0;
                }
            }
            Action::Mode(mode) => {
                if game.set_mode(mode) {
                    self.show_game(game, 0);
                }
            }
            Action::Panel(panel) => {
                self.show_panel(game, panel);
            }
            Action::Close => self.close_panel(game),
            Action::Claim => {
                let button = self.menu_button_bounds(2);
                let right = if self.width <= 360.0 { 13.0 } else { 17.0 };
                let origin = Point::new(button.right() - right - 12.5, button.center().y);
                if game.claim_gift(game.local_day) {
                    if game.state.settings.animations {
                        self.gift_flight = Some(GiftFlight {
                            began: game.now_millis,
                            origin,
                        });
                    }
                    self.message(game, "Подарок получен: +100 монет");
                }
            }
            Action::Shuffle => {
                let old = game.rack.clone();
                game.shuffle();
                self.cancel();
                self.invalid = None;
                if self.motion(game) {
                    let mut used = HashSet::new();
                    let previous = game
                        .rack
                        .iter()
                        .map(|letter| {
                            old.iter()
                                .enumerate()
                                .find(|(index, candidate)| {
                                    *candidate == letter && used.insert(*index)
                                })
                                .map_or(0, |(index, _)| index)
                        })
                        .collect();
                    self.shuffle = Some(Shuffling {
                        began: game.now_millis,
                        previous,
                    });
                }
            }
            Action::Hint => {
                self.cancel();
                let solved = game.progress().solved.clone();
                let before = game.progress().revealed.clone();
                let result = game.hint();
                if result.kind == "hint" {
                    let hinted = game
                        .progress()
                        .revealed
                        .iter()
                        .find(|key| !before.contains(key))
                        .cloned();
                    self.cell_changes(game, &solved, hinted.as_deref());
                }
                self.feedback(game, result);
            }
            Action::Next => {
                if game.next_level() {
                    self.show_game(game, 0);
                }
            }
            Action::Regenerate => {
                if game.regenerate_hardcore() {
                    self.show_game(game, 0);
                }
            }
            Action::Reset => {
                game.reset();
                self.panel = None;
                self.screen = Screen::Menu;
                self.cancel();
                self.menu_scroll = 0.0;
                self.screen_enter = game.now_millis;
                self.victory_after = None;
                self.message(game, "Прогресс сброшен");
            }
            Action::Background(index) => {
                let background = &BACKGROUNDS[index];
                if game
                    .state
                    .backgrounds
                    .owned
                    .iter()
                    .any(|id| id == background.id)
                    || background.price == 0
                {
                    game.select_background(background.id);
                } else {
                    self.activate(game, Action::Panel(Panel::Purchase(index)));
                }
            }
            Action::Buy(index) => {
                let result = game.purchase_background(BACKGROUNDS[index].id);
                if result.kind == "purchased" || result.kind == "owned" {
                    game.select_background(BACKGROUNDS[index].id);
                    self.activate(game, Action::Panel(Panel::Settings));
                } else {
                    self.feedback(game, result);
                }
            }
            Action::Toggle(setting) => {
                let value = match setting {
                    Setting::Sound => &mut game.state.settings.sound,
                    Setting::Music => &mut game.state.settings.music,
                    Setting::Animations => &mut game.state.settings.animations,
                    Setting::Tilt => &mut game.state.settings.background_motion,
                };
                *value = !*value;
                if matches!(setting, Setting::Tilt | Setting::Animations) {
                    self.tilt = animation::Tilt::default();
                }
                game.mark_dirty();
            }
            Action::Select(index) => self.pick(game, index),
            Action::Submit => self.submit_selection(game),
            Action::EditFeedback => self.effect["editFeedback"] = json!(self.feedback_text),
            Action::SubmitFeedback => {
                if self.feedback_text.is_empty() {
                    self.effect["editFeedback"] = json!(self.feedback_text);
                } else {
                    self.effect["closeFeedbackEditor"] = json!(true);
                    self.message(game, "Отправка пока недоступна. Текст не отправлен.");
                }
            }
            Action::ToggleSources => self.sources_open = !self.sources_open,
            Action::OpenLink(link) => {
                if link.starts_with("https://") {
                    self.effect["openLink"] = json!(link);
                }
            }
            Action::Nothing => {}
        }
    }
}

fn closest_tile(tiles: &[Point], pointer: Point, radius: f32) -> Option<usize> {
    tiles
        .iter()
        .enumerate()
        .map(|(index, tile)| (index, tile.distance(pointer)))
        .filter(|(_, distance)| *distance <= radius)
        .min_by(|(_, left), (_, right)| left.total_cmp(right))
        .map(|(index, _)| index)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, OnceLock};

    use crate::puzzles::PuzzleBook;

    use super::*;

    fn fixture() -> (Ui, Game) {
        static BOOK: OnceLock<Arc<PuzzleBook>> = OnceLock::new();
        let book = BOOK.get_or_init(|| {
            Arc::new(
                PuzzleBook::from_json(
                    include_str!("../../../public/data/game-lexicon.nouns.json"),
                    include_str!("../../../public/data/nouns.full.json"),
                    include_str!("../../../public/data/level-pools.json"),
                )
                .expect("production puzzle data"),
            )
        });
        (
            Ui::default(),
            Game::new(Arc::clone(book), "", 50_000, 0).expect("fresh campaign"),
        )
    }

    fn click(ui: &mut Ui, game: &mut Game, label: &str) {
        let control = ui
            .paint(game)
            .controls
            .into_iter()
            .find(|item| item.label == label)
            .unwrap_or_else(|| panic!("missing visible control: {label}"));
        let point = control.rect.center();
        ui.event(game, json!({"type":"down","id":0,"x":point.x,"y":point.y}));
        ui.event(game, json!({"type":"up","id":0,"x":point.x,"y":point.y}));
        finish_transition(ui, game);
    }

    fn finish_transition(ui: &mut Ui, game: &mut Game) {
        if let Some(leaving) = &ui.leaving {
            let duration = if leaving.screen {
                animation::SCREEN_OUT
            } else {
                animation::DIALOG_OUT
            };
            game.now_millis = leaving.began + duration.ceil() as i64;
            ui.event(game, json!({"type":"tick"}));
        }
    }

    #[test]
    fn native_controls_enter_a_word_and_reopen_its_translation() {
        let (mut ui, mut game) = fixture();
        click(&mut ui, &mut game, "Классика");
        let word = game.level().words[0].word.clone();
        let mut used = HashSet::new();
        for wanted in word.chars() {
            let index = game
                .rack
                .iter()
                .enumerate()
                .find(|(i, letter)| **letter == wanted && !used.contains(i))
                .map(|(i, _)| i)
                .unwrap();
            used.insert(index);
            let label = format!("{wanted}; буква {}", index + 1);
            let id = ui
                .paint(&game)
                .controls
                .iter()
                .position(|control| control.label == label)
                .unwrap();
            ui.event(&mut game, json!({"type":"activate","id":id}));
        }
        click(&mut ui, &mut game, "Проверить слово");
        assert!(game.progress().solved.contains(&word));
        click(&mut ui, &mut game, "Найденные слова");
        click(&mut ui, &mut game, &format!("Перевод: {word}"));
        assert!(matches!(ui.panel, Some(Panel::Translation { .. })));
        assert!(
            ui.frame(&game)["commands"]
                .as_array()
                .unwrap()
                .iter()
                .any(|command| command["text"] == word)
        );
        ui.event(&mut game, json!({"type":"back"}));
        finish_transition(&mut ui, &mut game);
        assert!(matches!(ui.panel, Some(Panel::Found)));
    }

    #[test]
    fn hardcore_pinch_regeneration_and_gift_operate_through_native_events() {
        let (mut ui, mut game) = fixture();
        click(&mut ui, &mut game, "Подарок");
        assert_eq!(game.state.coins, 190);
        assert_eq!(ui.frame(&game)["animate"], true);
        game.now_millis += 1_000;
        ui.event(&mut game, json!({"type":"tick"}));
        assert_eq!(ui.frame(&game)["animate"], false);
        click(&mut ui, &mut game, "Хардкор");
        let id = game.level().id;
        let board = ui.geometry().hardcore_focus.center();
        let zoom = ui.camera.zoom;
        ui.event(
            &mut game,
            json!({"type":"down","id":1,"x":board.x-20.0,"y":board.y}),
        );
        ui.event(
            &mut game,
            json!({"type":"down","id":2,"x":board.x+20.0,"y":board.y}),
        );
        ui.event(
            &mut game,
            json!({"type":"move","id":2,"x":board.x+59.0,"y":board.y}),
        );
        ui.event(&mut game, json!({"type":"cancel","id":1}));
        assert!(ui.camera.zoom > zoom);
        click(&mut ui, &mut game, "Перегенерировать уровень");
        click(&mut ui, &mut game, "Пересобрать");
        assert!(game.level().id > id);
        assert_eq!(game.state.coins, 190);
    }
}
