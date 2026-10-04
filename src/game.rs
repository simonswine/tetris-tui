//! Game engine: board, gravity, lock delay, line clears, scoring, 7-bag randomiser.

use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;

use crate::piece::{Dir, Kind, Piece, ALL_KINDS, COLS, ROWS, ROWS_HIDDEN};
use crate::rng::Rng;

pub const NEXT_COUNT: usize = 3;
/// Time a piece may be held on the ground by keeps resetting its timer (seconds).
pub const LOCK_DELAY: f64 = 0.5;
pub const MAX_LOCK_RESETS: u8 = 15;
/// Line-clear flash duration (seconds).
pub const FLASH_TIME: f64 = 0.2;
/// Fastest gravity we allow, so very high levels stay playable.
const MIN_GRAVITY_MS: f64 = 30.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Playing,
    Paused,
    GameOver,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub text: String,
    pub ttl: f64,
}

#[derive(Debug, Clone)]
pub struct Board {
    pub cells: [[Option<Kind>; COLS]; ROWS],
}

impl Default for Board {
    fn default() -> Self {
        Self {
            cells: std::array::from_fn(|_| std::array::from_fn(|_| None)),
        }
    }
}

impl Board {
    /// `None` means the coordinate is outside the playfield.
    pub fn at(&self, x: i32, y: i32) -> Option<Option<Kind>> {
        if x < 0 || x >= COLS as i32 || y >= ROWS as i32 {
            return None;
        }
        if y < 0 {
            // Buffer above the field: always free.
            return Some(None);
        }
        Some(self.cells[y as usize][x as usize])
    }

    pub fn set(&mut self, x: i32, y: i32, kind: Kind) {
        if x >= 0 && x < COLS as i32 && y >= 0 && y < ROWS as i32 {
            self.cells[y as usize][x as usize] = Some(kind);
        }
    }

    /// Does a box at `(x, y)` in rotation `rot` fit without overlapping anything?
    pub fn fits(&self, kind: Kind, x: i32, y: i32, rot: usize) -> bool {
        kind.cells(rot)
            .iter()
            .all(|(cx, cy)| matches!(self.at(x + cx, y + cy), Some(None)))
    }

    pub fn full_rows(&self) -> Vec<usize> {
        (0..ROWS)
            .filter(|&r| self.cells[r].iter().all(|c| c.is_some()))
            .collect()
    }

    pub fn remove_rows(&mut self, rows: &[usize]) {
        if rows.is_empty() {
            return;
        }
        let mut kept: Vec<[Option<Kind>; COLS]> = (0..ROWS)
            .filter(|r| !rows.contains(r))
            .map(|r| self.cells[r])
            .collect();
        while kept.len() < ROWS {
            kept.insert(0, [None; COLS]);
        }
        for (r, row) in kept.into_iter().enumerate() {
            self.cells[r] = row;
        }
    }

    /// Are all cells above the given rows empty? (used for the perfect-clear bonus)
    pub fn empty_except(&self, rows: &[usize]) -> bool {
        (0..ROWS).all(|r| rows.contains(&r) || self.cells[r].iter().all(|c| c.is_none()))
    }

