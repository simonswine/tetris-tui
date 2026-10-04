//! A tiny greedy player, used by `--demo` and by the engine tests.
//!
//! It is deliberately simple: try all four rotations in every column, score the stack that
//! results and keep the best. That is enough to survive a long game and to keep the board
//! interesting to watch.

use crate::game::{Board, Game};
use crate::piece::{Dir, COLS, ROWS};

/// What the bot wants to do with the current piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    /// Number of clockwise rotations to apply.
    pub rotations: usize,
    /// Box column to slide the piece to before dropping.
    pub column: i32,
}

/// Weighting for the stack heuristics.
fn evaluate(board: &Board) -> i64 {
    let filled = |c: usize, r: usize| matches!(board.at(c as i32, r as i32), Some(Some(_)));
    let mut heights = [0i64; COLS];
    let mut holes = 0i64;
    for (c, height) in heights.iter_mut().enumerate() {
        // A hole is an empty cell with a filled cell above and below it; the air on top of the
        // stack does not count.
        let Some(top) = (0..ROWS).find(|&r| filled(c, r)) else {
            continue;
        };
        *height = (ROWS - top) as i64;
        holes += (top..ROWS).filter(|&r| !filled(c, r)).count() as i64;
    }
    let aggregate: i64 = heights.iter().sum();
    let bumpiness: i64 = heights
        .windows(2)
        .map(|pair| (pair[0] - pair[1]).abs())
        .sum();
    let completed = board.full_rows().len() as i64;
    completed * 40 - aggregate * 4 - holes * 6 - bumpiness
}

/// Best placement for the piece in play, or `None` when nothing can be placed.
pub fn plan(game: &Game) -> Option<Plan> {
    let mut best: Option<(i64, Plan)> = None;
    for rotations in 0..4 {
        let mut probe = game.clone();
        for _ in 0..rotations {
            probe.rotate(Dir::Right);
        }
        // Walk the piece across the whole field and score every place it could land.
        while probe.move_horiz(Dir::Left) {}
        for _ in 0..COLS {
            let landing = probe.ghost();
            let mut stacked = probe.board.clone();
            for (cx, cy) in landing.cells() {
                stacked.set(landing.x + cx, landing.y + cy, landing.kind);
            }
            let score = evaluate(&stacked);
            let candidate = Plan {
                rotations,
                column: probe.current.x,
            };
            match &best {
                Some((previous, _)) if *previous >= score => {}
                _ => best = Some((score, candidate)),
            }
            if !probe.move_horiz(Dir::Right) {
                break;
            }
        }
    }
    best.map(|(_, plan)| plan)
}

/// Apply a plan to the game and drop. Returns true while the game is playable.
pub fn play_one(game: &mut Game) -> bool {
    let Some(plan) = plan(game) else {
        return false;
    };
    for _ in 0..plan.rotations {
        game.rotate(Dir::Right);
    }
    while game.current.x != plan.column {
        let direction = if plan.column > game.current.x {
            Dir::Right
        } else {
            Dir::Left
        };
        if !game.move_horiz(direction) {
            break;
        }
    }
    game.hard_drop();
    game.status == crate::game::Status::Playing
}
