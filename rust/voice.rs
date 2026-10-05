//! Native acoustic voice configuration on borrowed setup bytes.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn;
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
use crate::phoneme_data::InvalidPhonemeData as Error;

pub const PEAKS: usize = 9;
pub const TONE_BINS: usize = 1000;
fn space(byte: &u8) -> bool {
    matches!(byte, b'\t'..=b'\r' | b' ')
}
pub const DEFAULT_TONE: [i32; 12] = [600, 170, 1200, 135, 2000, 110, 3000, 110, -1, 0, 0, 0];

/// Compatible acoustic snapshot; names/table selection belong to the owner.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Voice {
    pub name: [u8; 40],
    pub language: [u8; 20],
    pub phoneme_table: i32,
    pub pitch_base: i32,
    pub pitch_range: i32,
    pub speed1: i32,
    pub speed2: i32,
    pub speed3: i32,
    pub speed_percent: i32,
    pub flutter: i32,
    pub roughness: i32,
    pub echo_delay: i32,
    pub echo_amplitude: i32,
    pub harmonic_peaks: i32,
    pub peak_shape: i32,
    pub voicing: i32,
    pub formant_factor: i32,
    pub consonant_amplitude: i32,
    pub voiced_consonant_amplitude: i32,
    pub sample_rate: i32,
    pub klatt: [i32; 8],
    pub frequency: [i16; PEAKS],
    pub height: [i16; PEAKS],
    pub width: [i16; PEAKS],
    pub frequency_add: [i16; PEAKS],
    pub base_frequency: [i16; PEAKS],
    pub base_height: [i16; PEAKS],
    pub breath: [i32; PEAKS],
    pub breath_width: [i32; PEAKS],
    pub tone: [u8; TONE_BINS],
}
impl Default for Voice {
    fn default() -> Self {
        Self {
            name: [0; 40],
            language: [0; 20],
            phoneme_table: 0,
            pitch_base: 0,
            pitch_range: 0,
            speed1: 0,
            speed2: 0,
            speed3: 0,
            speed_percent: 0,
            flutter: 0,
            roughness: 0,
            echo_delay: 0,
            echo_amplitude: 0,
            harmonic_peaks: 0,
            peak_shape: 0,
            voicing: 0,
            formant_factor: 0,
            consonant_amplitude: 0,
            voiced_consonant_amplitude: 0,
            sample_rate: 0,
            klatt: [0; 8],
            frequency: [0; PEAKS],
            height: [0; PEAKS],
            width: [0; PEAKS],
            frequency_add: [0; PEAKS],
            base_frequency: [0; PEAKS],
            base_height: [0; PEAKS],
            breath: [0; PEAKS],
            breath_width: [0; PEAKS],
            tone: [0; TONE_BINS],
        }
    }
}
/// `%d` prefix parsing with bounded bytes and checked integer range.
/// Empty input returns -1, matching the retained scanner's EOF assignment count.
pub fn numbers<const N: usize>(bytes: &[u8]) -> Result<([i32; N], i32), Error> {
    numbers_limit(bytes, N)
}
/// One `%d` conversion and its consumed byte count, including leading space.
/// Returning the cursor permits following `%s` conversions without discarding
/// a nonnumeric suffix of an otherwise valid integer.
pub(crate) fn decimal(bytes: &[u8]) -> Result<Option<(i32, usize)>, Error> {
    let mut cursor = 0;
    while bytes.get(cursor).is_some_and(space) {
        cursor += 1;
    }
    let negative = bytes.get(cursor) == Some(&b'-');
    if bytes.get(cursor).is_some_and(|c| matches!(c, b'-' | b'+')) {
        cursor += 1;
    }
    let start = cursor;
    let mut magnitude = 0_i64;
    while let Some(digit) = bytes.get(cursor).filter(|c| c.is_ascii_digit()) {
        magnitude = magnitude
            .checked_mul(10)
            .and_then(|n| n.checked_add(i64::from(*digit - b'0')))
            .ok_or(Error("voice integer overflow"))?;
        cursor += 1;
    }
    if start == cursor {
        return Ok(None);
    }
    let value = i32::try_from(if negative { -magnitude } else { magnitude })
        .map_err(|_| Error("voice integer overflow"))?;
    Ok(Some((value, cursor)))
}
fn numbers_limit<const N: usize>(bytes: &[u8], limit: usize) -> Result<([i32; N], i32), Error> {
    let mut values = [0; N];
    let mut cursor = 0;
    let mut assigned = 0;
    for value in &mut values[..limit] {
        while bytes.get(cursor).is_some_and(space) {
            cursor += 1;
        }
        if bytes.get(cursor).is_none_or(|c| *c == 0) {
            if assigned == 0 {
                assigned = -1;
            }
            break;
        }
        let Some((number, consumed)) = decimal(&bytes[cursor..])? else {
            break;
        };
        *value = number;
        cursor += consumed;
        assigned += 1;
    }
    Ok((values, assigned))
}
pub fn tone_points(bytes: &[u8]) -> Result<[i32; 12], Error> {
    let (values, count) = numbers::<10>(bytes)?;
    let mut points = [-1; 12];
    let count = count.max(0) as usize;
    points[..count].copy_from_slice(&values[..count]);
    Ok(points)
}
impl Voice {
    /// Snapshot acoustic settings for native formant transitions.
    pub fn formant_settings(
        &self,
        which: i32,
        other_glottal: bool,
        length_adjust: i32,
    ) -> crate::formant::Settings {
        crate::formant::Settings {
            which,
            klatt: u32::from(self.klatt[0] != 0),
            formant_factor: self.formant_factor,
            other_glottal: u32::from(other_glottal),
            length_adjust,
        }
    }
    /// Validate and build the entire curve before changing any caller storage.
    pub fn set_tone(&mut self, points: &mut [i32; 12]) -> Result<(), Error> {
        let mut staged = *points;
        let mut curve = self.tone;
        let mut first = 0;
        let mut height = staged[1];
        for pair in (0..12).step_by(2) {
            if staged[pair] == -1 {
                staged[pair] = (TONE_BINS * 8) as i32;
                if pair > 0 {
                    staged[pair + 1] = staged[pair - 1];
                }
            }
            let second = staged[pair] / 8;
            if !(0..=TONE_BINS as i32).contains(&second) {
                return Err(Error("voice tone frequency outside 0..=8000"));
            }
            let next_height = staged[pair + 1];
            if second > first {
                let difference = next_height
                    .checked_sub(height)
                    .ok_or(Error("voice tone height overflow"))?;
                for index in first..second {
                    let delta = (index - first)
                        .checked_mul(difference)
                        .ok_or(Error("voice tone interpolation overflow"))?
                        / (second - first);
                    let value = height
                        .checked_add(delta)
                        .ok_or(Error("voice tone interpolation overflow"))?;
                    curve[index as usize] = value.min(255) as u8;
                }
            }
            first = second;
            height = next_height;
        }
        self.tone = curve;
        *points = staged;
        Ok(())
    }
    /// Reset acoustic fields. Returns the maximum fast setting and rate snapshot;
    /// backend breath/phoneme-replacement resets remain explicit owner effects.
    pub fn reset(
        &mut self,
        sample_rate: i32,
        points: &mut [i32; 12],
    ) -> Result<(i32, [i32; PEAKS]), Error> {
        if sample_rate <= 0 {
            return Err(Error("voice sample rate must be positive"));
        }
        let mut next = *self;
        let mut staged_points = *points;
        next.pitch_base = 0x47000;
        next.pitch_range = 4104;
        next.formant_factor = 256;
        next.speed_percent = 100;
        next.echo_delay = 0;
        next.echo_amplitude = 0;
        next.flutter = 64;
        next.harmonic_peaks = 5;
        next.peak_shape = 0;
        next.voicing = 64;
        next.consonant_amplitude = 90;
        next.voiced_consonant_amplitude = 100;
        next.sample_rate = sample_rate;
        next.klatt = [0; 8];
        next.roughness = 2;
        next.frequency = [256; PEAKS];
        next.base_frequency = next.frequency;
        next.height = [260, 256, 240, 232, 200, 200, 256, 256, 256];
        next.base_height = next.height;
        next.width = [280, 256, 256, 320, 342, 342, 256, 256, 256];
        next.width[0] = next.width[0] * 105 / 100;
        next.breath = [0; PEAKS];
        next.breath_width = [0, 200, 200, 400, 400, 400, 600, 600, 600];
        next.frequency_add = [0; PEAKS];
        let rates = std::array::from_fn(|i| (if i == 0 { 240 } else { 170 }) * 22050 / sample_rate);
        next.set_tone(&mut staged_points)?;
        next.speed1 = 256;
        next.speed2 = 238;
        next.speed3 = 232;
        *self = next;
        *points = staged_points;
        Ok((450, rates))
    }
    /// Apply one acoustic directive. None means it belongs to another setup
    /// layer; Some(true) asks the owner to recompute its speed state.
    /// Rejected arithmetic/curves leave voice and fast settings unchanged.
    pub fn apply(
        &mut self,
        keyword: &[u8],
        bytes: &[u8],
        klatt_enabled: bool,
        fast: &mut i32,
    ) -> Result<Option<bool>, Error> {
        let maximum = match keyword {
            b"formant" => 5,
            b"pitch" | b"echo" | b"consonants" => 2,
            b"tone" => 10,
            b"breath" | b"breathw" | b"klatt" => 8,
            b"flutter" | b"roughness" | b"clarity" | b"voicing" | b"speed" | b"fast_test2" => 1,
            _ => return Ok(None),
        };
        if keyword == b"klatt" && !klatt_enabled {
            return Ok(None);
        }
        let (values, count) = numbers_limit::<10>(bytes, maximum as usize)?;
        let count = count.min(maximum);
        let mut next = *self;
        let mut next_fast = *fast;
        let mut speed = false;
        match keyword {
            b"formant" if count >= 2 && (0..=8).contains(&values[0]) => {
                let index = values[0] as usize;
                for (input, output) in [
                    (1, &mut next.frequency),
                    (2, &mut next.height),
                    (3, &mut next.width),
                ] {
                    let value = if count > input {
                        values[input as usize]
                    } else {
                        100
                    };
                    if value >= 0 {
                        let scaled = f64::from(value) * 2.56001;
                        if scaled.trunc() > f64::from(i32::MAX) {
                            return Err(Error("voice formant scale overflow"));
                        }
                        output[index] = scaled as i32 as i16;
                    }
                }
                if values[1] >= 0 {
                    next.base_frequency[index] = next.frequency[index];
                }
                if count <= 2 || values[2] >= 0 {
                    next.base_height[index] = next.height[index];
                }
                next.frequency_add[index] = values[4] as i16;
                if index == 0 {
                    next.width[0] = (i32::from(next.width[0]) * 105 / 100) as i16;
                }
            }
            b"pitch" if count == 2 => {
                let first = values[0]
                    .checked_sub(9)
                    .and_then(|n| n.checked_mul(4096))
                    .ok_or(Error("voice pitch overflow"))?;
                next.pitch_base = first;
                next.pitch_range = values[1]
                    .checked_sub(values[0])
                    .and_then(|n| n.checked_mul(108))
                    .ok_or(Error("voice pitch range overflow"))?;
                let factor = f64::from(
                    values[0]
                        .checked_sub(82)
                        .ok_or(Error("voice pitch overflow"))?,
                ) / 82.0;
                next.formant_factor = ((1.0 + factor / 4.0) * 256.0) as i32;
            }
            b"echo" => {
                next.echo_amplitude = 0;
                if count > 0 {
                    next.echo_delay = values[0];
                }
                if count > 1 {
                    next.echo_amplitude = values[1];
                }
            }
            b"flutter" if count == 1 => {
                next.flutter = values[0]
                    .checked_mul(32)
                    .ok_or(Error("voice flutter overflow"))?
            }
            b"roughness" if count == 1 => next.roughness = values[0],
            b"clarity" if count == 1 => {
                let mut value = values[0];
                if value > 4 {
                    next.peak_shape = 1;
                    value = 4;
                }
                next.harmonic_peaks = value
                    .checked_add(1)
                    .ok_or(Error("voice clarity overflow"))?;
            }
            b"tone" => next.set_tone(&mut tone_points(bytes)?)?,
            b"voicing" if count == 1 => {
                next.voicing = values[0]
                    .checked_mul(64)
                    .ok_or(Error("voice voicing overflow"))?
                    / 100
            }
            b"breath" | b"breathw" => {
                let array = if keyword == b"breath" {
                    &mut next.breath
                } else {
                    &mut next.breath_width
                };
                array[0] = count;
                array[1..].copy_from_slice(&values[..8]);
                if keyword == b"breath" {
                    for index in [1, 3, 5, 7] {
                        array[index] = array[index]
                            .checked_neg()
                            .ok_or(Error("voice breath overflow"))?;
                    }
                }
            }
            b"klatt" => {
                next.klatt.copy_from_slice(&values[..8]);
                next.klatt[5] = next.klatt[5]
                    .checked_sub(40)
                    .ok_or(Error("voice Klatt overflow"))?;
            }
            b"consonants" => {
                if count > 0 {
                    next.consonant_amplitude = values[0];
                }
                if count > 1 {
                    next.voiced_consonant_amplitude = values[1];
                }
            }
            b"speed" => {
                if count == 1 {
                    next.speed_percent = values[0];
                }
                speed = true;
            }
            b"fast_test2" => {
                if count == 1 {
                    next_fast = values[0];
                }
                speed = true;
            }
            _ => {}
        }
        *self = next;
        *fast = next_fast;
        Ok(Some(speed))
    }
}

