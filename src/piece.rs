//! Tetromino definitions: SRS shapes, rotation by bounding-box rotation, and SRS wall kicks.

use ratatui::style::Color;

/// Playfield width (Guideline standard).
pub const COLS: usize = 10;
/// Visible playfield height.
pub const ROWS_VISIBLE: usize = 20;
/// Hidden buffer rows above the visible field (spawn/rotation room).
pub const ROWS_HIDDEN: usize = 2;
pub const ROWS: usize = ROWS_VISIBLE + ROWS_HIDDEN;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    I,
    O,
    T,
    S,
    Z,
    J,
    L,
}

pub const ALL_KINDS: [Kind; 7] = [
    Kind::I,
    Kind::O,
    Kind::T,
    Kind::S,
    Kind::Z,
    Kind::J,
    Kind::L,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
}

impl Dir {
    pub fn dx(self) -> i32 {
        match self {
            Dir::Left => -1,
            Dir::Right => 1,
        }
    }
}

/// A falling tetromino. `(x, y)` is the top-left corner of its rotation box, in playfield
/// coordinates where `y` grows downwards.
#[derive(Debug, Clone, Copy)]
pub struct Piece {
    pub kind: Kind,
    pub x: i32,
    pub y: i32,
    /// 0 = spawn, 1 = R, 2 = 180, 3 = L
    pub rot: usize,
}

impl Piece {
    pub fn spawn(kind: Kind) -> Self {
        Self {
            kind,
            x: Kind::spawn_x(kind),
            y: Kind::spawn_y(kind),
            rot: 0,
        }
    }

    pub fn cells(&self) -> [(i32, i32); 4] {
        self.kind.cells(self.rot)
    }
}

/// Wall kick offsets in *screen* coordinates (y grows down), indexed by transition.
/// Order: 0→R, R→0, R→2, 2→R, 2→L, L→2, L→0, 0→L.
const KICKS_JLSTZ: [[(i32, i32); 5]; 8] = [
    [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],
    [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)],
    [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)],
    [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],
    [(0, 0), (1, 0), (1, 1), (0, 2), (1, 2)],
    [(0, 0), (-1, 0), (-1, -1), (0, -2), (-1, -2)],
    [(0, 0), (-1, 0), (-1, -1), (0, -2), (-1, -2)],
    [(0, 0), (1, 0), (1, 1), (0, 2), (1, 2)],
];

const KICKS_I: [[(i32, i32); 5]; 8] = [
    [(0, 0), (-2, 0), (1, 0), (-2, 1), (1, -2)],
    [(0, 0), (2, 0), (-1, 0), (2, -1), (-1, 2)],
    [(0, 0), (-1, 0), (2, 0), (-1, -2), (2, 1)],
    [(0, 0), (1, 0), (-2, 0), (1, 2), (-2, -1)],
    [(0, 0), (2, 0), (-1, 0), (2, -1), (-1, 2)],
    [(0, 0), (-2, 0), (1, 0), (-2, 1), (1, -2)],
    [(0, 0), (1, 0), (-2, 0), (1, 2), (-2, -1)],
    [(0, 0), (-1, 0), (2, 0), (-1, -2), (2, 1)],
];

const NO_KICK: [[(i32, i32); 5]; 8] = [[(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)]; 8];

impl Kind {
    pub fn color(self) -> Color {
        match self {
            Kind::I => Color::Indexed(51),  // cyan
            Kind::O => Color::Indexed(226), // yellow
            Kind::T => Color::Indexed(201), // magenta
            Kind::S => Color::Indexed(83),  // green
            Kind::Z => Color::Indexed(196), // red
            Kind::J => Color::Indexed(33),  // blue
            Kind::L => Color::Indexed(208), // orange
        }
    }

    /// Rotation box size (I uses 4x4, O uses 2x2 so rotation is a no-op).
    pub fn box_size(self) -> usize {
        match self {
            Kind::I => 4,
            Kind::O => 2,
            _ => 3,
        }
    }

    /// Occupied cells of the spawn (rotation 0) orientation, as `(x, y)` inside the box.
    fn base_cells(self) -> [(i32, i32); 4] {
        match self {
            Kind::I => [(0, 1), (1, 1), (2, 1), (3, 1)],
            Kind::O => [(0, 0), (1, 0), (0, 1), (1, 1)],
            Kind::T => [(1, 0), (0, 1), (1, 1), (2, 1)],
            Kind::S => [(1, 0), (2, 0), (0, 1), (1, 1)],
            Kind::Z => [(0, 0), (1, 0), (1, 1), (2, 1)],
            Kind::J => [(0, 0), (0, 1), (1, 1), (2, 1)],
            Kind::L => [(2, 0), (0, 1), (1, 1), (2, 1)],
        }
    }

