use std::collections::HashSet;
use std::f32::consts::TAU;

use serde_json::{Value, json};

use crate::game::{ActionResult, Game, Mode};

const WHITE: &str = "#FFF6EF";
const INK: &str = "#533763";
const PURPLE: &str = "#8045A0";
const PAPER: &str = "#FFF9F6";
const SECONDARY: &str = "#EADDEB";
const TRANSLUCENT: &str = "#47322640";
const LINE: &str = "#7AFFFFFF";
const GOLD: &str = "#FFDA8E";

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

const HELP: [&str; 7] = [
    "Соединяйте буквы пальцем и отпустите, чтобы проверить слово. Одну кнопку можно использовать один раз. Одинаковые буквы на разных кнопках доступны отдельно.",
    "Вернитесь к предыдущей букве, чтобы убрать последнюю. Прерывание касания отменяет ввод.",
    "На классическом уровне от 8 до 12 слов на поле. Остальные подходящие слова могут принести по 5 монет как бонусы.",
    "Перемешивание бесплатно. Подсказка стоит 20 монет. За первое прохождение классического уровня начисляется 60 монет.",
    "В хардкоре передвигайте поле пальцем и меняйте масштаб двумя пальцами. Подсказки в этом режиме недоступны.",
    "Перевод доступен в списке найденных слов. А и Ә, О и Ө, У и Ү, К и Ҡ — разные буквы.",
    "После победы открывается следующий уровень. Игра сохраняет прогресс автоматически.",
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

#[derive(Clone, Copy, Debug, Default)]
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
    Help,
    Attribution,
    Levels,
    Rating,
    Feedback,
    Reset,
    Regenerate,
    Replay(u64),
    Purchase(usize),
}

impl Panel {
    fn title(&self) -> &'static str {
        match self {
            Self::Found => "Найденные слова",
            Self::Translation { .. } => "Перевод и значения",
            Self::Settings => "Настройки",
            Self::About => "Об игре",
            Self::Help => "Как играть",
            Self::Attribution => "Источники и лицензия",
            Self::Levels => "Уровни",
            Self::Rating => "Рейтинг",
            Self::Feedback => "Обратная связь",
            Self::Reset => "Сбросить прогресс?",
            Self::Regenerate => "Пересобрать уровень?",
            Self::Replay(_) => "Решить ещё раз?",
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
    Level(u64),
    LevelPage(u64),
    Replay(u64),
    Background(usize),
    Buy(usize),
    Toggle(Setting),
    Select(usize),
    Submit,
    Zoom(f32),
    OpenLink(String),
    Nothing,
}

struct Control {
    rect: Rect,
    label: String,
    action: Action,
}

#[derive(Clone, Copy)]
struct ScrollArea {
    bounds: Rect,
    max: f32,
}

#[derive(Default)]
struct Paint {
    commands: Vec<Value>,
    controls: Vec<Control>,
    clips: Vec<Rect>,
    scroll: Option<ScrollArea>,
}

impl Paint {
    fn rect(&mut self, r: Rect, radius: f32, color: &str) {
        self.commands.push(
            json!({"kind":"rect","x":r.x,"y":r.y,"w":r.w,"h":r.h,"radius":radius,"color":color}),
        );
    }

    fn outline(&mut self, r: Rect, radius: f32, stroke: f32, color: &str) {
        self.commands.push(json!({"kind":"stroke_rect","x":r.x,"y":r.y,"w":r.w,"h":r.h,"radius":radius,"stroke":stroke,"color":color}));
    }

    fn circle(&mut self, p: Point, radius: f32, color: &str) {
        self.commands
            .push(json!({"kind":"circle","x":p.x,"y":p.y,"r":radius,"color":color}));
    }

    fn line(&mut self, a: Point, b: Point, stroke: f32, color: &str) {
        self.commands.push(json!({"kind":"line","x1":a.x,"y1":a.y,"x2":b.x,"y2":b.y,"stroke":stroke,"color":color}));
    }

    fn text(
        &mut self,
        p: Point,
        text: impl AsRef<str>,
        size: f32,
        bold: bool,
        align: &str,
        color: &str,
    ) {
        self.commands.push(json!({"kind":"text","x":p.x,"y":p.y,"text":text.as_ref(),"size":size,"bold":bold,"align":align,"color":color}));
    }

    fn center_text(&mut self, r: Rect, label: &str, size: f32, color: &str) {
        let p = r.center();
        self.text(
            Point::new(p.x, p.y + size * 0.34),
            label,
            size,
            true,
            "center",
            color,
        );
    }

