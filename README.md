# tetris-tui

Tetris for the terminal, built with [ratatui](https://ratatui.rs) + crossterm. Guideline rules,
SRS rotation with wall kicks, 7-bag randomiser, hold, ghost piece, lock delay and configurable
auto-repeat — no dependencies beyond the TUI stack.

```
                         T E T R I S   level 3
                           terminal edition
       ╭ HOLD ───────╮  ╭────────────────────╮  ╭ NEXT ───────╮
       │             │  │░░░░░░░░░░░░░░░░░░░░│  │             │
       │   ██        │  │░░░░░░░░░░░░░░░░░░░░│  │   ██        │
       │   ██████    │  │░░░░░░░░░░░░░░░░░░░░│  │   ██████    │
       ╰─────────────╯  │░░░░░░░░░░░░░░░░░░░░│  │             │
       ╭ STATS ──────╮  │░░░░░░▓▓▓▓░░░░░░░░░░│  │     ██      │
       │Score 12,480 │  │░░░░░░▓▓░░░░░░░░░░░░│  │   ██████    │
       │Best  40,120 │  │░░░░░░░░░░░░░░░░░░░░│  │             │
       │Lines     27 │  │░░░░██░░░░░░░░░░░░░░│  │  ████████   │
       │Level      3 │  │░░░░░░░░░░░░░░░░░░░░│  ╰─────────────╯
       │Pieces    31 │  │░░░░░░░░░░░░░░░░░░░░│
       │Time    2:14 │  │░░░░░░░░░░░░░░░░░░░░│  ╭ SCORING ────╮
       │Combo   x2   │  │░░░░░░░░░░░░░░░░░░░░│  │Tetris  800  │
       │             │  │░░░░░░░░░░░░░░░░░░░░│  │Triple  500  │
       │             │  │░░░░░░▒▒░░░░░░░░░░░░│  │Double  300  │
       │level 3  7/10│  │░░░░░░▒▒▒▒▒▒░░░░░░░░│  │Single  100  │
       ╰─────────────╯  ╰────────────────────╯  ╰─────────────╯
            ← → move  ↑ x rotate  z ccw  space drop  ↓ soft
                   c hold  p pause  r restart  q quit
```

## Build and run

```sh
cargo run --release            # play
cargo run --release -- --demo  # watch the built-in bot play itself
cargo test                     # engine + rendering tests
scripts/smoke-test.sh          # drives the real TUI in a tmux pty and prints frames
```

`--help` lists the flags; `--demo` never writes your high score.

Requirements: a terminal at least **60x26** (smaller shows a hint instead of a broken layout) and
256-colour support; truecolour is not needed.

## Controls

| Key | Action |
| --- | --- |
| `←` / `→` (or `a` / `d`) | move (DAS 130 ms, ARR 35 ms) |
| `↑` / `x` / `w` | rotate clockwise |
| `z` | rotate counter-clockwise |
| `↓` / `s` | soft drop (+1 per cell) |
| `space` | hard drop (+2 per cell) |
| `c` / `Shift` | hold |
| `p` | pause — also auto-pauses when the terminal loses focus |
| `r` | restart (`Enter` also restarts after game over) |
| `q` / `Esc` / `Ctrl-C` | quit |

## Rules implemented

* 10x20 visible field with two hidden buffer rows; pieces spawn centred and enter visibly.
* Super Rotation System: bounding-box rotation plus the official JLSTZ / I wall-kick tables.
* 7-bag randomiser, three-piece preview, one hold swap per piece.
* 500 ms lock delay with up to 15 move-resets, ghost piece, line-clear flash animation.
* Gravity follows the classic `(0.8 - (level-1)*0.007)^(level-1)` seconds-per-row curve,
  level = lines/10 + 1, floored at 30 ms/row so high levels stay playable.
* Scoring: 100/300/500/800 x level, T-spin 400-1600, combo 50 x combo x level,
  perfect clear 1000 x level.
* High score is stored in `~/.tetris_tui_highscore`.

## Configuration

| Variable | Meaning |
| --- | --- |
| `TETRIS_SEED` | seed the randomiser for a reproducible piece order (bug reports, tests) |

## Input details

At startup the game asks the terminal whether it speaks the
[kitty keyboard protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/). If it does, key
press/release events drive the game's own delayed-auto-shift timers, so horizontal movement feels
identical everywhere. On plain terminals the fallback is the terminal's own key auto-repeat.

## Source layout

| File | Contents |
| --- | --- |
| `src/main.rs` | terminal setup/teardown, ~30 FPS event loop |
| `src/game.rs` | board, gravity, lock delay, line clears, scoring, high score |
| `src/piece.rs` | tetromino shapes, SRS rotation and kick tables |
| `src/input.rs` | key events to actions, DAS/ARR handling |
| `src/ui.rs` | all drawing: field, panels, overlays |
| `src/rng.rs` | SplitMix64, so no `rand` dependency |
| `src/bot.rs` | the greedy player behind `--demo` (and the long-run engine test) |

`.cargo/config.toml` works around a broken bundled `ld.lld` wrapper on the machine this was built
on; delete it if your Rust toolchain links normally.
