//! Tiny dependency-free RNG (SplitMix64) so the game needs no crates beyond the TUI stack.

#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Honours `TETRIS_SEED` so runs can be reproduced (useful for tests and bug reports).
    pub fn from_env() -> Self {
        match std::env::var("TETRIS_SEED") {
            Ok(seed) if !seed.is_empty() => match seed.parse::<u64>() {
                Ok(value) => Self::new(value),
                Err(_) => {
                    eprintln!("tetris-tui: ignoring invalid TETRIS_SEED={seed:?}");
                    Self::from_time()
                }
            },
            _ => Self::from_time(),
        }
    }

    pub fn from_time() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x2545_F491_4F6C_DD1D);
        Self::new(nanos ^ (std::process::id() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform value in `0..n` (modulo bias is irrelevant here).
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.next_u64() % n as u64) as usize
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i + 1);
            items.swap(i, j);
        }
    }
}