    fn image(&mut self, path: &str, r: Rect, alpha: f32) {
        self.commands.push(json!({"kind":"image","x":r.x,"y":r.y,"w":r.w,"h":r.h,"path":path,"fit":"cover","alpha":alpha}));
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

    fn control(&mut self, mut rect: Rect, label: &str, action: Action) {
        for clip in &self.clips {
            let Some(visible) = rect.intersection(*clip) else {
                return;
            };
            rect = visible;
        }
        self.controls.push(Control {
            rect,
            label: label.to_string(),
            action,
        });
    }

    fn button(&mut self, r: Rect, label: &str, action: Action, style: Button) {
        let (background, color, border) = match style {
            Button::Primary => (PAPER, "#673D81", None),
            Button::Secondary => (TRANSLUCENT, WHITE, Some(LINE)),
            Button::Paper => (SECONDARY, INK, None),
            Button::Accent => ("#8254A4", WHITE, None),
            Button::Danger => ("#F4DCE1", "#963A52", None),
            Button::Disabled => ("#372D2539", "#DBC8DC", Some("#3FFFFFFF")),
        };
        self.rect(r, 11.0, background);
        if let Some(border) = border {
            self.outline(r, 11.0, 1.0, border);
        }
        let font_size = fitting_size(label, r.w - 12.0, 16.0);
        self.center_text(r, label, font_size, color);
        if !matches!(style, Button::Disabled) {
            self.control(r, label, action);
        }
    }

    fn icon_button(
        &mut self,
        r: Rect,
        kind: Icon,
        label: &str,
        action: Action,
        light: bool,
        enabled: bool,
    ) {
        self.rect(r, 10.0, if light { PAPER } else { TRANSLUCENT });
        self.outline(r, 10.0, 1.0, if light { "#60FFFDF6" } else { LINE });
        let color = if !enabled {
            "#AD96B3"
        } else if light {
            PURPLE
        } else {
            WHITE
        };
        self.icon(r.center(), kind, r.w.min(r.h) * 0.48, color);
        if enabled {
            self.control(r, label, action);
        }
    }

    fn icon(&mut self, p: Point, icon: Icon, size: f32, color: &str) {
        let transform = |a: (f32, f32)| Point::new(p.x + size * a.0, p.y + size * a.1);
        for &(a, b) in icon.segments() {
            self.line(transform(a), transform(b), (size * 0.083).max(1.2), color);
        }
    }

    fn paragraph(
        &mut self,
        x: f32,
        mut y: f32,
        width: f32,
        value: &str,
        size: f32,
        color: &str,
    ) -> f32 {
        for line in wrap(value, width, size) {
            self.text(Point::new(x, y + size), line, size, false, "left", color);
            y += size * 1.42;
        }
        y
    }
}

#[derive(Clone, Copy)]
enum Button {
    Primary,
    Secondary,
    Paper,
    Accent,
    Danger,
    Disabled,
}

#[derive(Clone, Copy)]
enum Icon {
    Menu,
    Bulb,
    Shuffle,
    Repeat,
    Close,
}

type Segment = ((f32, f32), (f32, f32));

impl Icon {
    fn segments(self) -> &'static [Segment] {
        match self {
            Self::Menu => &[
                ((-0.43, -0.3), (0.43, -0.3)),
                ((-0.43, 0.0), (0.43, 0.0)),
                ((-0.43, 0.3), (0.43, 0.3)),
            ],
            Self::Bulb => &[
                ((-0.18, 0.29), (0.18, 0.29)),
                ((-0.13, 0.44), (0.13, 0.44)),
                ((-0.18, 0.29), (-0.36, -0.04)),
                ((-0.36, -0.04), (-0.27, -0.35)),
                ((-0.27, -0.35), (0.0, -0.45)),
                ((0.0, -0.45), (0.27, -0.35)),
                ((0.27, -0.35), (0.36, -0.04)),
                ((0.36, -0.04), (0.18, 0.29)),
            ],
            Self::Shuffle => &[
                ((-0.47, -0.25), (-0.23, -0.25)),
                ((-0.23, -0.25), (0.23, 0.25)),
                ((0.23, 0.25), (0.46, 0.25)),
                ((0.32, 0.11), (0.46, 0.25)),
                ((0.32, 0.39), (0.46, 0.25)),
                ((-0.47, 0.25), (-0.23, 0.25)),
                ((-0.23, 0.25), (-0.08, 0.09)),
                ((0.08, -0.09), (0.23, -0.25)),
                ((0.23, -0.25), (0.46, -0.25)),
                ((0.32, -0.39), (0.46, -0.25)),
                ((0.32, -0.11), (0.46, -0.25)),
            ],
            Self::Repeat => &[
                ((-0.4, -0.08), (-0.18, -0.36)),
                ((-0.18, -0.36), (0.18, -0.36)),
                ((0.18, -0.36), (0.42, -0.08)),
                ((0.19, -0.1), (0.42, -0.08)),
                ((0.42, -0.33), (0.42, -0.08)),
                ((0.4, 0.09), (0.18, 0.37)),
                ((0.18, 0.37), (-0.18, 0.37)),
                ((-0.18, 0.37), (-0.42, 0.09)),
                ((-0.19, 0.11), (-0.42, 0.09)),
                ((-0.42, 0.33), (-0.42, 0.09)),
            ],
            Self::Close => &[
                ((-0.34, -0.34), (0.34, 0.34)),
                ((-0.34, 0.34), (0.34, -0.34)),
            ],
        }
    }
}

fn char_width(c: char) -> f32 {
    match c {
        ' ' | '\t' => 0.32,
        'i' | 'l' | 'I' | '.' | ',' | ':' | '!' | '|' => 0.31,
        'Ш' | 'Щ' | 'Ж' | 'М' | 'Ф' | 'Ы' | 'Ю' | 'W' | 'M' => 0.85,
        c if c.is_uppercase() => 0.68,
        _ => 0.57,
    }
}

fn text_width(value: &str) -> f32 {
    value.chars().map(char_width).sum()
}

fn fitting_size(value: &str, width: f32, size: f32) -> f32 {
    size.min((width / text_width(value).max(1.0)).max(6.0))
}

fn wrap(value: &str, width: f32, size: f32) -> Vec<String> {
    let max = (width / size).max(2.0);
    let mut out = Vec::new();
    let mut line = String::new();
    let mut used = 0.0;
    for word in value.split_whitespace() {
        let word_width = text_width(word);
        if !line.is_empty() && used + word_width + 0.32 > max {
            out.push(std::mem::take(&mut line));
            used = 0.0;
        }
        if !line.is_empty() {
            line.push(' ');
            used += 0.32;
        }
        for c in word.chars() {
            let width = char_width(c);
            if used + width > max && !line.is_empty() {
                out.push(std::mem::take(&mut line));
                used = 0.0;
            }
            line.push(c);
            used += width;
        }
    }
    if !line.is_empty() {
        out.push(line);
    }
    out
}

struct Geometry {
    board: Rect,
    header: Rect,
    wheel: Rect,
    found: Rect,
    preview: Rect,
}

impl Geometry {
    fn new(w: f32, h: f32) -> Self {
        let width = w.min(680.0);
        let left = (w - width) * 0.5;
        if w > h * 1.25 {
            let wheel_size = (h - 115.0).min(w * 0.38).clamp(90.0, 270.0);
            let wheel_x = w - (w * 0.29) - wheel_size * 0.15;
            return Self {
                board: Rect::new(12.0, 53.0, (wheel_x - 27.0).max(60.0), (h - 64.0).max(60.0)),
                header: Rect::new(14.0, 7.0, w - 28.0, 43.0),
                wheel: Rect::new(wheel_x, h - wheel_size - 48.0, wheel_size, wheel_size),
                found: Rect::new(wheel_x, h - 43.0, wheel_size, 37.0),
                preview: Rect::new(
                    wheel_x - 18.0,
                    h - wheel_size - 82.0,
                    wheel_size + 36.0,
                    31.0,
                ),
            };
        }
        let wheel_size = (width * 0.67).min(h * 0.42).clamp(105.0, 280.0);
        let wheel_y = h - wheel_size - 61.0;
        let board_top = if h < 520.0 { 68.0 } else { 79.0 };
        Self {
            board: Rect::new(
                left + 10.0,
                board_top,
                width - 20.0,
                (wheel_y - board_top - 35.0).max(50.0),
            ),
            header: Rect::new(left + 16.0, 10.0, width - 32.0, 47.0),
            wheel: Rect::new((w - wheel_size) * 0.5, wheel_y, wheel_size, wheel_size),
            preview: Rect::new(left + 12.0, wheel_y - 38.0, width - 24.0, 34.0),
            found: Rect::new((w - 208.0) * 0.5, h - 50.0, 208.0, 40.0),
        }
    }

    fn tiles(&self, n: usize) -> Vec<Point> {
        let center = self.wheel.center();
        let radius = self.wheel.w * 0.358;
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
        (self.wheel.w * 0.337 * (std::f32::consts::PI / n.max(2) as f32).sin()).min(27.0)
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
            39.0 * self.zoom
        } else {
            ((board.w - 10.0) / game.level().cols.max(1) as f32)
                .min((board.h - 6.0) / game.level().rows.max(1) as f32)
                .clamp(7.0, 47.0)
        }
    }

    fn fit(game: &Game, board: Rect) -> f32 {
        ((board.w - 24.0) / (39.0 * game.level().cols.max(1) as f32))
            .min((board.h - 24.0) / (39.0 * game.level().rows.max(1) as f32))
            .clamp(0.03, 1.0)
    }

    fn origin(self, game: &Game, board: Rect) -> Point {
        let cell = self.cell(game, board);
        let center = board.center();
        Point::new(
            center.x + self.offset.x - cell * game.level().cols as f32 * 0.5,
            center.y + self.offset.y - cell * game.level().rows as f32 * 0.5,
        )
    }

    fn constrain(&mut self, game: &Game, board: Rect) {
        let cell = self.cell(game, board);
        let max_x = ((board.w + cell * game.level().cols as f32) * 0.5 - 45.0).max(0.0);
        let max_y = ((board.h + cell * game.level().rows as f32) * 0.5 - 45.0).max(0.0);
        self.offset.x = self.offset.x.clamp(-max_x, max_x);
        self.offset.y = self.offset.y.clamp(-max_y, max_y);
    }

    fn scale(&mut self, game: &Game, board: Rect, factor: f32, anchor: Point) {
        let new = (self.zoom * factor).clamp((Self::fit(game, board) * 0.7).max(0.028), 2.5);
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
}

pub struct Ui {
    width: f32,
    height: f32,
    screen: Screen,
    panel: Option<Panel>,
    scroll: f32,
    found_scroll: f32,
    victory_scroll: f32,
    settings_scroll: f32,
    levels_page: u64,
    camera: Camera,
    gesture: Option<Gesture>,
    selection: Vec<usize>,
    tilt: Point,
    neutral_tilt: Option<Point>,
    effect: Value,
    toast: Option<TimedText>,
    repeat: Option<TimedText>,
    gift_flight: Option<GiftFlight>,
    reward: u64,
    started: i64,
    idle: Option<Idle>,
    idle_letters_next: bool,
    paused: bool,
    next_midnight: Option<i64>,
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            width: 390.0,
            height: 760.0,
            screen: Screen::Menu,
            panel: None,
            scroll: 0.0,
            found_scroll: 0.0,
            victory_scroll: 0.0,
            levels_page: 0,
            camera: Camera::default(),
            settings_scroll: 0.0,
            gesture: None,
            selection: Vec::new(),
            tilt: Point::default(),
            neutral_tilt: None,
            effect: json!({}),
            toast: None,
            repeat: None,
            gift_flight: None,
            reward: 0,
            started: 0,
            idle: None,
            idle_letters_next: false,
            paused: false,
            next_midnight: None,
        }
    }
}

