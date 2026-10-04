//! Rendering: playfield, hold/next panels, stats, overlays.

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Gauge, Paragraph, Widget};
use ratatui::Frame;

use crate::game::{Game, Status};
use crate::piece::{Kind, ROWS_HIDDEN, ROWS_VISIBLE};

/// Smallest terminal we can lay out without clipping.
pub const MIN_WIDTH: u16 = 60;
pub const MIN_HEIGHT: u16 = 26;

const COLS: u16 = 10;
const CELL: &str = "█";
const GHOST: &str = "░";
const DOT: &str = "·";
const BG: Color = Color::Rgb(12, 12, 16);

pub fn draw(f: &mut Frame, game: &Game, demo: bool) {
    let area = f.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        too_small(f, area);
        return;
    }

    let root = center(area, MIN_WIDTH, MIN_HEIGHT);
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(ROWS_VISIBLE as u16 + 2),
        Constraint::Length(2),
    ])
    .areas(root);

    let [left_col, _gap_l, board_area, _gap_r, right_col] = Layout::horizontal([
        Constraint::Length(17),
        Constraint::Length(2),
        Constraint::Length(ROWS_VISIBLE as u16 + 2),
        Constraint::Length(2),
        Constraint::Length(17),
    ])
    .areas(body);

    draw_header(f, header, game, demo);
    draw_footer(f, footer);

    let [hold_area, _, stats_area] = Layout::vertical([
        Constraint::Length(6),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .areas(left_col);
    draw_hold(f, hold_area, game);
    draw_stats(f, stats_area, game);

    let [next_area, _, score_area] = Layout::vertical([
        Constraint::Length(13),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .areas(right_col);
    draw_next(f, next_area, game);
    draw_scoring(f, score_area);

    draw_board(f, board_area, game);

    match game.status {
        Status::Playing => {}
        Status::Paused => draw_overlay(f, root, " PAUSED ", vec![], "press p to resume"),
        Status::GameOver => draw_overlay(
            f,
            root,
            " GAME OVER ",
            vec![
                Line::from(format!("score  {}", format_score(game.score))),
                Line::from(format!("lines  {}   pieces  {}", game.lines, game.pieces)),
                Line::from(format!("best   {}", format_score(game.best))),
            ],
            "r restart · q quit",
        ),
    }
}

// ------------------------------------------------------------------ helpers

fn center(area: Rect, w: u16, h: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(w) / 2,
        y: area.y + area.height.saturating_sub(h) / 2,
        width: w.min(area.width),
        height: h.min(area.height),
    }
}

fn panel(title: &str) -> Block<'static> {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .style(Style::default().bg(BG));
    if title.is_empty() {
        block
    } else {
        block.title(Line::from(Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::BOLD),
        )))
    }
}

fn piece_style(kind: Kind) -> Style {
    Style::default()
        .fg(kind.color())
        .bg(BG)
        .add_modifier(Modifier::BOLD)
}

/// Draw one playfield cell as two terminal columns. Out-of-bounds writes are ignored.
fn put_cell(buf: &mut Buffer, x: u16, y: u16, style: Style) {
    put_block(buf, x, y, style, CELL);
}

fn put_block(buf: &mut Buffer, x: u16, y: u16, style: Style, symbol: &str) {
    for dx in 0..2u16 {
        if let Some(cell) = buf.cell_mut(Position::new(x + dx, y)) {
            cell.set_symbol(symbol).set_style(style);
        }
    }
}

fn put_dot(buf: &mut Buffer, x: u16, y: u16) {
    let style = Style::default().fg(Color::Rgb(46, 46, 58)).bg(BG);
    for dx in 0..2u16 {
        if let Some(cell) = buf.cell_mut(Position::new(x + dx, y)) {
            cell.set_symbol(DOT).set_style(style);
        }
    }
}

/// Playfield coordinates -> screen coordinates (visible rows only).
fn to_screen(inner: Rect, x: i32, y: i32) -> Option<(u16, u16)> {
    let row = y - ROWS_HIDDEN as i32;
    if x < 0 || x >= COLS as i32 || row < 0 || row >= ROWS_VISIBLE as i32 {
        return None;
    }
    Some((inner.x + x as u16 * 2, inner.y + row as u16))
}

// -------------------------------------------------------------------- panels

