//! Tetris for the terminal — event loop and terminal lifecycle.

mod bot;
mod game;
mod input;
mod piece;
mod rng;
mod ui;

use std::error::Error;
use std::io::{self, Stdout, Write};
use std::panic;
use std::thread;
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, Show};
use crossterm::event::{
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use game::{Game, Status};
use input::{Action, Input};

/// ~30 FPS simulation/render tick.
const FRAME: Duration = Duration::from_millis(33);

type Tui = Terminal<CrosstermBackend<Stdout>>;

const HELP: &str = r#"tetris-tui - Tetris for the terminal

usage: tetris-tui [OPTIONS]

Options:
  -d, --demo    let the bot play itself (attract mode)
  -h, --help    show this help

Environment:
  TETRIS_SEED=<u64>   reproducible piece order

Controls: arrows/WASD move, z/x rotate, space hard drop, c hold, p pause, r restart, q quit
"#;

fn main() {
    let mut demo = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "-d" | "--demo" => demo = true,
            "-h" | "--help" => {
                print!("{HELP}");
                return;
            }
            other => {
                eprintln!("tetris-tui: unknown argument {other:?}\ntry --help");
                std::process::exit(2);
            }
        }
    }
    if let Err(err) = run(demo) {
        eprintln!("tetris-tui: {err}");
        std::process::exit(1);
    }
}

fn run(demo: bool) -> Result<(), Box<dyn Error>> {
    // Never leave the user with a broken terminal after a panic.
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = teardown();
        default_hook(info);
    }));

    let enhanced = terminal::supports_keyboard_enhancement().unwrap_or(false);
    let mut tui = setup(enhanced)?;
    let result = play(&mut tui, enhanced, demo);
    teardown()?;
    result
}

fn setup(enhanced: bool) -> Result<Tui, Box<dyn Error>> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    if enhanced {
        // Ask for press/release events; the game runs its own auto-repeat.
        execute!(
            stdout,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
            )
        )?;
    }
    execute!(stdout, EnterAlternateScreen, Hide)?;
    io::stdout().flush()?;
    let mut tui = Terminal::new(CrosstermBackend::new(stdout))?;
    tui.clear()?;
    Ok(tui)
}

fn teardown() -> io::Result<()> {
    let mut stdout = io::stdout();
    // Errors here are unavoidable when the pipe is gone; keep unwinding anyway.
    let _ = execute!(stdout, Show);
    let _ = execute!(stdout, PopKeyboardEnhancementFlags);
    let _ = execute!(stdout, LeaveAlternateScreen);
    let _ = stdout.flush();
    terminal::disable_raw_mode()
}

/// Seconds between two bot moves in demo mode.
const DEMO_STEP: f64 = 0.22;

fn play(tui: &mut Tui, enhanced: bool, demo: bool) -> Result<(), Box<dyn Error>> {
    let mut game = Game::new();
    // Watching the bot must not cost the player their high score.
    game.record_best = !demo;
    let mut input = Input::new(enhanced);
    let mut last = Instant::now();
    let mut demo_timer = DEMO_STEP;

    loop {
        let now = Instant::now();
        // Clamp so a suspended process does not teleport the piece after resume.
        let dt = now.duration_since(last).as_secs_f64().clamp(0.0, 0.25);
        last = now;

        let mut actions = input.poll()?;
        actions.extend(input.tick(dt));

        let mut quit = false;
        for action in actions {
            match action {
                Action::Move(dir) => {
                    game.move_horiz(dir);
                }
                Action::Rotate(dir) => {
                    game.rotate(dir);
                }
                Action::SoftDrop => {
                    game.soft_drop_step();
                }
                Action::HardDrop => game.hard_drop(),
                Action::Hold => game.hold(),
                Action::Pause => game.toggle_pause(),
                Action::Restart => game.reset(),
                Action::Confirm => {
                    if game.status == Status::GameOver {
                        game.reset();
                    }
                }
                Action::FocusLost => {
                    if game.status == Status::Playing {
                        game.toggle_pause();
                    }
                }
                Action::Quit => quit = true,
            }
        }
        if quit {
            break;
        }

        if demo {
            demo_timer -= dt;
            if demo_timer <= 0.0 {
                if game.status == Status::GameOver {
                    game.reset();
                } else if game.flash.is_empty() {
                    bot::play_one(&mut game);
                }
                demo_timer = DEMO_STEP;
            }
        }

        game.set_soft_drop(input.soft_active());
        game.tick(dt);

        tui.draw(|frame| ui::draw(frame, &game, demo))?;

        thread::sleep(FRAME.saturating_sub(now.elapsed()));
    }

    Ok(())
}