impl Ui {
    pub fn frame(&self, game: &Game) -> Value {
        let paint = self.paint(game);
        let controls: Vec<Value> = paint.controls.into_iter().enumerate().map(|(id, item)| {
            json!({"id":id,"label":item.label,"x":item.rect.x,"y":item.rect.y,"w":item.rect.w,"h":item.rect.h})
        }).collect();
        json!({"commands":paint.commands,"controls":controls,"animate":self.gift_flight.is_some()})
    }

    fn geometry(&self) -> Geometry {
        Geometry::new(self.width, self.height)
    }

    fn paint(&self, game: &Game) -> Paint {
        let mut paint = Paint::default();
        let canvas = Rect::new(0.0, 0.0, self.width, self.height);
        let name = game
            .state
            .settings
            .background
            .as_deref()
            .filter(|name| BACKGROUNDS.iter().any(|background| background.id == *name))
            .unwrap_or("ufa");
        let image = format!("assets/background-{name}.webp");
        let picture = if game.state.settings.background_motion {
            let pad = 16.0;
            Rect::new(
                self.tilt.x - pad,
                self.tilt.y - pad,
                self.width + pad * 2.0,
                self.height + pad * 2.0,
            )
        } else {
            canvas
        };
        paint.rect(canvas, 0.0, "#342048");
        paint.image(&image, picture, 1.0);
        paint.rect(canvas, 0.0, "#70382552");
        match self.screen {
            Screen::Menu => self.paint_menu(game, &mut paint),
            Screen::Play => self.paint_game(game, &mut paint),
            Screen::Victory => self.paint_victory(game, &mut paint),
        }
        if let Some(panel) = &self.panel {
            paint.controls.clear();
            paint.scroll = None;
            self.paint_panel(game, &mut paint, panel);
        }
        if let Some(flight) = &self.gift_flight {
            let t = (game.now_millis.saturating_sub(flight.began) as f32 / 620.0).clamp(0.0, 1.0);
            let easing = t * t * (3.0 - 2.0 * t);
            let size = 23.0 + t.sin() * 4.0;
            let from = self.menu_button_bounds(2).center();
            let to = Point::new(self.width - 75.0, 31.0);
            let x = from.x + (to.x - from.x) * easing;
            let y = from.y + (to.y - from.y) * easing - (t * std::f32::consts::PI).sin() * 38.0;
            paint.image(
                "assets/coin.png",
                Rect::new(x - size * 0.5, y - size * 0.5, size, size),
                1.0,
            );
        }
        if let Some(text) = &self.toast {
            let w = (text_width(&text.text) * 14.0 + 23.0).min(self.width - 26.0);
            let rect = Rect::new((self.width - w) * 0.5, 15.0, w, 36.0);
            paint.rect(rect, 10.0, PAPER);
            paint.center_text(
                rect,
                &text.text,
                fitting_size(&text.text, w - 12.0, 14.0),
                INK,
            );
        }
        paint
    }

    fn balance(&self, paint: &mut Paint, x_right: f32, y: f32, coins: u64) {
        let label = coins.to_string();
        let w = (text_width(&label) * 16.0 + 45.0).max(70.0);
        let bounds = Rect::new(x_right - w, y, w, 39.0);
        paint.rect(bounds, 11.0, TRANSLUCENT);
        paint.outline(bounds, 11.0, 1.0, LINE);
        paint.image(
            "assets/coin.png",
            Rect::new(bounds.x + 10.0, bounds.y + 10.0, 18.0, 18.0),
            1.0,
        );
        paint.text(
            Point::new(bounds.x + 34.0, bounds.y + 25.0),
            label,
            16.0,
            true,
            "left",
            WHITE,
        );
    }

    fn paint_menu(&self, game: &Game, paint: &mut Paint) {
        let w = self.width;
        let h = self.height;
        let top = 12.0;
        let center = w * 0.5;
        self.balance(paint, w - 16.0, top, game.state.coins);
        paint.button(
            Rect::new(16.0, top, 94.0, 39.0),
            "Рейтинг",
            Action::Panel(Panel::Rating),
            Button::Secondary,
        );
        let main_first = self.menu_button_bounds(0);
        let actions_width = main_first.w;
        let first = main_first.y;
        let title_size = ((first - 48.0) * 0.4).clamp(25.0, 81.0);
        if first > 210.0 {
            paint.text(
                Point::new(center, first - title_size - 40.0),
                "Башҡортса",
                15.0,
                true,
                "center",
                GOLD,
            );
            for (i, letter) in ["Ә", "Ө", "Ү"].iter().enumerate() {
                let box_rect = Rect::new(
                    center - 68.0 + i as f32 * 48.0,
                    first - title_size - 29.0,
                    40.0,
                    41.0,
                );
                paint.rect(box_rect, 9.0, "#EEFFF7ED");
                paint.center_text(box_rect, letter, 23.0, INK);
            }
        }
        paint.text(
            Point::new(center, first - 34.0),
            "Слова",
            title_size,
            true,
            "center",
            WHITE,
        );
        paint.text(
            Point::new(center, first - 12.0),
            "Башкирская словесная головоломка",
            fitting_size("Башкирская словесная головоломка", actions_width, 15.0),
            false,
            "center",
            WHITE,
        );
        let buttons = [
            ("Классика", Action::Mode(Mode::Classic), Button::Primary),
            ("Хардкор", Action::Mode(Mode::Hardcore), Button::Secondary),
            ("Подарок", Action::Claim, Button::Secondary),
            (
                "Настройки",
                Action::Panel(Panel::Settings),
                Button::Secondary,
            ),
            ("Об игре", Action::Panel(Panel::About), Button::Secondary),
        ];
        for (i, (label, action, style)) in buttons.into_iter().enumerate() {
            let bounds = self.menu_button_bounds(i);
            if matches!(action, Action::Claim) {
                let available = game
                    .state
                    .daily_gift
                    .last_claim_day
                    .is_none_or(|day| day < game.local_day);
                let style = if available { style } else { Button::Disabled };
                paint.button(bounds, label, action, style);
                if available {
                    paint.image(
                        "assets/coin.png",
                        Rect::new(bounds.right() - 45.0, bounds.center().y - 9.0, 17.0, 17.0),
                        1.0,
                    );
                } else {
                    let remain = daily_remaining(game, self.next_midnight);
                    paint.text(
                        Point::new(bounds.right() - 13.0, bounds.center().y + 4.0),
                        remain,
                        12.0,
                        false,
                        "right",
                        GOLD,
                    );
                }
            } else {
                paint.button(bounds, label, action, style);
            }
        }
        let button_end = self.menu_button_bounds(4).bottom();
        if h - button_end > 36.0 {
            paint.text(
                Point::new(center, button_end + 23.0),
                "Каждое найденное слово — маленькое открытие",
                fitting_size(
                    "Каждое найденное слово — маленькое открытие",
                    actions_width,
                    13.0,
                ),
                false,
                "center",
                WHITE,
            );
        }
    }

    fn menu_button_bounds(&self, index: usize) -> Rect {
        let width = (self.width - 44.0).min(395.0);
        let height = ((self.height - 122.0) / 8.0).clamp(28.0, 51.0);
        let gap = if self.height < 465.0 { 6.0 } else { 11.0 };
        let total = 5.0 * height + 4.0 * gap;
        let first = ((self.height - total) * 0.55 + 44.0).min(self.height - total - 10.0);
        Rect::new(
            (self.width - width) * 0.5,
            first + index as f32 * (height + gap),
            width,
            height,
        )
    }

