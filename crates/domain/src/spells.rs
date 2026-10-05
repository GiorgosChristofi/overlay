//! Summoner spell ordering.

use serde::{Deserialize, Serialize};

pub const FLASH: i64 = 4;

/// Which key the player keeps Flash on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FlashKey {
    #[default]
    D,
    F,
}

/// Orders two spells so Flash lands on the player's preferred key.
pub fn arrange(spells: [i64; 2], flash_key: FlashKey) -> [i64; 2] {
    let [a, b] = spells;
    match (flash_key, a == FLASH, b == FLASH) {
        (FlashKey::D, false, true) | (FlashKey::F, true, false) => [b, a],
        _ => spells,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn puts_flash_on_preferred_key() {
        assert_eq!(arrange([14, FLASH], FlashKey::D), [FLASH, 14]);
        assert_eq!(arrange([FLASH, 14], FlashKey::F), [14, FLASH]);
        assert_eq!(arrange([FLASH, 14], FlashKey::D), [FLASH, 14]);
    }

    #[test]
    fn leaves_flashless_pairs_alone() {
        assert_eq!(arrange([11, 6], FlashKey::F), [11, 6]);
    }
}
