//! Native speech-rate calibration and ordered optional Sonic effects.
// Copyright (C) 2005-2011 Jonathan Duddington; Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::voice::Voice;

const MINIMUM: i32 = 80;
const NORMAL: i32 = 175;
const MAXIMUM: i32 = 450;
// Calibration from setlengths.c; the retained C oracle checks every entry.
const LOOKUP: [u8; 280] = [
    255, 255, 255, 255, 255, 253, 249, 245, 242, 238, 235, 232, 228, 225, 222, 218, 216, 213, 210,
    207, 204, 201, 198, 196, 193, 191, 188, 186, 183, 181, 179, 176, 174, 172, 169, 168, 165, 163,
    161, 159, 158, 155, 153, 152, 150, 148, 146, 145, 143, 141, 139, 137, 136, 135, 133, 131, 130,
    129, 127, 126, 124, 123, 122, 120, 119, 118, 117, 115, 114, 113, 112, 111, 110, 109, 107, 106,
    105, 104, 103, 102, 101, 100, 99, 98, 97, 96, 95, 94, 93, 92, 91, 90, 89, 89, 88, 87, 86, 85,
    84, 83, 82, 82, 81, 80, 80, 79, 78, 77, 76, 76, 75, 75, 74, 73, 72, 71, 71, 70, 69, 69, 68, 67,
    67, 66, 66, 65, 64, 64, 63, 62, 62, 61, 61, 60, 59, 59, 58, 58, 57, 57, 56, 56, 55, 54, 54, 53,
    53, 52, 52, 52, 51, 50, 50, 49, 49, 48, 48, 47, 47, 46, 46, 46, 45, 45, 44, 44, 44, 43, 43, 42,
    41, 40, 40, 40, 39, 39, 39, 38, 38, 38, 37, 37, 37, 36, 36, 35, 35, 35, 35, 34, 34, 34, 33, 33,
    33, 32, 32, 31, 31, 31, 30, 30, 30, 29, 29, 29, 29, 28, 28, 27, 27, 27, 27, 26, 26, 26, 26, 25,
    25, 25, 24, 24, 24, 24, 23, 23, 23, 23, 22, 22, 22, 21, 21, 21, 21, 20, 20, 20, 20, 19, 19, 19,
    18, 18, 17, 17, 17, 16, 16, 16, 16, 16, 16, 15, 15, 15, 15, 14, 14, 14, 13, 13, 13, 12, 12, 12,
    12, 11, 11, 11, 11, 10, 10, 10, 9, 9, 9, 8, 8, 8,
];
const PAUSE: [u8; 25] = [
    22, 22, 22, 22, 22, 22, 22, 21, 21, 21, 21, 20, 20, 19, 19, 18, 17, 16, 15, 15, 15, 15, 15, 15,
    15,
];
const WAVE: [u8; 101] = [
    120, 121, 120, 119, 119, 118, 118, 117, 116, 116, 115, 114, 113, 112, 112, 111, 111, 110, 109,
    108, 107, 106, 106, 104, 103, 103, 102, 102, 102, 101, 101, 99, 98, 98, 97, 96, 96, 95, 94, 93,
    91, 90, 91, 90, 89, 88, 86, 85, 86, 85, 85, 84, 82, 81, 80, 79, 77, 78, 78, 76, 77, 75, 75, 74,
    73, 71, 72, 70, 69, 69, 69, 67, 65, 64, 63, 63, 63, 61, 61, 59, 59, 59, 58, 56, 57, 58, 56, 54,
    53, 52, 52, 53, 52, 52, 50, 48, 47, 47, 45, 46, 45,
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct Factors {
    pub pause: i32,
    pub clause_pause: i32,
    pub minimum_pause: u32,
    pub wave: i32,
    pub length_modifier: i32,
    pub length_modifier2: i32,
    pub minimum_sample: i32,
    pub fast_settings: i32,
}
/// Effect values use the compatibility Sonic multiplier scale of 1024.
/// Deliver the initialized prefix, in order, after committing the new state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct SonicEffects {
    pub count: u32,
    pub values: [i32; 2],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct State {
    pub factors: Factors,
    pub lengths: [i32; 3],
}
impl Default for State {
    fn default() -> Self {
        Self {
            factors: Factors::default(),
            lengths: [130, 121, 118],
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Control,
    Arithmetic,
}

fn multiply(left: i32, right: i32, divisor: i32) -> Result<i32, Error> {
    left.checked_mul(right)
        .map(|value| value / divisor)
        .ok_or(Error::Arithmetic)
}
impl State {
    /// Compute without allocation, I/O, callbacks or global state. Control 1
    /// updates translation lengths, 2 synthesis factors using the secondary
    /// rate, 3 both using the primary rate; 0 only resets common defaults.
    /// Invalid controls/arithmetic preserve the previous snapshot and emit no
    /// effects. Call from serialized owner/worker work after asset completion.
    pub fn configure(
        &mut self,
        voice: &Voice,
        primary: i32,
        secondary: i32,
        control: u32,
        sonic: bool,
    ) -> Result<SonicEffects, Error> {
        if control > 3 {
            return Err(Error::Control);
        }
        let mut next = *self;
        let factors = &mut next.factors;
        factors.minimum_sample = MAXIMUM;
        factors.length_modifier = 110;
        factors.length_modifier2 = 100;
        factors.minimum_pause = 5;
        let original = if control == 2 { secondary } else { primary };
        let mut rate = original;
        if voice.speed_percent > 0 {
            rate = multiply(rate, voice.speed_percent, 100)?;
        }
        let mut effects = SonicEffects::default();
        if sonic && control & 2 != 0 {
            effects.values[0] = 1024;
            effects.count = 1;
        }
        if sonic && (original > MAXIMUM || (original > factors.fast_settings && rate > 350)) {
            if control & 1 != 0 {
                next.lengths = [
                    multiply(73, voice.speed1, 256)?,
                    multiply(73, voice.speed2, 256)?,
                    multiply(73, voice.speed3, 256)?,
                ];
            }
            if control & 2 != 0 {
                // Match the C double calculation and truncation exactly. Reject
                // out-of-range conversion rather than relying on C undefined behavior.
                let multiplier = f64::from(rate) / f64::from(NORMAL) * 1024.0;
                if multiplier < f64::from(i32::MIN) || multiplier >= f64::from(i32::MAX) + 1.0 {
                    return Err(Error::Arithmetic);
                }
                effects.values[1] = multiplier as i32;
                effects.count = 2;
                factors.pause = 85;
                factors.clause_pause = MINIMUM;
                factors.minimum_pause = 22;
                factors.minimum_sample = MAXIMUM * 2;
                factors.wave = 211;
                factors.length_modifier = 210;
                factors.length_modifier2 = 170;
            }
            *self = next;
            return Ok(effects);
        }
        rate = rate.min(MAXIMUM);
        let mut factor = i32::from(LOOKUP[(rate.clamp(MINIMUM, 359) - MINIMUM) as usize]);
        if rate >= 380 {
            factor = 7;
        }
        if rate >= 400 {
            factor = 6;
        }
        if control & 1 != 0 {
            next.lengths = [
                multiply(factor, voice.speed1, 256)?,
                multiply(factor, voice.speed2, 256)?,
                multiply(factor, voice.speed3, 256)?,
            ];
            if factor <= 7 {
                next.lengths = [factor, factor - 1, factor - 1];
            }
        }
        if control & 2 != 0 {
            if rate > 350 {
                factors.length_modifier = 85 - (rate - 350) / 3;
                factors.length_modifier2 = 60 - (rate - 350) / 8;
            } else if rate > 250 {
                factors.length_modifier = 110 - (rate - 250) / 4;
                factors.length_modifier2 = 110 - (rate - 250) / 2;
            }
            let syllable = multiply(factor, voice.speed1, 256)?;
            factors.wave = if rate >= 170 {
                110_i32
                    .checked_add(multiply(150, syllable, 128)?)
                    .ok_or(Error::Arithmetic)?
            } else {
                128_i32
                    .checked_add(multiply(128, syllable, 130)?)
                    .ok_or(Error::Arithmetic)?
            };
            if rate >= 350 {
                factors.wave = i32::from(WAVE[(rate - 350) as usize]);
            }
            if rate >= 390 {
                factors.minimum_sample = MAXIMUM - (rate - 400) / 2;
                if rate > 440 {
                    factors.minimum_sample = 420 - (rate - 440);
                }
            }
            factors.pause = multiply(256, syllable, 115)?;
            if rate > 430 {
                factors.pause = 12;
            } else if rate > 400 {
                factors.pause = 13;
            } else if rate > 374 {
                factors.pause = 14;
            } else if rate > 350 {
                factors.pause = i32::from(PAUSE[(rate - 350) as usize]);
            }
            factors.clause_pause = factors.pause.max(16);
        }
        *self = next;
        Ok(effects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn voice() -> Voice {
        Voice {
            speed1: 256,
            speed2: 238,
            speed3: 232,
            ..Voice::default()
        }
    }
    #[test]
    fn controls_keep_unselected_fields_and_use_the_correct_rate() {
        let mut state = State::default();
        state.factors.wave = 123;
        state.factors.pause = 234;
        state.configure(&voice(), 175, 400, 1, false).unwrap();
        assert_eq!(state.lengths, [87, 80, 78]);
        assert_eq!(state.factors.wave, 123);
        let lengths = state.lengths;
        state.configure(&voice(), 175, 400, 2, false).unwrap();
        assert_eq!(state.lengths, lengths);
        assert_eq!(state.factors.wave, 85);
        state.configure(&voice(), 175, 400, 3, false).unwrap();
        assert_eq!(state.factors.wave, 211);
        assert_eq!(state.lengths, lengths);
    }
    #[test]
    fn sonic_effects_are_ordered_and_high_speed_parameters_are_distinct() {
        let mut state = State::default();
        state.factors.fast_settings = 450;
        assert_eq!(
            state.configure(&voice(), 600, 80, 3, true).unwrap(),
            SonicEffects {
                count: 2,
                values: [1024, 3510]
            }
        );
        assert_eq!(state.lengths, [73, 67, 66]);
        assert_eq!(state.factors.minimum_pause, 22);
        assert_eq!(
            state.configure(&voice(), 80, 80, 2, true).unwrap(),
            SonicEffects {
                count: 1,
                values: [1024, 0]
            }
        );
        assert_eq!(
            state.configure(&voice(), 600, 80, 1, true).unwrap(),
            SonicEffects::default()
        );
    }
    #[test]
    fn invalid_control_and_overflow_preserve_snapshots() {
        let mut state = State::default();
        let previous = state;
        assert_eq!(
            state.configure(&voice(), 175, 175, 4, false),
            Err(Error::Control)
        );
        assert_eq!(state, previous);
        let mut extreme = voice();
        extreme.speed_percent = 100;
        assert_eq!(
            state.configure(&extreme, i32::MAX, 175, 3, false),
            Err(Error::Arithmetic)
        );
        assert_eq!(state, previous);
        extreme.speed_percent = 0;
        extreme.speed2 = i32::MAX;
        assert_eq!(
            state.configure(&extreme, 175, 175, 3, false),
            Err(Error::Arithmetic)
        );
        assert_eq!(state, previous);
        extreme.speed2 = 238;
        assert_eq!(
            state.configure(&extreme, i32::MAX, 175, 3, true),
            Err(Error::Arithmetic)
        );
        assert_eq!(state, previous);
    }
}