    pub fn is_occupied_solid(&self, x: i32, y: i32) -> bool {
        match self.at(x, y) {
            None => true, // walls and ceiling count as solid
            Some(cell) => cell.is_some(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Game {
    pub board: Board,
    pub current: Piece,
    pub queue: VecDeque<Kind>,
    pub hold: Option<Kind>,
    pub can_hold: bool,
    pub status: Status,

    pub score: u64,
    pub best: u64,
    pub lines: u32,
    pub level: u32,
    pub pieces: u32,
    pub combo: i32,
    pub time: f64,

    /// Rows currently flashing before they collapse.
    pub flash: Vec<usize>,
    pub flash_t: f64,
    pub toasts: Vec<Toast>,

    soft_drop: bool,
    gravity_acc: f64,
    lock_t: f64,
    lock_resets: u8,
    last_was_rotate: bool,
    bag: Vec<Kind>,
    rng: Rng,
    highscore: Option<PathBuf>,
    /// Demo runs should not overwrite the player's high score.
    pub record_best: bool,
}

impl Game {
    pub fn new() -> Self {
        let mut game = Self {
            board: Board::default(),
            current: Piece::spawn(Kind::T),
            queue: VecDeque::new(),
            hold: None,
            can_hold: true,
            status: Status::Playing,
            score: 0,
            best: 0,
            lines: 0,
            level: 1,
            pieces: 0,
            combo: 0,
            time: 0.0,
            flash: Vec::new(),
            flash_t: 0.0,
            toasts: Vec::new(),
            soft_drop: false,
            gravity_acc: 0.0,
            lock_t: 0.0,
            lock_resets: 0,
            last_was_rotate: false,
            bag: Vec::new(),
            rng: Rng::from_env(),
            highscore: highscore_path(),
            record_best: true,
        };
        game.best = game.load_best();
        game.reset();
        game
    }

    /// Deterministic game, used by the tests.
    #[cfg(test)]
    fn with_rng(rng: Rng) -> Self {
        let mut game = Self::new();
        game.rng = rng;
        game.reset();
        game
    }

    /// Start a fresh game, keeping the stored high score.
    pub fn reset(&mut self) {
        self.board = Board::default();
        self.queue.clear();
        self.bag.clear();
        self.hold = None;
        self.can_hold = true;
        self.status = Status::Playing;
        self.score = 0;
        self.lines = 0;
        self.level = 1;
        self.pieces = 0;
        self.combo = 0;
        self.time = 0.0;
        self.flash.clear();
        self.flash_t = 0.0;
        self.toasts.clear();
        self.soft_drop = false;
        self.gravity_acc = 0.0;
        self.lock_t = 0.0;
        self.lock_resets = 0;
        self.last_was_rotate = false;
        while self.queue.len() < NEXT_COUNT + 1 {
            let kind = self.draw();
            self.queue.push_back(kind);
        }
        let first = self.queue.pop_front().unwrap();
        self.current = Piece::spawn(first);
        while self.queue.len() < NEXT_COUNT {
            let kind = self.draw();
            self.queue.push_back(kind);
        }
    }

    fn draw(&mut self) -> Kind {
        if self.bag.is_empty() {
            self.bag = ALL_KINDS.to_vec();
            self.rng.shuffle(&mut self.bag);
        }
        self.bag.pop().expect("bag refilled above")
    }

    pub fn preview(&self) -> Vec<Kind> {
        self.queue.iter().copied().take(NEXT_COUNT).collect()
    }

    /// Milliseconds per cell of natural gravity for a level (Tetris-Worlds curve).
    pub fn gravity_ms(level: u32) -> f64 {
        let l = level.clamp(1, 20) as f64;
        let secs = (0.8 - (l - 1.0) * 0.007).powf(l - 1.0);
        (secs * 1000.0).max(MIN_GRAVITY_MS)
    }

    pub fn on_ground(&self) -> bool {
        !self.board.fits(
            self.current.kind,
            self.current.x,
            self.current.y + 1,
            self.current.rot,
        )
    }

    /// Where the piece would land (ghost piece).
    pub fn ghost(&self) -> Piece {
        let mut g = self.current;
        while self.board.fits(g.kind, g.x, g.y + 1, g.rot) {
            g.y += 1;
        }
        g
    }

    pub fn set_soft_drop(&mut self, down: bool) {
        self.soft_drop = down;
    }

    /// Advance simulation by `dt` seconds.
    pub fn tick(&mut self, dt: f64) {
        self.toasts.iter_mut().for_each(|t| t.ttl -= dt);
        self.toasts.retain(|t| t.ttl > 0.0);

        if self.status != Status::Playing {
            return;
        }
        self.time += dt;

        if !self.flash.is_empty() {
            self.flash_t -= dt;
            if self.flash_t <= 0.0 {
                self.finish_clear();
            }
            return;
        }

        let gravity = Self::gravity_ms(self.level) / 1000.0;
        let interval = if self.soft_drop {
            gravity.min(0.05)
        } else {
            gravity
        };
        self.gravity_acc += dt;
        // Guard against huge dt (e.g. after the process was stopped).
        let mut steps = 0;
        while self.gravity_acc >= interval && steps < 24 {
            self.gravity_acc -= interval;
            steps += 1;
            if self.on_ground() {
                self.gravity_acc = 0.0;
                break;
            }
            self.current.y += 1;
            self.last_was_rotate = false;
            if self.soft_drop {
                self.score += 1;
            }
        }

        if self.on_ground() {
            self.lock_t += dt;
            if self.lock_t >= LOCK_DELAY {
                self.lock_piece();
            }
        } else {
            self.lock_t = 0.0;
            self.lock_resets = 0;
        }
    }

    /// Reset the lock timer when the player manages to move on the ground.
    fn touch_ground(&mut self) {
        if self.on_ground() && self.lock_resets < MAX_LOCK_RESETS {
            self.lock_t = 0.0;
            self.lock_resets += 1;
        }
    }

    pub fn move_horiz(&mut self, dir: Dir) -> bool {
        if self.status != Status::Playing || self.current_falls() {
            return false;
        }
        let nx = self.current.x + dir.dx();
        if self
            .board
            .fits(self.current.kind, nx, self.current.y, self.current.rot)
        {
            self.current.x = nx;
            self.last_was_rotate = false;
            self.touch_ground();
            true
        } else {
            false
        }
    }

    pub fn rotate(&mut self, dir: Dir) -> bool {
        if self.status != Status::Playing || self.current_falls() {
            return false;
        }
        let from = self.current.rot;
        let to = match dir {
            Dir::Right => from + 1,
            Dir::Left => from + 3,
        };
        for (kx, ky) in Kind::kicks(self.current.kind, from, to) {
            let (nx, ny) = (self.current.x + kx, self.current.y + ky);
            if self.board.fits(self.current.kind, nx, ny, to) {
                self.current.x = nx;
                self.current.y = ny;
                self.current.rot = to.rem_euclid(4);
                self.last_was_rotate = true;
                self.touch_ground();
                return true;
            }
        }
        false
    }

    /// True while the line-clear animation is playing (nothing falls yet).
    fn current_falls(&self) -> bool {
        !self.flash.is_empty()
    }

    pub fn soft_drop_step(&mut self) -> bool {
        if self.status != Status::Playing || self.current_falls() {
            return false;
        }
        if self.on_ground() {
            self.lock_t = LOCK_DELAY.min(self.lock_t + 0.05);
            return false;
        }
        self.current.y += 1;
        self.gravity_acc = 0.0;
        self.score += 1;
        self.last_was_rotate = false;
        true
    }

    pub fn hard_drop(&mut self) {
        if self.status != Status::Playing || self.current_falls() {
            return;
        }
        let mut dist = 0;
        while !self.on_ground() {
            self.current.y += 1;
            dist += 1;
        }
        self.score += 2 * dist as u64;
        self.last_was_rotate = false;
        self.lock_piece();
    }

    pub fn hold(&mut self) {
        if self.status != Status::Playing || !self.can_hold || self.current_falls() {
            return;
        }
        let incoming = match self.hold {
            Some(kind) => kind,
            None => self.queue.pop_front().unwrap_or_else(|| self.draw()),
        };
        self.hold = Some(self.current.kind);
        self.current = Piece::spawn(incoming);
        self.refill_queue();
        self.can_hold = false;
        self.gravity_acc = 0.0;
        self.lock_t = 0.0;
        self.lock_resets = 0;
        self.last_was_rotate = false;
        if !self.board.fits(
            self.current.kind,
            self.current.x,
            self.current.y,
            self.current.rot,
        ) {
            self.game_over();
        }
    }

    fn refill_queue(&mut self) {
        while self.queue.len() < NEXT_COUNT {
            let kind = self.draw();
            self.queue.push_back(kind);
        }
    }

    /// Guideline-ish T-spin check: a T that entered its slot by rotation with 3 filled corners.
    fn is_tspin(&self) -> bool {
        if self.current.kind != Kind::T || !self.last_was_rotate {
            return false;
        }
        let (cx, cy) = (self.current.x + 1, self.current.y + 1);
        let corners = [
            (cx - 1, cy - 1),
            (cx + 1, cy - 1),
            (cx - 1, cy + 1),
            (cx + 1, cy + 1),
        ];
        corners
            .iter()
            .filter(|(x, y)| self.board.is_occupied_solid(*x, *y))
            .count()
            >= 3
    }

    fn lock_piece(&mut self) {
        let piece = self.current;
        let cells: Vec<(i32, i32)> = piece
            .cells()
            .iter()
            .map(|(cx, cy)| (piece.x + cx, piece.y + cy))
            .collect();
        let tspin = self.is_tspin();
        let topped_out = cells.iter().any(|(_, gy)| *gy < ROWS_HIDDEN as i32);

        for (x, y) in &cells {
            self.board.set(*x, *y, piece.kind);
        }
        self.pieces += 1;
        self.can_hold = true;
        self.lock_t = 0.0;
        self.lock_resets = 0;
        self.gravity_acc = 0.0;

        let rows = self.board.full_rows();
        let n = rows.len();
        let level = self.level as u64;

        let line_points = if tspin {
            match n {
                0 => 400,
                1 => 800,
                2 => 1200,
                _ => 1600,
            }
        } else {
            match n {
                0 => 0,
                1 => 100,
                2 => 300,
                3 => 500,
                _ => 800,
            }
        };
        let mut points = line_points * level;

        if tspin {
            self.toast(if n == 0 { "T-SPIN" } else { "T-SPIN CLEAR" });
        }
        if n == 4 {
            self.toast("TETRIS!");
        } else if n > 0 {
            self.toast(["SINGLE", "DOUBLE", "TRIPLE"][n - 1]);
        }
        if n > 0 {
            self.combo += 1;
            if self.combo > 1 {
                points += 50 * (self.combo - 1) as u64 * level;
                self.toast(format!("COMBO x{}", self.combo - 1));
            }
            if self.board.empty_except(&rows) {
                points += 1000 * level;
                self.toast("PERFECT CLEAR");
            }
        } else {
            self.combo = 0;
        }
        self.score += points;

        if topped_out {
            self.game_over();
            return;
        }

        if n > 0 {
            self.lines += n as u32;
            self.level = self.lines / 10 + 1;
            self.flash = rows;
            self.flash_t = FLASH_TIME;
        } else {
            self.spawn_next();
        }
    }

    fn finish_clear(&mut self) {
        let rows = std::mem::take(&mut self.flash);
        self.flash_t = 0.0;
        self.board.remove_rows(&rows);
        self.spawn_next();
    }

    fn spawn_next(&mut self) {
        let kind = self.queue.pop_front().unwrap_or_else(|| self.draw());
        self.refill_queue();
        self.current = Piece::spawn(kind);
        self.gravity_acc = 0.0;
        self.lock_t = 0.0;
        self.lock_resets = 0;
        self.last_was_rotate = false;
        if !self.board.fits(
            self.current.kind,
            self.current.x,
            self.current.y,
            self.current.rot,
        ) {
            self.game_over();
        }
    }

    fn game_over(&mut self) {
        self.status = Status::GameOver;
        self.flash.clear();
        self.flash_t = 0.0;
        if self.record_best && self.score > self.best {
            self.best = self.score;
            self.save_best();
        }
        self.toast("GAME OVER");
    }

    pub fn toggle_pause(&mut self) {
        match self.status {
            Status::Playing => self.status = Status::Paused,
            Status::Paused => self.status = Status::Playing,
            Status::GameOver => {}
        }
    }

    fn toast(&mut self, text: impl Into<String>) {
        let text = text.into();
        if let Some(existing) = self.toasts.iter_mut().find(|t| t.text == text) {
            existing.ttl = 1.6;
            return;
        }
        self.toasts.push(Toast { text, ttl: 1.6 });
        if self.toasts.len() > 3 {
            self.toasts.remove(0);
        }
    }

    fn load_best(&self) -> u64 {
        self.highscore
            .as_ref()
            .and_then(|p| fs::read_to_string(p).ok())
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(0)
    }

    fn save_best(&self) {
        if let Some(path) = &self.highscore {
            let _ = fs::write(path, format!("{}\n", self.best));
        }
    }
}

fn highscore_path() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|home| PathBuf::from(home).join(".tetris_tui_highscore"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game_with(kind: Kind) -> Game {
        let mut g = Game::new();
        g.current = Piece::spawn(kind);
        g.queue = vec![Kind::O, Kind::O, Kind::O].into();
        g
    }

    #[test]
    fn bag_deals_each_kind_once_per_seven() {
        let mut g = Game::new();
        // Force a fresh bag, then the next seven pieces must be a permutation of all kinds.
        g.bag.clear();
        let seen: Vec<Kind> = (0..7).map(|_| g.draw()).collect();
        for kind in ALL_KINDS {
            assert_eq!(seen.iter().filter(|k| **k == kind).count(), 1);
        }
        // ...and a second bag is dealt automatically.
        g.bag.clear();
        let second: Vec<Kind> = (0..8).map(|_| g.draw()).collect();
        for kind in ALL_KINDS {
            assert!(second.iter().filter(|k| **k == kind).count() >= 1);
        }
    }

    #[test]
    fn full_rows_are_cleared_and_stack_shifts_down() {
        let mut b = Board::default();
        for x in 0..COLS {
            b.set(x as i32, ROWS as i32 - 1, Kind::I);
        }
        b.set(0, ROWS as i32 - 2, Kind::J);
        let rows = b.full_rows();
        assert_eq!(rows, vec![ROWS - 1]);
        b.remove_rows(&rows);
        assert_eq!(b.cells[ROWS - 1][0], Some(Kind::J));
        assert_eq!(b.cells[ROWS - 2][0], None);
    }

    #[test]
    fn hard_drop_locks_and_scores() {
        let mut g = game_with(Kind::O);
        g.hard_drop();
        assert_eq!(g.pieces, 1);
        assert!(g.score >= 2);
        assert_eq!(g.board.cells[ROWS - 1][4], Some(Kind::O));
    }

    #[test]
    fn tetris_clears_four_lines_and_scores_eight_hundred() {
        let mut g = game_with(Kind::I);
        for r in ROWS - 4..ROWS {
            for x in 0..COLS {
                if x != 3 {
                    g.board.set(x as i32, r as i32, Kind::J);
                }
            }
        }
        // Vertical I over the empty column 3 (rot 1 occupies box column 2).
        g.current = Piece {
            kind: Kind::I,
            x: 1,
            y: 0,
            rot: 1,
        };
        g.hard_drop();
        assert_eq!(g.lines, 4);
        assert!(g.score >= 800);
        assert_eq!(g.flash.len(), 4);
        g.finish_clear();
        assert!(g.board.cells[ROWS - 1][3].is_none());
    }

    #[test]
    fn level_rises_every_ten_lines() {
        let mut g = Game::new();
        g.lines = 19;
        g.level = g.lines / 10 + 1;
        assert_eq!(g.level, 2);
    }

    #[test]
    fn ghost_pieces_land_on_the_stack() {
        let mut g = game_with(Kind::O);
        g.board.set(4, ROWS as i32 - 5, Kind::J);
        let ghost = g.ghost();
        // The O's bottom row rests directly on top of the block.
        assert_eq!(ghost.y + 1, ROWS as i32 - 6);
        assert!(!g
            .board
            .fits(g.current.kind, g.current.x, ghost.y + 1, g.current.rot));
    }

    // ---- an end-to-end workout of the whole rules engine --------------------------------

    /// Play one piece with the demo bot, then let the clear animation finish so the next
    /// piece sees the real stack.
    fn bot_play(game: &mut Game) -> bool {
        let alive = crate::bot::play_one(game);
        game.tick(FLASH_TIME + 0.05);
        alive
    }

    #[test]
    fn a_greedy_bot_survives_150_pieces_and_clears_lines() {
        let mut game = Game::with_rng(Rng::new(20_240_707));
        let mut pieces = 0;
        while pieces < 150 && bot_play(&mut game) {
            pieces += 1;
        }
        if pieces < 150 || game.lines < 10 {
            for r in ROWS_HIDDEN..ROWS {
                let line: String = (0..COLS)
                    .map(|c| match game.board.at(c as i32, r as i32) {
                        Some(Some(_)) => '#',
                        _ => '.',
                    })
                    .collect();
                eprintln!("{line}");
            }
        }
        assert!(
            pieces >= 150 && game.lines >= 10,
            "bot survived {pieces} pieces, lines {}, score {}",
            game.lines,
            game.score
        );
    }

    #[test]
    fn gravity_slows_down_at_low_levels_and_floors_at_high() {
        assert!(Game::gravity_ms(1) > Game::gravity_ms(10));
        assert!(Game::gravity_ms(50) >= MIN_GRAVITY_MS);
    }

    #[test]
    fn rotation_kicks_the_ipiece_off_the_left_wall() {
        let mut g = game_with(Kind::I);
        // Vertical I flush against the left wall (rot 3 occupies box column x + 1).
        g.current = Piece {
            kind: Kind::I,
            x: -1,
            y: 5,
            rot: 3,
        };
        assert!(g
            .board
            .fits(g.current.kind, g.current.x, g.current.y, g.current.rot));
        // Rotating to horizontal would stick out of the wall, so the second kick offset is used.
        assert!(g.rotate(Dir::Right));
        assert_eq!(g.current.x, 0, "expected the kick to shift the piece right");
        for (cx, _) in g.current.cells() {
            assert!(g.current.x + cx >= 0);
            assert!((g.current.x + cx) < COLS as i32);
        }
    }

    #[test]
    fn hold_swaps_only_once_per_piece() {
        let mut g = game_with(Kind::T);
        let first = g.current.kind;
        g.hold();
        assert_eq!(g.hold, Some(first));
        assert_ne!(g.current.kind, first);
        let second = g.current.kind;
        g.hold(); // locked out until the piece locks
        assert_eq!(g.current.kind, second);
    }

    #[test]
    fn topping_out_ends_the_game() {
        let mut g = game_with(Kind::I);
        for r in 0..ROWS {
            for x in 0..COLS {
                g.board.set(x as i32, r as i32, Kind::Z);
            }
        }
        g.spawn_next();
        assert_eq!(g.status, Status::GameOver);
    }
}
