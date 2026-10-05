//! Bounded MBROLA pitch text and exact little-endian PCM amplitude scaling.
// Copyright (C) 2005-2013 Jonathan Duddington, 2015-2016 Reece H. Dunn;
// Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::synthesis_parameters::{Error, Pitch};
use std::fmt::Write;
pub struct Text {
    bytes: [u8; 128],
    length: usize,
}
impl Text {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
    pub fn terminated(&self) -> &[u8] {
        &self.bytes[..self.length + 1]
    }
}
impl Write for Text {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        let end = self.length.checked_add(text.len()).ok_or(std::fmt::Error)?;
        if end >= self.bytes.len() {
            return Err(std::fmt::Error);
        }
        self.bytes[self.length..end].copy_from_slice(text.as_bytes());
        self.length = end;
        Ok(())
    }
}
fn value(envelope: u8, pitch: Pitch) -> Result<i32, Error> {
    i32::from(envelope)
        .checked_mul(pitch.range)
        .map(|value| value >> 8)
        .and_then(|value| value.checked_add(pitch.base))
        .ok_or(Error::Arithmetic)
}
/// Format the fixed 80-percent contour and endpoint using owned stack text.
/// The C order selects the minimum's interior position after the maximum's.
pub fn pitch_text(
    envelope: &[u8; 128],
    envelope_number: i32,
    pitch: Pitch,
    split: i32,
    final_only: bool,
) -> Result<Text, Error> {
    let env_split = split.checked_mul(128).ok_or(Error::Arithmetic)? / 100;
    let env_split = env_split.checked_abs().ok_or(Error::Arithmetic)?;
    if split != 0 && env_split == 0 {
        return Err(Error::Split);
    }
    let mut maximum = 0;
    let mut minimum = 0;
    for index in 1..128 {
        if envelope[index] > envelope[maximum] {
            maximum = index;
        }
        if envelope[index] < envelope[minimum] {
            minimum = index;
        }
    }
    let mut middle = 64;
    if maximum > 0 && maximum < 127 {
        middle = maximum;
    }
    if minimum > 0 && minimum < 127 {
        middle = minimum;
    }
    let positions = [middle / 2, middle, middle + (127 - middle) / 2];
    let first = value(envelope[0], pitch)?;
    let last = value(envelope[127], pitch)? / 4096;
    let mut text = Text {
        bytes: [0; 128],
        length: 0,
    };
    if split >= 0 {
        write!(text, " 0 {}", first / 4096).map_err(|_| Error::Capacity)?;
    }
    if envelope_number > 1 {
        for position in positions {
            let pitch = value(envelope[position], pitch)? / 4096;
            let position = position as i32;
            let offset = if split > 0 {
                (position * 80) / env_split
            } else if split < 0 {
                ((position - env_split) * 80) / env_split
            } else {
                (position * 80) / 128
            };
            if offset > 0 && offset <= 80 {
                write!(text, " {offset} {pitch}").map_err(|_| Error::Capacity)?;
            }
        }
    }
    if split <= 0 {
        write!(text, " 80 {last}").map_err(|_| Error::Capacity)?;
    }
    writeln!(text, " 100 {last}").map_err(|_| Error::Capacity)?;
    if final_only {
        text.bytes.fill(0);
        text.length = 0;
        writeln!(text, "\t100 {last}").map_err(|_| Error::Capacity)?;
    }
    Ok(text)
}
/// Scale initialized 16-bit little-endian PCM in place. The common amplitude
/// range needs one pass; unusually large values are validated before mutation.
/// No scratch buffer, allocation, callbacks or accelerator conversion is used.
pub fn scale_pcm(bytes: &mut [u8], amplitude: i32) -> Result<(), Error> {
    if bytes.len() % 2 != 0 {
        return Err(Error::SampleAlignment);
    }
    if !(-65535..=65535).contains(&amplitude) {
        for sample in bytes.chunks_exact(2) {
            i32::from(i16::from_le_bytes([sample[0], sample[1]]))
                .checked_mul(amplitude)
                .ok_or(Error::Arithmetic)?;
        }
    }
    for sample in bytes.chunks_exact_mut(2) {
        let value = i32::from(i16::from_le_bytes([sample[0], sample[1]])) * amplitude / 40;
        let value = value.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        sample.copy_from_slice(&value.to_le_bytes());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contours_keep_endpoint_split_and_final_whitespace() {
        let mut env = [0; 128];
        for (index, value) in env.iter_mut().enumerate() {
            *value = index as u8;
        }
        let pitch = Pitch {
            base: 100 * 4096,
            range: 256 * 4096,
        };
        assert_eq!(
            pitch_text(&env, 0, pitch, 0, false).unwrap().bytes(),
            b" 0 100 80 227 100 227\n"
        );
        assert_eq!(
            pitch_text(&env, 2, pitch, 0, true).unwrap().bytes(),
            b"\t100 227\n"
        );
        assert!(pitch_text(
            &env,
            2,
            Pitch {
                base: i32::MAX,
                range: i32::MAX
            },
            0,
            false
        )
        .is_err());
    }
    #[test]
    fn scaling_keeps_signed_samples_clamps_and_rejects_before_writes() {
        let mut bytes = [0, 128, 255, 127, 16, 0, 240, 255];
        scale_pcm(&mut bytes, 80).unwrap();
        assert_eq!(bytes, [0, 128, 255, 127, 32, 0, 224, 255]);
        let before = bytes;
        assert_eq!(scale_pcm(&mut bytes, i32::MAX), Err(Error::Arithmetic));
        assert_eq!(bytes, before);
        let mut zeros = [0; 4];
        scale_pcm(&mut zeros, i32::MAX).unwrap();
        assert_eq!(zeros, [0; 4]);
        assert_eq!(scale_pcm(&mut [0; 3], 40), Err(Error::SampleAlignment));
    }
}
