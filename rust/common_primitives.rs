//! Owned random sequence, bounded string copies and stream-word assembly.
// Copyright (C) 2005-2013 Jonathan Duddington, 2013-2017 Reece H. Dunn.
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Capacity,
    Bounds,
    Arithmetic,
}
pub fn advance(state: u32) -> u32 {
    ((u64::from(state) * 1103515245 + 12345) % 0x7fff_ffff) as u32
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Random {
    state: u32,
}
impl Random {
    pub fn from_state(state: u32) -> Self {
        Self { state }
    }
    pub fn state(&self) -> u32 {
        self.state
    }
    /// Seeding includes the legacy dummy generator flush. Signed seeds retain
    /// their low32 bits; an unseeded default instance starts from state zero.
    pub fn seed(&mut self, seed: i64) {
        self.state = advance(seed as u32);
    }
    /// Preserve the original remainder-minus-min expression, including ranges
    /// with negative divisors. Undefined zero-divisor/overflow inputs reject
    /// before changing the owned state. This is not a corrected distribution.
    pub fn next(&mut self, min: i64, max: i64) -> Result<i64, Error> {
        let range = max
            .checked_sub(min)
            .and_then(|v| v.checked_add(1))
            .filter(|v| *v != 0)
            .ok_or(Error::Arithmetic)?;
        let state = advance(self.state);
        let result = (i64::from(state) % range)
            .checked_sub(min)
            .ok_or(Error::Arithmetic)?;
        self.state = state;
        Ok(result)
    }
}
/// Plan reads only initialized source bytes through NUL or the truncation
/// bound. Source length after that point is irrelevant. Zero capacity rejects.
pub fn copy_length(source: &[u8], capacity: usize) -> Result<usize, Error> {
    let limit = capacity.checked_sub(1).ok_or(Error::Capacity)?;
    for index in 0..limit {
        if *source.get(index).ok_or(Error::Bounds)? == 0 {
            return Ok(index);
        }
    }
    Ok(limit)
}
/// All destination bytes are initialized, including legacy zero padding and
/// the forced final NUL. Admission completes before any destination write.
pub fn copy_string(source: &[u8], output: &mut [u8]) -> Result<(), Error> {
    let length = copy_length(source, output.len())?;
    output[..length].copy_from_slice(&source[..length]);
    output[length..].fill(0);
    Ok(())
}
/// Exactly four byte-source calls, even after EOF. Their low-byte bit patterns
/// are packed little endian; EOF(-1) contributes255 like the C stdio helper.
pub fn read4(mut read: impl FnMut() -> i32) -> i32 {
    let mut bytes = [0; 4];
    for byte in &mut bytes {
        *byte = read() as u8;
    }
    u32::from_le_bytes(bytes) as i32
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn random_instances_keep_flush_negative_ranges_and_guarded_state() {
        let mut first = Random::default();
        let second = Random::default();
        assert_eq!(first.next(-8192, 8191), Ok(20537));
        assert_eq!(first.state(), 12345);
        assert_eq!(second.state(), 0);
        first.seed(-1);
        assert_eq!(first.state(), advance(u32::MAX));
        let previous = first.state();
        assert_eq!(first.next(1, 0), Err(Error::Arithmetic));
        assert_eq!(first.state(), previous);
        assert!(first.next(3, 0).is_ok());
        let previous = first.state();
        assert_eq!(first.next(i64::MIN, i64::MAX), Err(Error::Arithmetic));
        assert_eq!(first.state(), previous);
    }
    #[test]
    fn copy_admission_precedes_padding_and_stream_reads_do_not_stop_at_eof() {
        let mut output = [0xa5; 7];
        copy_string(b"ab\0", &mut output).unwrap();
        assert_eq!(&output, b"ab\0\0\0\0\0");
        copy_string(b"abcdefgh", &mut output).unwrap();
        assert_eq!(&output, b"abcdef\0");
        let previous = output;
        assert_eq!(copy_string(b"ab", &mut output), Err(Error::Bounds));
        assert_eq!(output, previous);
        assert_eq!(copy_string(b"", &mut []), Err(Error::Capacity));
        let mut calls = 0;
        assert_eq!(
            read4(|| {
                calls += 1;
                -1
            }),
            -1
        );
        assert_eq!(calls, 4);
        let mut input = [0x78, 0x56, 0x34, 0x12].into_iter();
        assert_eq!(read4(|| input.next().unwrap()), 0x12345678);
    }
}