fn draw_header(f: &mut Frame, area: Rect, game: &Game, demo: bool) {
    let title = Line::from(vec![
        Span::styled(
            "T E T R I S",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled(
            format!("level {}", game.level),
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    let subtitle = Line::from(Span::styled(
        if demo {
            "attract mode — q quits"
        } else {
            "terminal edition"
        },
        Style::default().fg(Color::DarkGray),
    ));
    f.render_widget(
        Paragraph::new(vec![title, subtitle]).alignment(Alignment::Center),
        area,
    );
}

fn key() -> Style {
    Style::default().fg(Color::Black).bg(Color::Gray)
}

fn hint() -> Style {
    Style::default().fg(Color::DarkGray)
}

fn draw_footer(f: &mut Frame, area: Rect) {
    let line = |parts: &[(&str, bool)]| {
        Line::from(
            parts
                .iter()
                .map(|(text, is_key)| {
                    Span::styled((*text).to_string(), if *is_key { key() } else { hint() })
                })
                .collect::<Vec<_>>(),
        )
    };
    let lines = vec![
        line(&[
            (" ← → ", true),
            ("move ", false),
            (" ↑ x ", true),
            ("rotate ", false),
            (" z ", true),
            ("ccw ", false),
            (" space ", true),
            ("drop ", false),
            (" ↓ ", true),
            ("soft", false),
        ]),
        line(&[
            (" c ", true),
            ("hold ", false),
            (" p ", true),
            ("pause ", false),
            (" r ", true),
            ("restart ", false),
            (" q ", true),
            ("quit", false),
        ]),
    ];
    f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), area);
}

fn draw_board(f: &mut Frame, area: Rect, game: &Game) {
    let block = panel("");
    let inner = block.inner(area);
    block.render(area, f.buffer_mut());

    // Empty grid.
    for r in 0..ROWS_VISIBLE as u16 {
        for c in 0..COLS {
            put_dot(f.buffer_mut(), inner.x + c * 2, inner.y + r);
        }
    }

    // Locked stack (rows being cleared flash white).
    for r in 0..ROWS_VISIBLE {
        let row = ROWS_HIDDEN + r;
        let flashing = game.flash.contains(&row);
        for c in 0..COLS as usize {
            if let Some(kind) = game.board.cells[row][c] {
                let style = if flashing {
                    flash_style(game.flash_t)
                } else {
                    piece_style(kind)
                };
                put_cell(
                    f.buffer_mut(),
                    inner.x + c as u16 * 2,
                    inner.y + r as u16,
                    style,
                );
            }
        }
    }

    // Ghost + falling piece.
    if game.flash.is_empty() {
        let ghost = game.ghost();
        let ghost_style = Style::default()
            .fg(ghost.kind.color())
            .bg(BG)
            .add_modifier(Modifier::DIM);
        for (cx, cy) in ghost.cells() {
            if let Some((sx, sy)) = to_screen(inner, ghost.x + cx, ghost.y + cy) {
                put_block(f.buffer_mut(), sx, sy, ghost_style, GHOST);
            }
        }
        for (cx, cy) in game.current.cells() {
            if let Some((sx, sy)) = to_screen(inner, game.current.x + cx, game.current.y + cy) {
                put_cell(f.buffer_mut(), sx, sy, piece_style(game.current.kind));
            }
        }
    }

    // Scoring toasts float over the top of the field.
    if !game.toasts.is_empty() {
        let lines: Vec<Line> = game
            .toasts
            .iter()
            .map(|t| {
                Line::from(Span::styled(
                    t.text.clone(),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ))
            })
            .collect();
        let height = (game.toasts.len() as u16).min(inner.height.saturating_sub(1));
        f.render_widget(
            Paragraph::new(lines).alignment(Alignment::Center),
            Rect {
                y: inner.y,
                height,
                ..inner
            },
        );
    }
}

fn flash_style(t: f64) -> Style {
    let color = if t > 0.12 {
        Color::White
    } else if t > 0.06 {
        Color::Rgb(220, 220, 160)
    } else {
        Color::Rgb(150, 150, 110)
    };
    Style::default()
        .fg(color)
        .bg(BG)
        .add_modifier(Modifier::BOLD)
}

fn draw_hold(f: &mut Frame, area: Rect, game: &Game) {
    let block = panel("HOLD");
    let inner = block.inner(area);
    block.render(area, f.buffer_mut());
    let Some(kind) = game.hold else { return };
    let style = if game.can_hold {
        piece_style(kind)
    } else {
        Style::default()
            .fg(kind.color())
            .bg(BG)
            .add_modifier(Modifier::DIM)
    };
    draw_mini(f.buffer_mut(), inner, kind, style);
}

fn draw_next(f: &mut Frame, area: Rect, game: &Game) {
    let block = panel("NEXT");
    let inner = block.inner(area);
    block.render(area, f.buffer_mut());
    let previews = game.preview();
    if previews.is_empty() {
        return;
    }
    let slots = Layout::vertical(vec![
        Constraint::Ratio(1, previews.len() as u32);
        previews.len()
    ])
    .split(inner);
    for (kind, slot) in previews.iter().zip(slots.iter().copied()) {
        draw_mini(f.buffer_mut(), slot, *kind, piece_style(*kind));
    }
}

/// Draw a tetromino centred inside `area`, one row per cell row, two columns per cell.
fn draw_mini(buf: &mut Buffer, area: Rect, kind: Kind, style: Style) {
    let (min_x, min_y, w, h) = kind.bounds(0);
    let ox = area.x + area.width.saturating_sub(w as u16 * 2) / 2;
    let oy = area.y + area.height.saturating_sub(h as u16) / 2;
    for (cx, cy) in kind.cells(0) {
        let x = ox + (cx - min_x) as u16 * 2;
        let y = oy + (cy - min_y) as u16;
        if x + 1 < area.x + area.width && y < area.y + area.height {
            put_cell(buf, x, y, style);
        }
    }
}

fn label(label: &str, value: String, style: Style) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<8}"), Style::default().fg(Color::DarkGray)),
        Span::styled(value, style),
    ])
}

