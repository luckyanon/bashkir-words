use super::{Geometry, Rect, theme};

pub(super) fn game(viewport_width: f32, viewport_height: f32) -> Geometry {
    let width = viewport_width.max(280.0);
    let deck_width = width.min(540.0);
    let wide = viewport_width >= 800.0;
    let narrow = viewport_width <= 360.0;
    let compact = viewport_height <= 760.0;
    let short = viewport_height <= 620.0;
    let border = if wide { 1.0 } else { 0.0 };
    let pad_x = if narrow { 14.0 } else { 25.0 };
    let pad_top = if compact {
        12.0
    } else if wide {
        25.0
    } else {
        18.0
    };
    let pad_bottom = if compact {
        10.0
    } else if wide {
        20.0
    } else {
        16.0
    };
    let content_left = (width - deck_width) * 0.5 + pad_x + border;
    let content_width = deck_width - (pad_x + border) * 2.0;
    let menu_size: f32 = if short { 40.0 } else { 48.0 };
    let header = Rect::new(content_left, pad_top, content_width, menu_size.max(47.0));
    let wheel_size = if short {
        175.0
    } else if compact {
        (viewport_width * 0.64).min(245.0)
    } else if wide {
        280.0
    } else {
        (viewport_width * 0.70).min(290.0)
    };
    let wheel_gap = if short {
        7.0
    } else if compact {
        8.0
    } else {
        12.0
    };
    let footer_pad = if compact { 8.0 } else { 12.0 };
    let (found_padding, found_h, found_font) = if narrow {
        (8.0, 44.0, 12.0)
    } else {
        (12.0, 46.0, 14.0)
    };
    let found_width = 20.0
        + 8.0
        + theme::text_width("Найденные слова", found_font, false)
        + found_padding * 2.0
        + 2.0;
    let board_margin = if short { 56.0 } else { 62.0 };
    let board_top = header.bottom() + board_margin;
    let content_bottom = (viewport_height - pad_bottom)
        .max(board_top + 90.0 + 38.0 + wheel_gap + wheel_size + footer_pad + found_h);
    let found = Rect::new(
        (width - found_width) * 0.5,
        content_bottom - found_h,
        found_width,
        found_h,
    );
    let wheel = Rect::new(
        (width - wheel_size) * 0.5,
        found.y - footer_pad - wheel_size,
        wheel_size,
        wheel_size,
    );
    let preview = Rect::new(
        content_left,
        wheel.y - wheel_gap - 38.0,
        content_width,
        38.0,
    );
    let focus_top = header.bottom() + 28.0;
    Geometry {
        board: Rect::new(
            content_left,
            board_top,
            content_width,
            (preview.y - board_top).max(90.0),
        ),
        viewport: Rect::new(0.0, 0.0, viewport_width, viewport_height),
        hardcore_focus: Rect::new(
            content_left,
            focus_top,
            content_width,
            (preview.y - focus_top).max(90.0),
        ),
        header,
        wheel,
        found,
        preview,
        menu_size,
        tile_max: if short { 42.0 } else { 53.0 },
    }
}

pub(super) const SUBTITLE: &str = "Башкирская словесная головоломка";
pub(super) const NOTE: &str = "Каждое найденное слово — маленькое открытие";

pub(super) struct Menu {
    pub stage: Rect,
    pub actions: [Rect; 5],
    pub note_y: f32,
    pub eyebrow_y: f32,
    pub title_y: f32,
    pub subtitle_y: f32,
    pub letters_y: Option<f32>,
    pub title_size: f32,
    pub subtitle_lines: Vec<String>,
    pub note_lines: Vec<String>,
}

pub(super) fn menu(viewport_width: f32, viewport_height: f32, scroll: f32) -> Menu {
    let width = viewport_width.max(280.0);
    let deck_width = width.min(520.0);
    let short = viewport_height <= 620.0;
    let compact = viewport_height <= 760.0;
    let pad_x = if viewport_width <= 360.0 { 20.0 } else { 28.0 };
    let pad_top = if short { 20.0 } else { 24.0 };
    let pad_bottom = if short { 20.0 } else { 25.0 };
    let gap = if short {
        20.0
    } else if compact {
        24.0
    } else if viewport_width >= 800.0 {
        36.0
    } else {
        32.0
    };
    let center = width * 0.5;
    let title_size = if short {
        62.0
    } else if compact {
        72.0
    } else {
        (viewport_width * 0.14).clamp(70.0, 94.0)
    };
    let subtitle_lines = theme::wrap(
        SUBTITLE,
        250.0_f32.min(deck_width - pad_x * 2.0),
        18.0,
        false,
    );
    let note_lines = theme::wrap(NOTE, 270.0_f32.min(deck_width - pad_x * 2.0), 13.0, false);
    let eyebrow_height = theme::line_height(13.0, false);
    let letters_height = if short { 0.0 } else { 56.0 + 8.0 + 17.0 };
    let title_height = letters_height + title_size + 17.0 + 27.0 * subtitle_lines.len() as f32;
    let actions_width = (deck_width - pad_x * 2.0).min(330.0);
    let line_height = theme::line_height(16.0, true);
    let primary_height = line_height + if short { 26.0 } else { 34.0 };
    let secondary_height = line_height + if short { 28.0 } else { 32.0 };
    let actions_height = primary_height + secondary_height * 4.0 + 48.0;
    let note_height = 19.5 * note_lines.len() as f32;
    let height = pad_top
        + 8.0
        + eyebrow_height
        + title_height
        + actions_height
        + note_height
        + 3.0 * gap
        + pad_bottom;
    let stage_y = -scroll.clamp(0.0, (height - viewport_height).max(0.0));
    let eyebrow_y = stage_y + pad_top + 8.0 + ((viewport_height - height).max(0.0) * 0.5);
    let stack_y = eyebrow_y + eyebrow_height + gap;
    let title_y = stack_y + letters_height;
    let subtitle_y = title_y + title_size + 17.0;
    let left = center - actions_width * 0.5;
    let actions_y = stack_y + title_height + gap;
    let mut actions = [Rect::default(); 5];
    let mut y = actions_y;
    for (index, rect) in actions.iter_mut().enumerate() {
        let height = if index == 0 {
            primary_height
        } else {
            secondary_height
        };
        *rect = Rect::new(left, y, actions_width, height);
        y += height + 12.0;
    }
    Menu {
        stage: Rect::new(
            (width - deck_width) * 0.5,
            stage_y,
            deck_width,
            height.max(viewport_height),
        ),
        actions,
        eyebrow_y,
        title_y,
        subtitle_y,
        note_y: actions_y + actions_height + gap,
        letters_y: (!short).then_some(stack_y),
        title_size,
        subtitle_lines,
        note_lines,
    }
}