    fn paint_game(&self, game: &Game, paint: &mut Paint) {
        let g = self.geometry();
        let level = game.level();
        let progress = game.progress();
        let header = g.header;
        self.paint_board(game, paint, g.board);
        let menu = Rect::new(header.x, header.y, 44.0, 42.0);
        paint.icon_button(
            menu,
            Icon::Menu,
            "В меню",
            Action::Navigate(Screen::Menu),
            false,
            true,
        );
        self.balance(paint, header.right(), header.y, game.state.coins);
        let label = if game.is_hardcore() {
            "Хардкор".to_string()
        } else {
            format!("Уровень {}", level.id)
        };
        paint.text(
            Point::new(header.center().x, header.y + 23.0),
            label,
            21.0,
            true,
            "center",
            WHITE,
        );
        if game.is_hardcore() {
            paint.text(
                Point::new(header.center().x, header.y + 43.0),
                format!("{}/{}", progress.solved.len(), level.words.len()),
                14.0,
                true,
                "center",
                GOLD,
            );
            paint.icon_button(
                Rect::new(header.right() - 41.0, header.y + 46.0, 40.0, 37.0),
                Icon::Repeat,
                "Пересобрать уровень",
                Action::Panel(Panel::Regenerate),
                false,
                true,
            );
        } else {
            let hint = Rect::new(menu.x, menu.bottom() + 8.0, 42.0, 40.0);
            let has_hidden = game.cells().len() > progress.revealed.len();
            paint.icon_button(
                hint,
                Icon::Bulb,
                "Подсказка, 20 монет",
                Action::Hint,
                true,
                game.state.coins >= 20 && has_hidden,
            );
            paint.text(
                Point::new(hint.center().x - 4.0, hint.bottom() + 13.0),
                "20",
                12.0,
                true,
                "center",
                WHITE,
            );
            paint.image(
                "assets/coin.png",
                Rect::new(hint.center().x + 5.0, hint.bottom() + 3.0, 12.0, 12.0),
                1.0,
            );
        }
        if game.is_hardcore() {
            let x = g.board.right() - 37.0;
            paint.button(
                Rect::new(x, g.board.bottom() - 79.0, 33.0, 34.0),
                "+",
                Action::Zoom(1.25),
                Button::Primary,
            );
            paint.button(
                Rect::new(x, g.board.bottom() - 40.0, 33.0, 34.0),
                "−",
                Action::Zoom(0.8),
                Button::Primary,
            );
        }
        self.paint_wheel(game, paint, &g);
        if let Some(hint) = &self.idle {
            if !hint.marks_tiles {
                if let Some(meaning) = game.meanings(&hint.word).first() {
                    let max_width = self.width - 111.0;
                    let lines = wrap(meaning, max_width - 16.0, 13.0);
                    let rect = Rect::new(
                        (self.width - max_width) * 0.5,
                        g.board.y + 4.0,
                        max_width,
                        10.0 + lines.len() as f32 * 17.0,
                    );
                    paint.rect(rect, 11.0, PAPER);
                    for (index, row) in lines.into_iter().enumerate() {
                        paint.text(
                            Point::new(rect.center().x, rect.y + 17.0 * (index + 1) as f32),
                            row,
                            13.0,
                            true,
                            "center",
                            INK,
                        );
                    }
                }
            }
        }
        paint.button(
            g.found,
            "Найденные слова",
            Action::Panel(Panel::Found),
            Button::Secondary,
        );
    }

    fn paint_board(&self, game: &Game, paint: &mut Paint, area: Rect) {
        let step = self.camera.cell(game, area);
        let origin = self.camera.origin(game, area);
        let gap = (step * 0.085).max(0.6);
        let size = (step - gap).max(2.0);
        let progress = game.progress();
        let revealed: HashSet<_> = progress.revealed.iter().map(String::as_str).collect();
        let solved: HashSet<_> = progress.solved.iter().map(String::as_str).collect();
        paint.clip(area);
        for cell in game.cells() {
            let rect = Rect::new(
                origin.x + cell.col as f32 * step + gap * 0.5,
                origin.y + cell.row as f32 * step + gap * 0.5,
                size,
                size,
            );
            if rect.intersection(area).is_none() {
                continue;
            }
            let shown = revealed.contains(cell.key.as_str());
            let complete = shown && cell.words.iter().any(|word| solved.contains(word.as_str()));
            let color = if complete {
                "#8754A9"
            } else if shown {
                "#E6CFE8"
            } else {
                "#F8FFF9F1"
            };
            paint.rect(
                Rect::new(rect.x, rect.y + 1.6, rect.w, rect.h),
                size * 0.17,
                "#6741265D",
            );
            paint.rect(rect, size * 0.17, color);
            if shown && size >= 7.0 {
                paint.center_text(
                    rect,
                    &cell.letter.to_string(),
                    (size * 0.63).min(45.0),
                    if complete { WHITE } else { INK },
                );
            }
            if self
                .repeat
                .as_ref()
                .is_some_and(|message| cell.words.contains(&message.text))
            {
                paint.outline(rect.inset(1.0), size * 0.17, 2.0, "#43CC77");
            }
        }
        paint.unclip();
    }

