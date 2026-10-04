//! Keyboard input: translates crossterm key events into game actions.
//!
//! Two input paths:
//! * Terminals with the kitty keyboard protocol report press *and* release, so we run our own
//!   DAS/ARR (delayed auto shift / auto repeat) timers for crisp, configurable horizontal repeat.
//! * Plain terminals only deliver auto-repeated presses, so those are passed through as they
//!   come: the terminal decides how fast a held key repeats.

use std::time::Duration;

use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, ModifierKeyCode,
};

use crate::piece::Dir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Move(Dir),
    Rotate(Dir),
    SoftDrop,
    HardDrop,
    Hold,
    Pause,
    Restart,
    /// Enter: restart after game over.
    Confirm,
    Quit,
    /// Focus left the terminal (pause the game).
    FocusLost,
}

/// Delay before a held direction key starts repeating (seconds).
const DAS: f64 = 0.13;
/// Interval between repeats while a direction key is held (seconds).
const ARR: f64 = 0.035;
/// Delay before soft drop starts repeating.
const SOFT_DELAY: f64 = 0.1;
/// Interval between held soft drops.
const SOFT_ARR: f64 = 0.04;

pub struct Input {
    enhanced: bool,
    held: Option<Dir>,
    das_timer: f64,
    soft_held: bool,
    soft_timer: f64,
}

impl Input {
    pub fn new(enhanced: bool) -> Self {
        Self {
            enhanced,
            held: None,
            das_timer: 0.0,
            soft_held: false,
            soft_timer: 0.0,
        }
    }

    pub fn soft_active(&self) -> bool {
        self.soft_held
    }

    /// Drain every key event currently available.
    pub fn poll(&mut self) -> std::io::Result<Vec<Action>> {
        let mut actions = Vec::new();
        while event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Key(key) => actions.extend(self.handle_key(key)),
                Event::FocusLost => actions.push(Action::FocusLost),
                Event::FocusGained | Event::Mouse(_) | Event::Resize(..) => {}
                _ => {}
            }
        }
        Ok(actions)
    }

    fn handle_key(&mut self, key: KeyEvent) -> Vec<Action> {
        use KeyCode::{Char, Down, Enter, Esc, Left, Right, Up};

        match key.kind {
            KeyEventKind::Release => {
                match key.code {
                    Left | Right | Char('a') | Char('A') | Char('d') | Char('D') => {
                        self.held = None
                    }
                    Down | Char('s') | Char('S') => self.soft_held = false,
                    _ => {}
                }
                return Vec::new();
            }
            // The terminal's own auto-repeat; our timers handle repetition instead.
            KeyEventKind::Repeat => return Vec::new(),
            KeyEventKind::Press => {}
        }

        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, Char('c') | Char('C') | Char('q') | Char('Q'))
        {
            return vec![Action::Quit];
        }
        if !key.modifiers.is_empty() && !key.modifiers.contains(KeyModifiers::SHIFT) {
            return Vec::new();
        }

        let action = match key.code {
            Char('q') | Char('Q') | Esc => Some(Action::Quit),
            Char('p') | Char('P') => Some(Action::Pause),
            Char('r') | Char('R') => Some(Action::Restart),
            Enter => Some(Action::Confirm),
            Left | Char('a') | Char('A') => {
                self.held = Some(Dir::Left);
                self.das_timer = DAS;
                Some(Action::Move(Dir::Left))
            }
            Right | Char('d') | Char('D') => {
                self.held = Some(Dir::Right);
                self.das_timer = DAS;
                Some(Action::Move(Dir::Right))
            }
            Down | Char('s') | Char('S') => {
                self.soft_held = true;
                self.soft_timer = SOFT_DELAY;
                Some(Action::SoftDrop)
            }
            Up | Char('w') | Char('W') | Char('x') | Char('X') => Some(Action::Rotate(Dir::Right)),
            Char('z') | Char('Z') => Some(Action::Rotate(Dir::Left)),
            Char(' ') => Some(Action::HardDrop),
            Char('c')
            | Char('C')
            | KeyCode::Modifier(ModifierKeyCode::LeftShift | ModifierKeyCode::RightShift) => {
                Some(Action::Hold)
            }
            _ => None,
        };
        action.into_iter().collect()
    }

    /// Own auto-repeat, used when the terminal tells us about key releases.
    pub fn tick(&mut self, dt: f64) -> Vec<Action> {
        if !self.enhanced {
            return Vec::new();
        }
        let mut actions = Vec::new();
        if let Some(dir) = self.held {
            self.das_timer -= dt;
            while self.das_timer <= 0.0 {
                self.das_timer += ARR;
                actions.push(Action::Move(dir));
            }
        }
        if self.soft_held {
            self.soft_timer -= dt;
            while self.soft_timer <= 0.0 {
                self.soft_timer += SOFT_ARR;
                actions.push(Action::SoftDrop);
            }
        }
        actions
    }
}
