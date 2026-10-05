//! Bounded pitch, formant and amplitude calibration on explicit snapshots.
// Copyright (C) 2005-2013 Jonathan Duddington, 2015-2016 Reece H. Dunn;
// Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::voice::Voice;
#[path = "synthesis_data.rs"]
mod data;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Arithmetic,
    PitchIndex,
    EmphasisIndex,
    Split,
    Capacity,
    SampleAlignment,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct Pitch {
    pub base: i32,
    pub range: i32,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct Embedded {
    pub pitch: i32,
    pub tone: i32,
    pub range: i32,
}
fn mul(a: i32, b: i32) -> Result<i32, Error> {
    a.checked_mul(b).ok_or(Error::Arithmetic)
}
fn add(a: i32, b: i32) -> Result<i32, Error> {
    a.checked_add(b).ok_or(Error::Arithmetic)
}
fn sub(a: i32, b: i32) -> Result<i32, Error> {
    a.checked_sub(b).ok_or(Error::Arithmetic)
}
/// Shared waveform/MBROLA pitch math, preserving the C intermediate order and
/// truncating division. Invalid indices or overflow fail before publication.
pub fn pitch(
    voice: &Voice,
    mut first: i32,
    mut second: i32,
    embedded: Embedded,
) -> Result<Pitch, Error> {
    if first > second {
        std::mem::swap(&mut first, &mut second);
    }
    let value = sub(embedded.pitch.min(101), embedded.tone)?.max(0);
    let factor = *data::PITCH_ADJUST
        .get(value as usize)
        .ok_or(Error::PitchIndex)? as i32;
    let mut base = mul(voice.pitch_base, factor)? / 128;
    let range = mul(voice.pitch_range, embedded.range)? / 50;
    base = sub(base, mul(sub(range, voice.pitch_range)?, 18)?)?;
    let first = add(base, mul(first, range)? / 2)?;
    let second = add(base, mul(second, range)? / 2)?;
    Ok(Pitch {
        base: first,
        range: sub(second, first)?,
    })
}
/// Plan all six formant frequencies and two heights before mutating the voice.
pub fn pitch_formants(voice: &mut Voice, pitch: i32, tone: i32) -> Result<(), Error> {
    let pitch = pitch.min(101);
    let factor = if pitch > 50 {
        add(256, mul(25, sub(pitch, 50)?)? / 50)?
    } else {
        256
    };
    let mut frequency = [0; 6];
    for (i, value) in frequency.iter_mut().enumerate() {
        *value = (mul(i32::from(voice.base_frequency[i]), factor)? / 256) as i16;
    }
    let factor = mul(tone, 3)?;
    let height0 = (mul(i32::from(voice.base_height[0]), sub(256, mul(factor, 2)?)?)? / 256) as i16;
    let height1 = (mul(i32::from(voice.base_height[1]), sub(256, factor)?)? / 256) as i16;
    voice.frequency[..6].copy_from_slice(&frequency);
    voice.height[0] = height0;
    voice.height[1] = height1;
    Ok(())
}
pub fn general_amplitude(amplitude: i32, emphasis: usize) -> Result<i32, Error> {
    let factor = *data::AMP_EMPHASIS
        .get(emphasis)
        .ok_or(Error::EmphasisIndex)? as i32;
    let amplitude = mul(amplitude, 55)? / 100;
    Ok(mul(amplitude, factor)? / 16)
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct Amplitude {
    pub increment: i32,
    pub value: i32,
    pub voiced: i32,
}
pub fn amplitude(
    length: i32,
    value: i32,
    general: i32,
    consonant: i32,
) -> Result<Amplitude, Error> {
    let increment = if length == 0 {
        0
    } else {
        (256 * 128 * 64) / length
    };
    let value = mul(value, general)? / 16;
    let voiced = mul(mul(value, consonant)?, 15)? / 100;
    Ok(Amplitude {
        increment,
        value,
        voiced,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pitch_and_amplitude_keep_order_and_reject_invalid_snapshots() {
        let voice = Voice {
            pitch_base: 12800,
            pitch_range: 100,
            ..Default::default()
        };
        let embedding = Embedded {
            pitch: 50,
            tone: 0,
            range: 50,
        };
        assert_eq!(
            pitch(&voice, 10, 0, embedding).unwrap(),
            Pitch {
                base: 12800,
                range: 500
            }
        );
        assert_eq!(general_amplitude(100, 4).unwrap(), 75);
        assert_eq!(
            amplitude(0, 16, 60, 40).unwrap(),
            Amplitude {
                increment: 0,
                value: 60,
                voiced: 360
            }
        );
        assert_eq!(
            pitch(
                &voice,
                0,
                1,
                Embedded {
                    pitch: 101,
                    tone: -1,
                    range: 50
                }
            ),
            Err(Error::PitchIndex)
        );
        assert_eq!(general_amplitude(10, 5), Err(Error::EmphasisIndex));
        assert_eq!(amplitude(1, i32::MAX, 60, 40), Err(Error::Arithmetic));
    }
    #[test]
    fn formant_plan_preserves_voice_on_overflow() {
        let mut voice = Voice::default();
        voice.base_frequency[..6].fill(256);
        voice.base_height[..2].fill(256);
        pitch_formants(&mut voice, 100, 2).unwrap();
        assert_eq!(&voice.frequency[..6], &[281; 6]);
        assert_eq!(&voice.height[..2], &[244, 250]);
        let before = voice;
        assert_eq!(
            pitch_formants(&mut voice, 100, i32::MAX),
            Err(Error::Arithmetic)
        );
        assert_eq!(voice, before);
    }
}