    fn paint_wheel(&self, game: &Game, paint: &mut Paint, g: &Geometry) {
        let center = g.wheel.center();
        let radius = g.wheel.w * 0.5;
        let layout = g.tiles(game.rack.len());
        let tile_radius = g.tile_radius(game.rack.len());
        paint.circle(Point::new(center.x, center.y + 3.0), radius, "#5C3C2958");
        paint.circle(center, radius, "#E3FFF9F5");
        for indexes in self.selection.windows(2) {
            if let (Some(a), Some(b)) = (layout.get(indexes[0]), layout.get(indexes[1])) {
                paint.line(*a, *b, 4.0, "#A45DB4");
            }
        }
        if let Some(Gesture::Wheel { point, .. }) = self.gesture.as_ref() {
            if let Some(last) = self.selection.last().and_then(|i| layout.get(*i)) {
                let d = center.distance(*point);
                let end = if d > radius {
                    Point::new(
                        center.x + (point.x - center.x) * radius / d,
                        center.y + (point.y - center.y) * radius / d,
                    )
                } else {
                    *point
                };
                paint.line(*last, end, 4.0, "#A45DB4");
            }
        }
        let mut hinted: Vec<char> = self
            .idle
            .as_ref()
            .filter(|hint| hint.marks_tiles)
            .map(|hint| hint.word.chars().collect())
            .unwrap_or_default();
        for (index, (&letter, at)) in game.rack.iter().zip(layout.iter().copied()).enumerate() {
            let selected = self.selection.contains(&index);
            let marked = hinted
                .iter()
                .position(|&x| x == letter)
                .map(|i| hinted.remove(i))
                .is_some();
            paint.circle(Point::new(at.x, at.y + 1.4), tile_radius, "#5981608E");
            paint.circle(
                at,
                tile_radius,
                if selected {
                    "#9459B5"
                } else if marked {
                    "#F5D16A"
                } else {
                    "#F8FFF9F4"
                },
            );
            let bounds = Rect::new(
                at.x - tile_radius,
                at.y - tile_radius,
                tile_radius * 2.0,
                tile_radius * 2.0,
            );
            paint.outline(
                bounds,
                tile_radius,
                1.1,
                if selected { "#8550AC" } else { "#AE916A9E" },
            );
            let label = letter.to_string();
            paint.center_text(
                bounds,
                &label,
                (tile_radius * 1.22).min(34.0),
                if selected { WHITE } else { "#624078" },
            );
            paint.control(
                bounds,
                &format!("{label}, буква {}", index + 1),
                Action::Select(index),
            );
        }
        let shuffle = Rect::new(center.x - 22.0, center.y - 22.0, 44.0, 44.0);
        paint.icon(center, Icon::Shuffle, 23.0, "#825894");
        paint.control(shuffle, "Перемешать буквы", Action::Shuffle);
        let word = self.selected_word(game);
        if word.is_empty() {
            paint.center_text(g.preview, "· · ·", 17.0, "#DDFFF1F8");
        } else {
            let count = word.chars().count() as f32;
            let step = ((g.preview.w - 4.0) / count).min(g.preview.h + 3.0);
            let size = step - 3.0;
            let origin = g.preview.center().x - step * count * 0.5;
            for (index, letter) in word.chars().enumerate() {
                let rect = Rect::new(
                    origin + (index as f32) * step,
                    g.preview.center().y - size * 0.5,
                    size,
                    size,
                );
                paint.rect(rect, 6.0, PAPER);
                paint.center_text(rect, &letter.to_string(), size * 0.63, INK);
            }
            if self.gesture.is_none() {
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

impl Ui {
    fn paint_victory(&self, game: &Game, paint: &mut Paint) {
        let width = (self.width - 32.0).min(450.0);
        let left = (self.width - width) * 0.5;
        let center = self.width * 0.5;
        let condensed = self.height < 620.0;
        let badge = if condensed { 25.0 } else { 38.0 };
        paint.circle(Point::new(center, badge + 21.0), badge + 5.0, "#44FFDA8E");
        paint.circle(Point::new(center, badge + 21.0), badge, GOLD);
        paint.text(
            Point::new(center, badge + 21.0 + badge * 0.31),
            "✓",
            badge * 1.23,
            true,
            "center",
            PURPLE,
        );
        let stage = if game.is_hardcore() {
            "Хардкор".to_string()
        } else {
            format!("Уровень {}", game.level().id)
        };
        let heading = badge * 2.0 + 53.0;
        paint.text(
            Point::new(center, heading - 9.0),
            stage,
            15.0,
            true,
            "center",
            GOLD,
        );
        paint.text(
            Point::new(center, heading + 23.0),
            "Уровень пройден!",
            fitting_size("Уровень пройден!", width, 31.0),
            true,
            "center",
            WHITE,
        );
        let reward = if self.reward > 0 {
            format!("Награда +{}", self.reward)
        } else {
            "Награда уже получена".to_string()
        };
        let chip = Rect::new(center - 116.0, heading + 36.0, 232.0, 37.0);
        paint.rect(chip, 10.0, TRANSLUCENT);
        paint.center_text(chip, &reward, 15.0, WHITE);
        let list_top = chip.bottom() + 12.0;
        let bottom = self.height - 103.0;
        let list = Rect::new(left, list_top, width, (bottom - list_top - 8.0).max(32.0));
        self.word_list(game, paint, list, self.victory_scroll, false);
        paint.button(
            Rect::new(left, bottom, width, 44.0),
            "Следующий уровень",
            Action::Next,
            Button::Primary,
        );
        paint.button(
            Rect::new(left, bottom + 49.0, width, 41.0),
            "В меню",
            Action::Navigate(Screen::Menu),
            Button::Secondary,
        );
    }

    fn panel_rect(&self, panel: &Panel) -> Rect {
        let width = (self.width - 28.0).min(460.0);
        let source_height = match panel {
            Panel::Reset | Panel::Replay(_) | Panel::Purchase(_) => 220.0,
            Panel::Regenerate | Panel::Rating => 156.0,
            Panel::Feedback => 195.0,
            Panel::Translation { .. } => 448.0,
            Panel::Found => (self.height * 0.72).clamp(180.0, 610.0),
            Panel::Settings | Panel::Help | Panel::About | Panel::Attribution | Panel::Levels => {
                self.height - 65.0
            }
        };
        let height = source_height.min(self.height - 22.0);
        Rect::new(
            (self.width - width) * 0.5,
            (self.height - height) * 0.5,
            width,
            height,
        )
    }

    fn paint_panel(&self, game: &Game, paint: &mut Paint, panel: &Panel) {
        let card = self.panel_rect(panel);
        paint.rect(
            Rect::new(0.0, 0.0, self.width, self.height),
            0.0,
            "#93241533",
        );
        paint.rect(card, 20.0, PAPER);
        let close = Rect::new(card.right() - 49.0, card.y + 13.0, 36.0, 36.0);
        let label = panel.title();
        paint.text(
            Point::new(card.x + 18.0, card.y + 38.0),
            label,
            fitting_size(label, card.w - 78.0, 22.0),
            true,
            "left",
            INK,
        );
        paint.rect(close, 9.0, SECONDARY);
        paint.icon(close.center(), Icon::Close, 21.0, PURPLE);
        paint.control(close, "Закрыть", Action::Close);
        let bounds = Rect::new(card.x + 17.0, card.y + 63.0, card.w - 34.0, card.h - 76.0);
        match panel {
            Panel::Found => self.word_list(game, paint, bounds, self.found_scroll, true),
            Panel::Settings => self.paint_settings(game, paint, bounds),
            Panel::Translation { word, .. } => self.paint_translation(game, paint, bounds, word),
            Panel::About => self.paint_about(paint, bounds),
            Panel::Help => self.paint_help(paint, bounds),
            Panel::Attribution => self.paint_attribution(paint, bounds),
            Panel::Levels => self.paint_levels(game, paint, bounds),
            Panel::Rating => {
                paint.center_text(bounds, "В разработке", 17.0, INK);
            }
            Panel::Feedback => {
                paint.paragraph(
                    bounds.x,
                    bounds.y + 5.0,
                    bounds.w,
                    "Отправка предложений из игры пока недоступна.",
                    16.0,
                    INK,
                );
            }
            Panel::Reset => {
                self.confirm(paint, bounds, "Игра начнётся с первого уровня и 90 монет. Найденные слова, награды и открытые фоны будут сброшены.", "Сбросить", Action::Reset, true);
            }
            Panel::Regenerate => {
                self.confirm(
                    paint,
                    bounds,
                    "Текущее поле хардкора будет заменено.",
                    "Пересобрать",
                    Action::Regenerate,
                    false,
                );
            }
            Panel::Replay(id) => {
                self.confirm(
                    paint,
                    bounds,
                    "Клетки будут скрыты. Награду за уровень и бонусы можно получить один раз.",
                    "Играть снова",
                    Action::Replay(*id),
                    false,
                );
            }
            Panel::Purchase(index) => {
                let bg = &BACKGROUNDS[*index];
                self.confirm(
                    paint,
                    bounds,
                    &format!("Фон «{}» стоит {} монет.", bg.title, bg.price),
                    "Купить",
                    Action::Buy(*index),
                    false,
                );
            }
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
        let button_h = 40.0;
        let button_y = bounds.bottom() - button_h;
        let width = (bounds.w - 8.0) * 0.5;
        paint.paragraph(
            bounds.x + 2.0,
            bounds.y,
            bounds.w - 4.0,
            description,
            15.0,
            INK,
        );
        paint.button(
            Rect::new(bounds.x, button_y, width, button_h),
            "Отмена",
            Action::Close,
            Button::Paper,
        );
        paint.button(
            Rect::new(bounds.right() - width, button_y, width, button_h),
            label,
            action,
            if danger {
                Button::Danger
            } else {
                Button::Accent
            },
        );
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
        let row_height = 60.0;
        let content = words.len() as f32 * row_height;
        let max = (content - bounds.h).max(0.0);
        let scroll = scroll.clamp(0.0, max);
        paint.scroll = Some(ScrollArea { bounds, max });
        paint.clip(bounds);
        if words.is_empty() {
            paint.paragraph(
                bounds.x + 5.0,
                bounds.y + 4.0,
                bounds.w - 10.0,
                "Здесь появятся разгаданные и бонусные слова.",
                16.0,
                if from_found { INK } else { WHITE },
            );
        }
        let solved: HashSet<_> = game.progress().solved.iter().map(String::as_str).collect();
        for (index, word) in words.iter().enumerate() {
            let rect = Rect::new(
                bounds.x,
                bounds.y + index as f32 * row_height - scroll,
                bounds.w,
                row_height - 6.0,
            );
            if rect.intersection(bounds).is_none() {
                continue;
            }
            paint.rect(rect, 10.0, "#F6EDF5");
            let translate = Rect::new(rect.right() - 96.0, rect.y + 9.0, 87.0, 35.0);
            let size = fitting_size(word, rect.w - 115.0, 18.0);
            paint.text(
                Point::new(rect.x + 12.0, rect.y + 22.0),
                word,
                size,
                true,
                "left",
                INK,
            );
            let kind = if solved.contains(word.as_str()) {
                "Слово поля"
            } else {
                "Бонусное слово"
            };
            paint.text(
                Point::new(rect.x + 12.0, rect.y + 41.0),
                kind,
                12.0,
                false,
                "left",
                "#806885",
            );
            paint.button(
                translate,
                "Перевод",
                Action::Panel(Panel::Translation {
                    word: word.clone(),
                    from_found,
                }),
                Button::Paper,
            );
        }
        paint.unclip();
        self.scrollbar(paint, bounds, scroll, max);
    }

    fn scroll_start(&self, paint: &mut Paint, bounds: Rect) -> f32 {
        paint.clip(bounds);
        bounds.y - self.scroll
    }

    fn scroll_finish(&self, paint: &mut Paint, bounds: Rect, y: f32) {
        let max = (y - bounds.y + self.scroll - bounds.h).max(0.0);
        paint.unclip();
        paint.scroll = Some(ScrollArea { bounds, max });
        self.scrollbar(paint, bounds, self.scroll, max);
    }

    fn scrollbar(&self, paint: &mut Paint, bounds: Rect, scroll: f32, max: f32) {
        if max > 0.0 {
            let height = (bounds.h * bounds.h / (bounds.h + max)).max(14.0);
            let top = bounds.y + (bounds.h - height).max(0.0) * scroll.clamp(0.0, max) / max;
            paint.rect(
                Rect::new(bounds.right() - 3.0, top, 3.0, height),
                1.5,
                "#AB91B1",
            );
        }
    }

    fn paint_settings(&self, game: &Game, paint: &mut Paint, bounds: Rect) {
        let mut y = self.scroll_start(paint, bounds);
        let settings = &game.state.settings;
        let switches = [
            ("Звук", Setting::Sound, settings.sound),
            ("Музыка", Setting::Music, settings.music),
            ("Анимации", Setting::Animations, settings.animations),
            ("Движение фона", Setting::Tilt, settings.background_motion),
        ];
        for (label, setting, active) in switches {
            let row = Rect::new(bounds.x, y, bounds.w - 5.0, 45.0);
            paint.text(
                Point::new(row.x + 3.0, row.y + 28.0),
                label,
                16.0,
                false,
                "left",
                INK,
            );
            let toggle = Rect::new(row.right() - 50.0, y + 9.0, 46.0, 25.0);
            paint.rect(toggle, 13.0, if active { "#8B51AA" } else { "#D9C5DF" });
            let knob_x = if active {
                toggle.right() - 13.0
            } else {
                toggle.x + 13.0
            };
            paint.circle(Point::new(knob_x, toggle.center().y), 9.0, "#FFFFFF");
            let accessibility = format!(
                "{label}, {}",
                if active {
                    "включено"
                } else {
                    "выключено"
                }
            );
            paint.control(row, &accessibility, Action::Toggle(setting));
            paint.line(
                Point::new(row.x, row.bottom()),
                Point::new(row.right(), row.bottom()),
                1.0,
                "#E8DAE7",
            );
            y += 45.0;
        }
        y += 13.0;
        paint.text(
            Point::new(bounds.x + 3.0, y + 16.0),
            "Фон игры",
            17.0,
            true,
            "left",
            INK,
        );
        y += 28.0;
        let card_w = (bounds.w - 12.0) * 0.5;
        for (index, bg) in BACKGROUNDS.iter().enumerate() {
            let owned = bg.price == 0 || game.state.backgrounds.owned.iter().any(|id| id == bg.id);
            let selected = game.state.settings.background.as_deref().unwrap_or("ufa") == bg.id;
            let r = Rect::new(
                bounds.x + (index % 2) as f32 * (card_w + 8.0),
                y + (index / 2) as f32 * 108.0,
                card_w,
                100.0,
            );
            paint.rect(r, 10.0, if selected { "#FFF0D3" } else { "#EEE4EF" });
            let pic = Rect::new(r.x + 5.0, r.y + 5.0, r.w - 10.0, 58.0);
            paint.image(&format!("assets/background-{}.webp", bg.id), pic, 1.0);
            paint.text(
                Point::new(r.center().x, r.y + 80.0),
                bg.title,
                14.0,
                true,
                "center",
                INK,
            );
            let detail = if selected {
                "Выбран".to_string()
            } else if owned {
                "Открыт".to_string()
            } else {
                format!("{} монет", bg.price)
            };
            paint.text(
                Point::new(r.center().x, r.y + 94.0),
                &detail,
                11.0,
                false,
                "center",
                INK,
            );
            if selected {
                paint.outline(r, 10.0, 2.0, "#AA7727");
            }
            paint.control(
                r,
                &format!("{}: {detail}", bg.title),
                Action::Background(index),
            );
        }
        y += 324.0;
        y = paint.paragraph(
            bounds.x + 3.0,
            y + 5.0,
            bounds.w - 8.0,
            "Настройки сохраняются на этом устройстве.",
            14.0,
            "#806885",
        ) + 11.0;
        paint.button(
            Rect::new(bounds.x, y, bounds.w - 5.0, 42.0),
            "Сбросить прогресс",
            Action::Panel(Panel::Reset),
            Button::Danger,
        );
        self.scroll_finish(paint, bounds, y + 48.0);
    }

    fn paint_translation(&self, game: &Game, paint: &mut Paint, bounds: Rect, word: &str) {
        let mut y = self.scroll_start(paint, bounds);
        paint.text(
            Point::new(bounds.x + 3.0, y + 35.0),
            word,
            fitting_size(word, bounds.w - 8.0, 35.0),
            true,
            "left",
            PURPLE,
        );
        y += 44.0;
        let meanings = game.meanings(word);
        if meanings.is_empty() {
            y = paint.paragraph(
                bounds.x + 4.0,
                y,
                bounds.w - 8.0,
                "В этой базе пока нет русского перевода",
                15.0,
                INK,
            ) + 8.0;
        } else {
            paint.text(
                Point::new(bounds.x + 4.0, y + 14.0),
                "существительное",
                14.0,
                false,
                "left",
                "#8F709C",
            );
            y += 25.0;
            for (index, meaning) in meanings.iter().enumerate() {
                paint.text(
                    Point::new(bounds.x + 4.0, y + 17.0),
                    format!("{}.", index + 1),
                    16.0,
                    true,
                    "left",
                    PURPLE,
                );
                y = paint.paragraph(bounds.x + 29.0, y, bounds.w - 35.0, meaning, 16.0, INK) + 8.0;
            }
        }
        let links = game.sources(word);
        if !links.is_empty() {
            y += 5.0;
            paint.text(
                Point::new(bounds.x + 4.0, y + 16.0),
                "Источники словаря",
                15.0,
                true,
                "left",
                PURPLE,
            );
            y += 25.0;
            for (index, link) in links
                .into_iter()
                .filter(|url| url.starts_with("https://"))
                .take(3)
                .enumerate()
            {
                paint.button(
                    Rect::new(bounds.x + 4.0, y, bounds.w - 9.0, 38.0),
                    &format!("Источник {}", index + 1),
                    Action::OpenLink(link),
                    Button::Paper,
                );
                y += 45.0;
            }
        }
        self.scroll_finish(paint, bounds, y + 3.0);
    }

    fn paint_about(&self, paint: &mut Paint, bounds: Rect) {
        let mut y = self.scroll_start(paint, bounds);
        y = paint.paragraph(bounds.x + 4.0, y, bounds.w - 8.0,
            "«Слова» — башкирская словесная головоломка. Игра использует базу башкирских существительных с русскими значениями. В словарных записях приведены источники.", 16.0, INK) + 13.0;
        for (label, panel) in [
            ("Как играть", Panel::Help),
            ("Уровни", Panel::Levels),
            ("Источники и лицензия", Panel::Attribution),
            ("Обратная связь", Panel::Feedback),
        ] {
            paint.button(
                Rect::new(bounds.x, y, bounds.w - 5.0, 43.0),
                label,
                Action::Panel(panel),
                Button::Paper,
            );
            y += 51.0;
        }
        paint.text(
            Point::new(bounds.x + 4.0, y + 16.0),
            "Задать вопрос",
            15.0,
            true,
            "left",
            PURPLE,
        );
        y += 25.0;
        let w = (bounds.w - 15.0) * 0.5;
        for (index, (name, icon, url)) in [
            (
                "Telegram",
                "assets/contact-telegram.webp",
                "https://t.me/goinmasss",
            ),
            ("VK", "assets/contact-vk.png", "https://vk.ru/luckyanon"),
        ]
        .into_iter()
        .enumerate()
        {
            let row = Rect::new(bounds.x + index as f32 * (w + 8.0), y, w, 41.0);
            paint.button(row, name, Action::OpenLink(url.to_string()), Button::Paper);
            paint.image(icon, Rect::new(row.x + 7.0, row.y + 10.0, 19.0, 19.0), 1.0);
        }
        y += 50.0;
        self.scroll_finish(paint, bounds, y);
    }

    fn paint_help(&self, paint: &mut Paint, bounds: Rect) {
        let mut y = self.scroll_start(paint, bounds);
        for (i, description) in HELP.iter().enumerate() {
            paint.text(
                Point::new(bounds.x + 4.0, y + 17.0),
                format!("{}.", i + 1),
                16.0,
                true,
                "left",
                PURPLE,
            );
            y = paint.paragraph(bounds.x + 27.0, y, bounds.w - 33.0, description, 16.0, INK) + 12.0;
        }
        self.scroll_finish(paint, bounds, y);
    }

    fn paint_attribution(&self, paint: &mut Paint, bounds: Rect) {
        let mut y = self.scroll_start(paint, bounds);
        let document = include_str!("../../../public/ATTRIBUTION.md");
        for row in document.lines() {
            let line = row.trim();
            if line.is_empty() {
                y += 7.0;
                continue;
            }
            if let Some(heading) = line.strip_prefix("## ") {
                y += 6.0;
                paint.text(
                    Point::new(bounds.x + 4.0, y + 17.0),
                    heading,
                    fitting_size(heading, bounds.w - 8.0, 17.0),
                    true,
                    "left",
                    PURPLE,
                );
                y += 27.0;
            } else if !line.starts_with("# ") {
                let (text, links) = markdown_links(line);
                y = paint.paragraph(bounds.x + 4.0, y, bounds.w - 8.0, &text, 14.0, INK);
                for (label, url) in links {
                    paint.button(
                        Rect::new(bounds.x + 4.0, y + 4.0, bounds.w - 9.0, 35.0),
                        &label,
                        Action::OpenLink(url),
                        Button::Paper,
                    );
                    y += 43.0;
                }
                y += 3.0;
            }
        }
        self.scroll_finish(paint, bounds, y);
    }

    fn paint_levels(&self, game: &Game, paint: &mut Paint, bounds: Rect) {
        let mut y = self.scroll_start(paint, bounds);
        let col_width = (bounds.w - 29.0) / 5.0;
        let first_id = self.levels_page.saturating_mul(20).saturating_add(1);
        for index in 0..20 {
            let id = first_id.saturating_add(index);
            let label = id.to_string();
            let box_rect = Rect::new(
                bounds.x + (index % 5) as f32 * (col_width + 6.0),
                y + (index / 5) as f32 * 52.0,
                col_width,
                45.0,
            );
            let style = if id > game.state.max_unlocked {
                Button::Disabled
            } else if id == game.state.current_level {
                Button::Accent
            } else {
                Button::Paper
            };
            paint.button(box_rect, &label, Action::Level(id), style);
        }
        y += 215.0;
        let gap = 8.0;
        let w = (bounds.w - gap - 6.0) * 0.5;
        let previous = self.levels_page.saturating_sub(1);
        let next = self.levels_page.saturating_add(1);
        paint.button(
            Rect::new(bounds.x, y, w, 40.0),
            "Назад",
            Action::LevelPage(previous),
            if self.levels_page > 0 {
                Button::Paper
            } else {
                Button::Disabled
            },
        );
        paint.button(
            Rect::new(bounds.x + w + gap, y, w, 40.0),
            "Дальше",
            Action::LevelPage(next),
            if first_id.saturating_add(20) <= game.state.max_unlocked {
                Button::Paper
            } else {
                Button::Disabled
            },
        );
        y = paint.paragraph(
            bounds.x + 3.0,
            y + 52.0,
            bounds.w - 8.0,
            "Следующий уровень открывается после победы. Пройденные уровни можно решать снова.",
            14.0,
            INK,
        );
        self.scroll_finish(paint, bounds, y + 7.0);
    }
}

fn markdown_links(text: &str) -> (String, Vec<(String, String)>) {
    let mut rest = text;
    let mut prose = String::new();
    let mut links = Vec::new();
    while let Some(begin) = rest.find('[') {
        let Some((title, after)) = rest[begin + 1..].split_once("](") else {
            break;
        };
        let Some((url, suffix)) = after.split_once(')') else {
            break;
        };
        prose.push_str(&rest[..begin]);
        prose.push_str(title);
        if url.starts_with("https://") {
            links.push((title.to_string(), url.to_string()));
        }
        rest = suffix;
    }
    prose.push_str(rest);
    (prose.replace('`', ""), links)
}

impl Ui {
    pub fn take_effects(&mut self) -> Value {
        std::mem::replace(&mut self.effect, json!({}))
    }

    pub fn event(&mut self, game: &mut Game, event: Value) {
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
                    self.width = width;
                    self.height = height;
                    self.camera.constrain(game, self.geometry().board);
                    self.cancel();
                }
            }
            "down" => self.down(game, id, point),
            "move" => self.moved(game, id, point),
            "up" => self.up(game, id, point),
            "cancel" => self.cancel(),
            "activate" => {
                if let Some(action) = usize::try_from(id)
                    .ok()
                    .and_then(|id| self.paint(game).controls.into_iter().nth(id))
                {
                    self.activate(game, action.action);
                }
            }
            "back" => self.back(game),
            "scroll" => {
                let g = self.geometry();
                let delta = number("delta").clamp(-10.0, 10.0);
                if self.panel.is_none()
                    && self.screen == Screen::Play
                    && game.is_hardcore()
                    && g.board.contains(point)
                {
                    self.camera
                        .scale(game, g.board, (-delta * 0.13).exp(), point);
                } else if let Some(area) = self
                    .paint(game)
                    .scroll
                    .filter(|area| area.bounds.contains(point))
                {
                    self.scroll_by(delta * 32.0, area.max);
                }
            }
            "pause" => {
                self.paused = true;
                self.cancel();
                self.idle = None;
            }
            "resume" => {
                self.paused = false;
                self.neutral_tilt = None;
                self.attempt(game.now_millis);
            }
            "sensor" if game.state.settings.background_motion && !self.paused => {
                self.update_tilt(point)
            }
            "tick" => self.tick_idle(game),
            _ => {}
        }
        if self
            .toast
            .as_ref()
            .is_some_and(|item| item.until <= game.now_millis)
        {
            self.toast = None;
        }
        if self
            .repeat
            .as_ref()
            .is_some_and(|item| item.until <= game.now_millis)
        {
            self.repeat = None;
        }
        if self
            .gift_flight
            .as_ref()
            .is_some_and(|item| game.now_millis.saturating_sub(item.began) >= 620)
        {
            self.gift_flight = None;
        }
    }

    fn update_tilt(&mut self, point: Point) {
        let neutral = self.neutral_tilt.get_or_insert(point);
        let x = (point.x - neutral.x) * 0.55;
        let y = (point.y - neutral.y) * 0.55;
        let limit = 15.0;
        neutral.x += (x - x.clamp(-limit, limit)) * 0.17;
        neutral.y += (y - y.clamp(-limit, limit)) * 0.17;
        self.tilt.x += (x.clamp(-limit, limit) - self.tilt.x) * 0.16;
        self.tilt.y += (y.clamp(-limit, limit) - self.tilt.y) * 0.16;
    }

    fn down(&mut self, game: &mut Game, id: i64, point: Point) {
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
        if game_visible {
            if let Some(index) = self.tile_at(game, point) {
                self.selection.clear();
                self.pick(game, index);
                self.gesture = Some(Gesture::Wheel { id, point });
                return;
            }
        }
        let paint = self.paint(game);
        let scroll = paint.scroll.filter(|area| area.bounds.contains(point));
        let control = paint
            .controls
            .into_iter()
            .rev()
            .find(|item| item.rect.contains(point));
        if let Some(control) = control {
            self.gesture = Some(Gesture::Press {
                id,
                start: point,
                current: point,
                action: control.action,
                rect: control.rect,
                scrolled: false,
                scroll,
            });
        } else if game_visible && game.is_hardcore() && self.geometry().board.contains(point) {
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
            .is_some_and(|panel| !self.panel_rect(panel).contains(point))
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
                if let Some(area) = scroll {
                    if *scrolled || start.distance(point) > 7.0 {
                        *scrolled = true;
                        self.scroll_by(current.y - point.y, area.max);
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
        let board = self.geometry().board;
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
            let result = game.submit(&word);
            self.feedback(game, result);
        }
        self.selection.clear();
    }

    fn feedback(&mut self, game: &Game, result: ActionResult) {
        if result.completed && (result.kind == "target" || result.kind == "hint") {
            self.show_game(game, result.reward);
        } else {
            match result.kind {
                "bonus" if result.reward > 0 => {
                    self.message(game, format!("Бонусное слово: +{}", result.reward))
                }
                "bonus" => self.message(game, "Бонусное слово"),
                "repeat" => {
                    self.message(game, "Слово уже найдено");
                    self.focus_word(game, &result.word);
                    self.repeat = Some(TimedText {
                        text: result.word,
                        until: game.now_millis + 1_000,
                    });
                }
                "insufficient" => self.message(game, "Недостаточно монет"),
                "excluded-length" => self.message(
                    game,
                    format!(
                        "На этом уровне нет слов из {} букв",
                        result.word.chars().count()
                    ),
                ),
                "invalid" | "unbuildable" | "outside" => self.message(game, "Слово не подходит"),
                _ => {}
            }
        }
    }

    fn focus_word(&mut self, game: &Game, word: &str) {
        if !game.is_hardcore() {
            return;
        }
        let Some(item) = game.level().words.iter().find(|item| item.word == word) else {
            return;
        };
        let board = self.geometry().board;
        let size = self.camera.cell(game, board);
        let origin = self.camera.origin(game, board);
        let cells: Vec<_> = item
            .cells()
            .map(|(row, col, _)| {
                Point::new(
                    origin.x + (col as f32 + 0.5) * size,
                    origin.y + (row as f32 + 0.5) * size,
                )
            })
            .collect();
        if !cells.is_empty()
            && cells
                .iter()
                .any(|point| !board.inset(13.0).contains(*point))
        {
            let center = board.center();
            let word_center = cells[0].midpoint(cells[cells.len() - 1]);
            self.camera.pan(
                game,
                board,
                center.x - word_center.x,
                center.y - word_center.y,
            );
        }
    }

    fn message(&mut self, game: &Game, text: impl Into<String>) {
        self.toast = Some(TimedText {
            text: text.into(),
            until: game.now_millis + 2_400,
        });
    }

    fn scroll_by(&mut self, delta: f32, max: f32) {
        let target = if matches!(self.panel, Some(Panel::Found)) {
            &mut self.found_scroll
        } else if self.panel.is_some() {
            &mut self.scroll
        } else {
            &mut self.victory_scroll
        };
        *target = (target.clamp(0.0, max) + delta).clamp(0.0, max);
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
            Camera::fit(game, self.geometry().board).max(0.70)
        } else {
            1.0
        };
        self.camera = Camera {
            zoom,
            ..Camera::default()
        };
        self.cancel();
        self.toast = None;
        self.repeat = None;
        self.idle_letters_next = false;
        self.attempt(game.now_millis);
    }

    fn show_panel(&mut self, game: &Game, panel: Panel) {
        if matches!(self.panel, Some(Panel::Settings)) {
            self.settings_scroll = self.scroll;
        }
        self.scroll = 0.0;
        if matches!(panel, Panel::Found) {
            self.found_scroll = f32::MAX;
        }
        if matches!(panel, Panel::Levels) {
            self.levels_page = game.state.current_level.saturating_sub(1) / 20;
        }
        self.cancel();
        self.attempt(game.now_millis);
        self.panel = Some(panel);
    }

    fn close_panel(&mut self, game: &Game) {
        self.cancel();
        self.scroll = 0.0;
        self.attempt(game.now_millis);
        self.panel = match self.panel.take() {
            Some(Panel::Translation {
                from_found: true, ..
            }) => Some(Panel::Found),
            Some(Panel::Help | Panel::Attribution | Panel::Levels | Panel::Feedback) => {
                Some(Panel::About)
            }
            Some(Panel::Reset | Panel::Purchase(_)) => {
                self.scroll = self.settings_scroll;
                Some(Panel::Settings)
            }
            Some(Panel::Replay(_)) => Some(Panel::Levels),
            _ => None,
        };
    }

    fn cancel(&mut self) {
        self.selection.clear();
        self.gesture = None;
    }

    fn back(&mut self, game: &Game) {
        if self.panel.is_some() {
            self.close_panel(game);
        } else if !self.selection.is_empty() {
            self.cancel();
        } else if self.screen != Screen::Menu {
            self.screen = Screen::Menu;
            self.cancel();
            self.attempt(game.now_millis);
        } else {
            self.effect["exit"] = json!(true);
        }
    }

    fn activate(&mut self, game: &mut Game, action: Action) {
        self.attempt(game.now_millis);
        match action {
            Action::Navigate(screen) => {
                self.screen = screen;
                self.panel = None;
                self.cancel();
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
                if game.claim_gift(game.local_day) {
                    if game.state.settings.animations {
                        self.gift_flight = Some(GiftFlight {
                            began: game.now_millis,
                        });
                    }
                    self.message(game, "Подарок получен: +100 монет");
                }
            }
            Action::Shuffle => {
                game.shuffle();
                self.cancel();
            }
            Action::Hint => {
                self.cancel();
                let result = game.hint();
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
            }
            Action::Level(id) => {
                if crate::save::contains(&game.state.completed_ranges, id) {
                    self.show_panel(game, Panel::Replay(id));
                } else if game.start_level(id) {
                    self.show_game(game, 0);
                }
            }
            Action::LevelPage(page) => {
                self.levels_page = page;
                self.scroll = 0.0;
            }
            Action::Replay(id) => {
                if game.replay(id) {
                    self.show_game(game, 0);
                }
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
                    self.show_panel(game, Panel::Purchase(index));
                }
            }
            Action::Buy(index) => {
                let result = game.purchase_background(BACKGROUNDS[index].id);
                if result.kind == "purchased" || result.kind == "owned" {
                    game.select_background(BACKGROUNDS[index].id);
                    self.close_panel(game);
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
                if setting == Setting::Tilt {
                    self.neutral_tilt = None;
                    self.tilt = Point::default();
                }
                game.mark_dirty();
            }
            Action::Select(index) => self.pick(game, index),
            Action::Submit => self.submit_selection(game),
            Action::Zoom(factor) => {
                let board = self.geometry().board;
                self.camera.scale(game, board, factor, board.center());
            }
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
            let label = format!("{wanted}, буква {}", index + 1);
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
        click(&mut ui, &mut game, "Перевод");
        assert!(matches!(ui.panel, Some(Panel::Translation { .. })));
        assert!(
            ui.frame(&game)["commands"]
                .as_array()
                .unwrap()
                .iter()
                .any(|command| command["text"] == word)
        );
        ui.event(&mut game, json!({"type":"back"}));
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
        let board = ui.geometry().board.center();
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
        click(&mut ui, &mut game, "Пересобрать уровень");
        click(&mut ui, &mut game, "Пересобрать");
        assert!(game.level().id > id);
        assert_eq!(game.state.coins, 190);
    }
}