    /// Cells of `self` in rotation state `rot`, relative to the box top-left.
    pub fn cells(self, rot: usize) -> [(i32, i32); 4] {
        let n = self.box_size() as i32;
        let mut pts = self.base_cells();
        for _ in 0..rot.rem_euclid(4) {
            for p in pts.iter_mut() {
                *p = (n - 1 - p.1, p.0); // clockwise
            }
        }
        pts
    }

    /// Column of the box top-left at spawn (pieces enter centred, left of centre preferred).
    pub fn spawn_x(kind: Kind) -> i32 {
        match kind {
            Kind::O => 4,
            _ => 3,
        }
    }

    /// Row of the box top-left at spawn. Part of the piece sits in the hidden buffer row and the
    /// rest shows on the top row of the visible field immediately.
    pub fn spawn_y(_kind: Kind) -> i32 {
        1
    }

    /// Wall kick candidates for a rotation, in `from -> to` order (screen coordinates).
    pub fn kicks(kind: Kind, from: usize, to: usize) -> &'static [(i32, i32)] {
        let from = from.rem_euclid(4);
        let to = to.rem_euclid(4);
        if kind == Kind::O || from == to {
            return &NO_KICK[0];
        }
        let idx = match (from, to) {
            (0, 1) => 0,
            (1, 0) => 1,
            (1, 2) => 2,
            (2, 1) => 3,
            (2, 3) => 4,
            (3, 2) => 5,
            (3, 0) => 6,
            (0, 3) => 7,
            _ => return &NO_KICK[0],
        };
        match kind {
            Kind::I => &KICKS_I[idx],
            _ => &KICKS_JLSTZ[idx],
        }
    }

    /// Bounding box `(min_x, min_y, w, h)` of the piece in rotation `rot` — used for previews.
    pub fn bounds(self, rot: usize) -> (i32, i32, i32, i32) {
        let cells = self.cells(rot);
        let min_x = cells.iter().map(|c| c.0).min().unwrap();
        let max_x = cells.iter().map(|c| c.0).max().unwrap();
        let min_y = cells.iter().map(|c| c.1).min().unwrap();
        let max_y = cells.iter().map(|c| c.1).max().unwrap();
        (min_x, min_y, max_x - min_x + 1, max_y - min_y + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_four_cells_per_rotation() {
        for kind in ALL_KINDS {
            for rot in 0..4 {
                let cells = kind.cells(rot);
                assert_eq!(cells.len(), 4);
                for (i, a) in cells.iter().enumerate() {
                    for b in cells.iter().skip(i + 1) {
                        assert_ne!(a, b, "{kind:?} rot {rot} has duplicate cells");
                    }
                    assert!(a.0 >= 0 && a.1 >= 0, "{kind:?} rot {rot} leaves the box");
                    assert!((a.0 as usize) < kind.box_size());
                    assert!((a.1 as usize) < kind.box_size());
                }
            }
        }
    }

    #[test]
    fn four_rotations_return_to_start() {
        for kind in ALL_KINDS {
            assert_eq!(kind.cells(0), kind.cells(4), "{kind:?}");
        }
    }

    #[test]
    fn o_rotation_does_not_move_it() {
        let sorted = |rot: usize| {
            let mut cells = Kind::O.cells(rot).to_vec();
            cells.sort();
            cells
        };
        assert_eq!(sorted(0), sorted(1));
        assert_eq!(sorted(0), sorted(2));
        assert_eq!(sorted(0), sorted(3));
    }

    #[test]
    fn i_states_match_srs_layout() {
        // rot 0 -> row 1, rot R -> column 2, rot 2 -> row 2, rot L -> column 1
        assert!(Kind::I.cells(0).iter().all(|c| c.1 == 1));
        assert!(Kind::I.cells(1).iter().all(|c| c.0 == 2));
        assert!(Kind::I.cells(2).iter().all(|c| c.1 == 2));
        assert!(Kind::I.cells(3).iter().all(|c| c.0 == 1));
    }

    #[test]
    fn spawn_pieces_are_inside_the_field() {
        for kind in ALL_KINDS {
            let p = Piece::spawn(kind);
            for (cx, cy) in p.cells() {
                let gx = p.x + cx;
                let gy = p.y + cy;
                assert!(gx >= 0 && gx < COLS as i32, "{kind:?} spawns off-field");
                assert!(gy >= 0 && gy < ROWS as i32);
            }
        }
    }
}