fn draw_stats(f: &mut Frame, area: Rect, game: &Game) {
    let block = panel("STATS");
    let inner = block.inner(area);
    block.render(area, f.buffer_mut());

    let num = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);
    let lines = vec![
        label("Score", format_points(game.score), num),
        label(
            "Best",
            format_points(game.best),
            Style::default().fg(Color::Yellow),
        ),
        label("Lines", game.lines.to_string(), num),
        label("Level", game.level.to_string(), num),
        label("Pieces", game.pieces.to_string(), num),
        label("Time", format_time(game.time), num),
        label(
            "Combo",
            if game.combo > 1 {
                format!("x{}", game.combo - 1)
            } else {
                "-".to_string()
            },
            Style::default().fg(Color::Cyan),
        ),
    ];
    f.render_widget(Paragraph::new(lines), inner);

    let progress = game.lines % 10;
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::Rgb(32, 32, 44)))
        .ratio(progress as f64 / 10.0)
        .label(format!("level {}  {progress}/10", game.level))
        .use_unicode(true);
    let gauge_area = Rect {
        y: inner.y + inner.height.saturating_sub(1),
        height: 1,
        ..inner
    };
    f.render_widget(gauge, gauge_area);
}

fn draw_scoring(f: &mut Frame, area: Rect) {
    let block = panel("SCORING");
    let inner = block.inner(area);
    block.render(area, f.buffer_mut());
    let rows = [
        ("Tetris", "800"),
        ("Triple", "500"),
        ("Double", "300"),
        ("Single", "100"),
        ("T-Spin", "400+"),
        ("Perfect", "1000"),
    ];
    let lines: Vec<Line> = rows
        .iter()
        .map(|(what, pts)| {
            Line::from(vec![
                Span::styled(format!("{what:<8}"), Style::default().fg(Color::Gray)),
                Span::styled((*pts).to_string(), Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_overlay(f: &mut Frame, area: Rect, title: &str, body: Vec<Line<'static>>, footer: &str) {
    let width = 34u16.min(area.width.saturating_sub(2));
    let height = (body.len() as u16 + 6).min(area.height);
    let area = center(area, width, height);
    f.render_widget(Clear, area);

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(Line::from(Span::styled(
            title,
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )))
        .style(Style::default().bg(BG))
        .border_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(area);
    block.render(area, f.buffer_mut());

    let mut lines = body;
    lines.push(Line::from(String::new()));
    lines.push(Line::from(Span::styled(
        footer.to_string(),
        Style::default().fg(Color::DarkGray),
    )));
    f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), inner);
}

fn too_small(f: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(Span::styled(
            "terminal too small",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!(
                "need at least {MIN_WIDTH}x{MIN_HEIGHT}, have {}x{}",
                area.width, area.height
            ),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "resize the window · q quits",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    f.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center),
        center(area, area.width, 3),
    );
}

pub fn format_score(score: u64) -> String {
    let digits = score.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// Score for the narrow side panel: never wider than the panel itself.
pub fn format_points(points: u64) -> String {
    match points {
        0..=999_999 => format_score(points),
        1_000_000..=999_999_999 => format!("{:.2}M", points as f64 / 1e6),
        _ => format!("{:.2}G", points as f64 / 1e9),
    }
}

fn format_time(secs: f64) -> String {
    let total = secs.max(0.0).floor() as u64;
    format!("{}:{:02}", total / 60, total % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::piece::ROWS;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    /// Render one frame off-screen and return it as plain text, one line per row.
    fn render_lines(game: &Game, width: u16, height: u16) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, game, false)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[Position::new(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn a_full_clearing_row_renders_as_one_solid_line() {
        let mut game = Game::new();
        let bottom = ROWS - 1;
        for c in 0..COLS as usize {
            game.board.set(c as i32, bottom as i32, Kind::Z);
        }
        game.flash = vec![bottom];
        game.flash_t = 0.2;
        let lines = render_lines(&game, 70, 30);
        let solid: String = "█".repeat(COLS as usize * 2);
        assert!(
            lines.iter().any(|line| line.contains(&solid)),
            "expected a solid row of cleared cells:\n{}",
            lines.join("\n")
        );
    }

    #[test]
    fn overlay_text_is_shown_on_top_of_the_field() {
        let mut game = Game::new();
        game.status = Status::GameOver;
        let lines = render_lines(&game, 70, 30);
        assert!(lines.iter().any(|l| l.contains("GAME OVER")));
        assert!(lines.iter().any(|l| l.contains("restart")));
    }

    #[test]
    fn small_terminals_get_an_explanation() {
        let game = Game::new();
        let lines = render_lines(&game, 30, 10);
        assert!(lines.iter().any(|l| l.contains("too small")));
    }

    #[test]
    fn tick_advances_the_clock_only_while_playing() {
        let mut game = Game::new();
        game.tick(1.0);
        assert!(game.time > 0.0);
        game.status = Status::Paused;
        let before = game.time;
        game.tick(1.0);
        assert_eq!(game.time, before);
    }

    #[test]
    fn thousands_separators() {
        assert_eq!(format_score(0), "0");
        assert_eq!(format_score(999), "999");
        assert_eq!(format_score(1000), "1,000");
        assert_eq!(format_score(1234567), "1,234,567");
    }

    #[test]
    fn big_scores_stay_inside_the_panel() {
        assert_eq!(format_points(15_904), "15,904");
        assert_eq!(format_points(1_234_567), "1.23M");
        assert_eq!(format_points(12_345_678_901), "12.35G");

        let mut game = Game::new();
        game.score = 15_904;
        game.best = 1_234_567;
        let lines = render_lines(&game, 70, 30);
        assert!(
            lines.iter().any(|l| l.contains("15,904")) && lines.iter().any(|l| l.contains("1.23M")),
            "stats were clipped:\n{}",
            lines.join("\n")
        );
    }

    #[test]
    fn time_is_mm_ss() {
        assert_eq!(format_time(0.0), "0:00");
        assert_eq!(format_time(65.9), "1:05");
    }

    #[test]
    fn hidden_rows_do_not_render() {
        let inner = Rect {
            x: 4,
            y: 2,
            width: 20,
            height: 20,
        };
        assert_eq!(to_screen(inner, 0, (ROWS_HIDDEN - 1) as i32), None);
        assert_eq!(to_screen(inner, 0, ROWS_HIDDEN as i32), Some((4, 2)));
        assert_eq!(to_screen(inner, -1, ROWS_HIDDEN as i32), None);
        assert_eq!(to_screen(inner, 10, ROWS_HIDDEN as i32), None);
    }
}