/// Borrowed fgets-style directive chunks. Width includes the terminating NUL
/// used by C (normally N_PATH_BUF); parsing itself does no I/O or allocation.
pub struct Directives<'a> {
    bytes: &'a [u8],
    width: usize,
}
impl<'a> Directives<'a> {
    pub fn new(bytes: &'a [u8], width: usize) -> Result<Self, Error> {
        if !(2..=4096).contains(&width) {
            return Err(Error("voice line width must be 2..=4096"));
        }
        Ok(Self { bytes, width })
    }
}
impl<'a> Iterator for Directives<'a> {
    type Item = (&'a [u8], &'a [u8]);
    fn next(&mut self) -> Option<Self::Item> {
        while !self.bytes.is_empty() {
            let limit = self.bytes.len().min(self.width - 1);
            let count = self.bytes[..limit]
                .iter()
                .position(|b| *b == b'\n')
                .map_or(limit, |i| i + 1);
            let mut line = &self.bytes[..count];
            self.bytes = &self.bytes[count..];
            line = &line[..line.iter().position(|b| *b == 0).unwrap_or(line.len())];
            if line.first() == Some(&b'#') {
                continue;
            }
            while line.len() > 1 && line.last().is_some_and(space) {
                line = &line[..line.len() - 1];
            }
            if let Some(index) = line.windows(2).position(|s| s == b"//") {
                line = &line[..index];
            }
            let split = line.iter().position(space).unwrap_or(line.len());
            if split == 0 || line.is_empty() {
                continue;
            }
            return Some((&line[..split], line.get(split + 1..).unwrap_or(b"")));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_curve_rate_and_arithmetic_without_changing_state() {
        let mut voice = Voice::default();
        voice.name[0] = b'x';
        voice.phoneme_table = 3;
        let mut points = DEFAULT_TONE;
        voice.reset(22050, &mut points).unwrap();
        let retained = voice;
        let mut fast = 450;
        for (key, input) in [
            (b"tone".as_slice(), b"9000 100".as_slice()),
            (b"tone", b"0 -2147483648 8000 2147483647"),
            (b"pitch", b"2147483647 99"),
            (b"breath", b"-2147483648"),
            (b"flutter", b"2147483647"),
            (b"voicing", b"2147483647"),
            (b"fast_test2", b"99999999999999999999999999"),
        ] {
            assert!(voice.apply(key, input, true, &mut fast).is_err());
            assert_eq!(voice, retained);
            assert_eq!(fast, 450);
        }
        assert!(voice.reset(0, &mut points).is_err());
        let original_points = points;
        points[0] = 9000;
        let malformed = points;
        assert!(voice.reset(22050, &mut points).is_err());
        assert_eq!(points, malformed);
        assert_eq!(voice, retained);
        assert_eq!(original_points[8], 8000);
        assert_eq!(voice.name[0], b'x');
        assert_eq!(voice.phoneme_table, 3);
    }
    #[test]
    fn preserves_partial_assignments_eof_and_ignored_trailing_fields() {
        assert_eq!(numbers::<8>(b"\x0b +0\x0c -1.5 7").unwrap().1, 2);
        assert_eq!(numbers::<8>(b" \t").unwrap().1, -1);
        assert_eq!(numbers::<8>(b"+").unwrap().1, 0);
        assert!(numbers::<8>(b"2147483648").is_err());
        let mut voice = Voice::default();
        let mut fast = 450;
        voice.reset(22050, &mut DEFAULT_TONE.clone()).unwrap();
        voice.base_frequency[1] = 999;
        voice.base_height[1] = 888;
        voice
            .apply(b"formant", b"1 -1 -1 -1", true, &mut fast)
            .unwrap();
        assert_eq!(voice.base_frequency[1], 999);
        assert_eq!(voice.base_height[1], 888);
        voice.apply(b"clarity", b"5", true, &mut fast).unwrap();
        voice.apply(b"clarity", b"1", true, &mut fast).unwrap();
        assert_eq!(voice.peak_shape, 1);
        assert_eq!(
            voice.apply(b"speed", b"oops", true, &mut fast).unwrap(),
            Some(true)
        );
        voice.apply(b"breath", b"", true, &mut fast).unwrap();
        assert_eq!(voice.breath, [-1, 0, 0, 0, 0, 0, 0, 0, 0]);
        voice
            .apply(b"pitch", b"82 118 999999999999999999999", true, &mut fast)
            .unwrap();
        assert_eq!(voice.formant_factor, 256);
        let retained = voice;
        assert_eq!(
            voice.apply(b"klatt", b"1 2", false, &mut fast).unwrap(),
            None
        );
        assert_eq!(voice, retained);
        assert_eq!(
            voice.apply(b"language", b"en", true, &mut fast).unwrap(),
            None
        );
    }
    #[test]
    fn borrowed_directives_keep_fgets_chunk_and_comment_rules() {
        let bytes = b"# skip\nformant 1 90 // keep spacing\ntone\n pitch 82 118\nvoicing\x0b100\n";
        let mut lines = Directives::new(bytes, 4096).unwrap();
        assert_eq!(
            lines.next(),
            Some((b"formant".as_slice(), b"1 90 ".as_slice()))
        );
        assert_eq!(lines.next(), Some((b"tone".as_slice(), b"".as_slice())));
        assert_eq!(
            lines.next(),
            Some((b"voicing".as_slice(), b"100".as_slice()))
        );
        assert_eq!(lines.next(), None);
        let chunks: Vec<_> = Directives::new(b"abcdefgh\n", 5).unwrap().collect();
        assert_eq!(
            chunks,
            vec![
                (b"abcd".as_slice(), b"".as_slice()),
                (b"efgh".as_slice(), b"".as_slice())
            ]
        );
        assert!(Directives::new(b"", 1).is_err());
    }
}
