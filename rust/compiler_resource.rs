//! Owned phoneme resource dispatch, sample/envelope encoding and reference cache.
// Copyright (C) 2005-2014 Jonathan Duddington, 2013-2017 Reece H. Dunn.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{engine_io, spect_compile, spect_file, ssml::Wide, ssml_prosody, wavegen::Wavegen};
pub const MAX_SOURCE: usize = 16 * 1024 * 1024;
pub const MAX_REFERENCES: usize = 65536;
pub const MAX_NAMES: usize = 8 * 1024 * 1024;
const INVALID: i32 = spect_file::UNSUPPORTED;
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Progress {
    pub address: i32,
    pub references: i32,
    pub duplicates: i32,
    pub frames: i32,
}
pub struct Request<'a> {
    pub source: &'a [u8],
    pub name: &'a [u8],
    pub width: usize,
    pub rate: i32,
    pub control: i32,
    pub initial: Progress,
    pub mnemonic: i32,
    pub table: i32,
    pub lower: &'a [i32; 256],
    pub space: &'a [u8; 256],
    pub decimal: u8,
}
pub trait Host {
    fn log(&mut self, bytes: &[u8]);
    /// Primitive diagnostic projection: missing, format, bad spectrum, rate, mono.
    fn error(&mut self, code: i32, a: i32, b: i32, name: &[u8]);
    fn position(&mut self) -> Result<i64, i32>;
    fn generator(&mut self) -> &mut Wavegen;
    /// Destination zero is phondata, one is its manifest.
    fn write(&mut self, destination: i32, bytes: &[u8]) -> Result<(), i32>;
}
struct Reference {
    name: std::ops::Range<usize>,
    address: i32,
    _mnemonic: i32,
    _table: i32,
    next: usize,
}
pub struct Workspace {
    input: Vec<u8>,
    payload: Vec<u8>,
    names: Vec<u8>,
    references: Vec<Reference>,
    buckets: [usize; 256],
    sequence: spect_file::Sequence,
    spectrum: spect_compile::Payload,
}
impl Default for Workspace {
    fn default() -> Self {
        Self {
            input: Vec::new(),
            payload: Vec::new(),
            names: Vec::new(),
            references: Vec::new(),
            buckets: [usize::MAX; 256],
            sequence: spect_file::Sequence::default(),
            spectrum: spect_compile::Payload::default(),
        }
    }
}
impl Workspace {
    pub fn clear(&mut self) {
        self.input.clear();
        self.payload.clear();
        self.names.clear();
        self.references.clear();
        self.buckets.fill(usize::MAX);
        self.sequence.clear();
    }
    pub fn capacities(&self) -> [usize; 4] {
        [
            self.input.capacity(),
            self.payload.capacity(),
            self.names.capacity(),
            self.references.capacity(),
        ]
    }
    fn insert(&mut self, request: &Request<'_>, hash: usize, address: i32) -> Result<(), i32> {
        let end = self
            .names
            .len()
            .checked_add(request.name.len())
            .filter(|v| *v <= MAX_NAMES)
            .ok_or(12)?;
        if self.references.len() >= MAX_REFERENCES {
            return Err(12);
        }
        self.names.try_reserve(request.name.len()).map_err(|_| 12)?;
        self.references.try_reserve(1).map_err(|_| 12)?;
        let start = self.names.len();
        self.names.extend_from_slice(request.name);
        let index = self.references.len();
        self.references.push(Reference {
            name: start..end,
            address,
            _mnemonic: request.mnemonic,
            _table: request.table,
            next: self.buckets[hash],
        });
        self.buckets[hash] = index;
        Ok(())
    }
    pub fn load(&mut self, host: &mut impl Host, request: &Request<'_>) -> (i32, Progress) {
        let mut out = request.initial;
        if request.name == b"NULL" {
            return (0, out);
        }
        if request.name == b"DFT" {
            out.address = 1;
            return (0, out);
        }
        if !(2..=4096).contains(&request.width)
            || request.name.len() > 4095
            || request.source.len() > 4095
        {
            return (INVALID, out);
        }
        let Some(references) = out.references.checked_add(1) else {
            return (INVALID, out);
        };
        out.references = references;
        let hash = hash(request.name, request.lower);
        let mut index = self.buckets[hash];
        while let Some(record) = self.references.get(index) {
            if &self.names[record.name.clone()] == request.name {
                let Some(duplicates) = out.duplicates.checked_add(1) else {
                    return (INVALID, out);
                };
                out.duplicates = duplicates;
                out.address = record.address;
                break;
            }
            index = record.next;
        }
        if out.address == 0 {
            if let Err(status) = self.read_and_encode(host, request, &mut out) {
                return (status, out);
            }
        }
        if out.address > 0 {
            if let Err(status) = self.insert(request, hash, out.address) {
                return (status, out);
            }
        }
        (0, out)
    }
    fn read_and_encode(
        &mut self,
        host: &mut impl Host,
        r: &Request<'_>,
        out: &mut Progress,
    ) -> Result<(), i32> {
        let mut filename = [0; 4096];
        let base = filename_into(&mut filename, r.source, r.name, b"", r.width);
        let path = crate::voice_storage::compat_path(&filename[..base]).map_err(|_| INVALID)?;
        let fallback = match engine_io::read_file_into(&path, MAX_SOURCE, &mut self.input) {
            Ok(()) => false,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound
                        | std::io::ErrorKind::PermissionDenied
                        | std::io::ErrorKind::IsADirectory
                        | std::io::ErrorKind::NotADirectory
                ) =>
            {
                let n = filename_into(&mut filename, r.source, r.name, b".wav", r.width);
                let path =
                    crate::voice_storage::compat_path(&filename[..n]).map_err(|_| INVALID)?;
                if let Err(error) = engine_io::read_file_into(&path, MAX_SOURCE, &mut self.input) {
                    host.error(1, 0, 0, r.name);
                    return Err(spect_compile::errno(&error));
                }
                true
            }
            Err(error) => return Err(spect_compile::errno(&error)),
        };
        let id = self.input.get(..4).unwrap_or(&[]);
        let kind = if id == b"SPEC" {
            // The old spectral controller reopens the unsuffixed name, even
            // after a .wav fallback probe. Keep this defined legacy behavior.
            if fallback {
                let n = filename_into(&mut filename, r.source, r.name, b"", r.width);
                let path =
                    crate::voice_storage::compat_path(&filename[..n]).map_err(|_| INVALID)?;
                if let Err(error) = engine_io::read_file_into(&path, MAX_SOURCE, &mut self.input) {
                    let mut message = [0; 4120];
                    let prefix = b"Failed to open: '";
                    message[..prefix.len()].copy_from_slice(prefix);
                    message[prefix.len()..prefix.len() + n].copy_from_slice(&filename[..n]);
                    message[prefix.len() + n] = b'\'';
                    host.log(&message[..prefix.len() + n + 1]);
                    host.error(3, 0, 0, r.name);
                    return Err(spect_compile::errno(&error));
                }
            }
            if let Err(code) = self.sequence.bind(&self.input) {
                if code == INVALID {
                    host.log(b"Unsupported spectral file format.\n");
                }
                host.error(3, 0, 0, r.name);
                self.sequence.clear();
                return Err(code);
            }
            out.address = position(host)?;
            let packed =
                self.spectrum
                    .pack(&mut self.sequence, host.generator(), r.rate, r.control);
            self.sequence.clear();
            packed?;
            out.frames = out
                .frames
                .checked_add(self.spectrum.frames as i32)
                .ok_or(INVALID)?;
            host.write(0, self.spectrum.bytes())?;
            b'S'
        } else if id == b"RIFF" {
            if self.input.len() < 44 {
                return Err(INVALID);
            }
            let rate = word(&self.input, 24)?;
            let bytes_per_second = word(&self.input, 28)?;
            let expected = rate.checked_mul(2).ok_or(INVALID)?;
            if rate != r.rate || bytes_per_second != expected {
                host.error(if rate != r.rate { 4 } else { 5 }, rate, r.rate, r.name);
                out.address = 0;
                return Ok(());
            }
            let address = position(host)?;
            self.wave(address)?;
            host.write(0, &self.payload)?;
            out.address = address | 0x800000;
            b'W'
        } else if id == b"PITC" {
            let address = position(host)?;
            let bytes = self.input.get(12..140).ok_or(INVALID)?;
            // Correct the old fread(size=128,count=1) !=128 test, which never
            // wrote a valid binary envelope and returned stale errno instead.
            host.write(0, bytes)?;
            out.address = address;
            b'E'
        } else if id == b"ENVE" {
            self.envelope(r.space, r.decimal)?;
            let address = position(host)?;
            host.write(0, &self.payload)?;
            out.address = address;
            b'E'
        } else {
            host.error(2, 0, 0, r.name);
            out.address = -1;
            return Err(INVALID);
        };
        if out.address > 0 {
            let mut message = [0; 4120];
            let prefix = format!("{}  0x{:05x}  ", char::from(kind), out.address & 0x7fffff);
            let n = prefix.len();
            message[..n].copy_from_slice(prefix.as_bytes());
            message[n..n + r.name.len()].copy_from_slice(r.name);
            message[n + r.name.len()] = b'\n';
            host.write(1, &message[..n + r.name.len() + 1])?;
        }
        Ok(())
    }
    fn wave(&mut self, position: i32) -> Result<(), i32> {
        let samples = &self.input[44..];
        let max = samples
            .chunks_exact(2)
            .map(|b| i32::from(i16::from_le_bytes([b[0], b[1]])).abs())
            .max()
            .unwrap_or(0);
        let scale = max / 127 + 1;
        let length = (word(&self.input, 40)? / 2)
            .checked_add(scale << 16)
            .ok_or(INVALID)?;
        let n = 4 + samples.len() / 2 + 3;
        self.payload.clear();
        self.payload.try_reserve(n).map_err(|_| 12)?;
        self.payload.extend_from_slice(&length.to_le_bytes());
        for b in samples.chunks_exact(2) {
            let sample = f32::from(i16::from_le_bytes([b[0], b[1]]));
            self.payload
                .push(((sample / scale as f32 + 0.5) as i32).clamp(-128, 127) as u8);
        }
        while (position as usize + self.payload.len()) & 3 != 0 {
            self.payload.push(0);
        }
        Ok(())
    }
    fn envelope(&mut self, space: &[u8; 256], decimal: u8) -> Result<(), i32> {
        let mut x = [0_f32; 20];
        let mut y = x;
        let mut linear = [0_i32; 20];
        let mut count = 0;
        let mut at = 0;
        line(&self.input, &mut at); // fgets consumes at most 127 bytes per chunk.
        while at < self.input.len() {
            let text = line(&self.input, &mut at);
            if let Some((a, b, c)) = point(text, space, decimal)? {
                if count >= 19 || !a.is_finite() || !b.is_finite() {
                    return Err(INVALID);
                }
                x[count] = a * 1.28_f32;
                y[count] = b;
                linear[count] = c;
                count += 1;
            }
        }
        // Zero/one-point C input used uninitialized envelope/interpolation data.
        if count < 2 {
            return Err(INVALID);
        }
        x[count] = x[count - 1];
        y[count] = y[count - 1];
        let mut envelope = [0; 128];
        let mut ix = 0;
        let mut ix2 = 0;
        for (value, byte) in envelope.iter_mut().enumerate() {
            let value = value as f32;
            if count > 3 && value > *x.get(ix + 3).ok_or(INVALID)? {
                ix += 1;
            }
            if count > 2 && value >= *x.get(ix2 + 1).ok_or(INVALID)? {
                ix2 += 1;
            }
            let n = if linear[ix2] > 0 {
                let u = ix2 + 1;
                let b = y[ix2] + (y[u] - y[ix2]) * (value - x[ix2]) / (x[u] - x[ix2]);
                f64::from(b) * 2.55
            } else {
                let n = if count > 3 { 4 } else { 3 };
                let a = x.get(ix..ix + n).ok_or(INVALID)?;
                let b = y.get(ix..ix + n).ok_or(INVALID)?;
                f64::from(spect_file::interpolate(a, b, value)? * 255_f32 / 100_f32)
            };
            if !n.is_finite() || !(-2147483648.0..2147483648.0).contains(&n) {
                return Err(INVALID);
            }
            *byte = (n as i32).clamp(0, 255) as u8;
        }
        self.payload.clear();
        self.payload.try_reserve(128).map_err(|_| 12)?;
        self.payload.extend_from_slice(&envelope);
        Ok(())
    }
}
fn position(host: &mut impl Host) -> Result<i32, i32> {
    let n = host.position()?;
    if !(0..=i64::from(i32::MAX)).contains(&n) {
        return Err(INVALID);
    }
    Ok(n as i32)
}
fn word(bytes: &[u8], at: usize) -> Result<i32, i32> {
    Ok(i32::from_le_bytes(
        bytes.get(at..at + 4).ok_or(INVALID)?.try_into().unwrap(),
    ))
}
pub fn hash(name: &[u8], lower: &[i32; 256]) -> usize {
    let mut value = 0_i32;
    for c in name {
        value = value
            .wrapping_mul(8)
            .wrapping_add(lower[*c as usize].wrapping_sub(97));
        value = (value & 0x1ff) ^ (value >> 8);
    }
    (value.wrapping_add(name.len() as i32) & 255) as usize
}
fn filename_into(
    out: &mut [u8; 4096],
    source: &[u8],
    name: &[u8],
    suffix: &[u8],
    width: usize,
) -> usize {
    let mut n = 0;
    for bytes in [source, b"/", name, suffix] {
        let length = bytes.len().min(width - 1 - n);
        out[n..n + length].copy_from_slice(&bytes[..length]);
        n += length;
    }
    out[n] = 0;
    n
}
fn line<'a>(bytes: &'a [u8], at: &mut usize) -> &'a [u8] {
    let start = *at;
    while *at < bytes.len() && *at - start < 127 {
        let c = bytes[*at];
        *at += 1;
        if c == b'\n' {
            break;
        }
    }
    let end = bytes[start..*at]
        .iter()
        .position(|c| *c == 0)
        .map_or(*at, |n| start + n);
    &bytes[start..end]
}
fn point(bytes: &[u8], space: &[u8; 256], decimal: u8) -> Result<Option<(f32, f32, i32)>, i32> {
    let mut wide = [0_u32; 128];
    for (i, c) in bytes.iter().enumerate() {
        wide[i] = u32::from(*c);
    }
    let wide = Wide::U32(&wide[..bytes.len() + 1]);
    let float = |at| -> Result<Option<(f32, usize)>, i32> {
        let Some((value, end)) = ssml_prosody::number(wide, at, u32::from(decimal), |c| {
            c < 256 && space[c as usize] != 0
        })
        .map_err(|_| INVALID)?
        else {
            return Ok(None);
        };
        let mut start = at;
        while start < end && space[bytes[start] as usize] != 0 {
            start += 1;
        }
        let token = &bytes[start..end];
        let mut normalized = [0; 128];
        for (i, c) in token.iter().enumerate() {
            normalized[i] = if *c == decimal { b'.' } else { *c };
        }
        let text = std::str::from_utf8(&normalized[..token.len()]).map_err(|_| INVALID)?;
        // Direct decimal-to-binary32 avoids double rounding through binary64.
        let value = if text.trim_start_matches(['+', '-']).starts_with("0x")
            || text.trim_start_matches(['+', '-']).starts_with("0X")
        {
            hex32(text.as_bytes())
        } else {
            text.parse::<f32>().unwrap_or(value as f32)
        };
        Ok(Some((value, end)))
    };
    let Some((a, at)) = float(0)? else {
        return Ok(None);
    };
    let Some((b, mut at)) = float(at)? else {
        return Ok(None);
    };
    while at < bytes.len() && space[bytes[at] as usize] != 0 {
        at += 1;
    }
    let negative = bytes.get(at) == Some(&b'-');
    if matches!(bytes.get(at), Some(b'+' | b'-')) {
        at += 1;
    }
    let mut n = 0_i64;
    while let Some(c @ b'0'..=b'9') = bytes.get(at) {
        n = n
            .checked_mul(10)
            .and_then(|n| n.checked_add(i64::from(c - b'0')))
            .ok_or(INVALID)?;
        at += 1;
    }
    if negative {
        n = -n;
    }
    let n = i32::try_from(n).map_err(|_| INVALID)?;
    Ok(Some((a, b, n)))
}

// Direct hexadecimal-to-binary32 rounding, including ties and subnormals.
// The shared lexer has already admitted the complete significand/exponent.
fn hex32(token: &[u8]) -> f32 {
    let negative = token.first() == Some(&b'-');
    let start = usize::from(matches!(token.first(), Some(b'+' | b'-'))) + 2;
    let tail = &token[start..];
    let marker = tail
        .iter()
        .position(|c| matches!(c, b'p' | b'P'))
        .unwrap_or(tail.len());
    let significand = &tail[..marker];
    let exponent = if marker == tail.len() {
        0
    } else {
        let exponent = &tail[marker + 1..];
        std::str::from_utf8(exponent)
            .unwrap()
            .parse::<i64>()
            .unwrap_or(if exponent.first() == Some(&b'-') {
                -1_000_000
            } else {
                1_000_000
            })
            .clamp(-1_000_000, 1_000_000)
    };
    let fractional = significand
        .iter()
        .position(|c| *c == b'.')
        .map_or(0, |at| significand.len() - at - 1);
    let bits = || {
        significand
            .iter()
            .filter(|c| **c != b'.')
            .flat_map(|c| {
                let digit = match c {
                    b'0'..=b'9' => c - b'0',
                    b'a'..=b'f' => c - b'a' + 10,
                    b'A'..=b'F' => c - b'A' + 10,
                    _ => unreachable!("validated hexadecimal grammar"),
                };
                (0..4).rev().map(move |bit| (digit >> bit) & 1)
            })
            .skip_while(|bit| *bit == 0)
    };
    let length = bits().count() as i64;
    let sign = u32::from(negative) << 31;
    if length == 0 {
        return f32::from_bits(sign);
    }
    let scale = exponent - 4 * fractional as i64;
    let mut power = length - 1 + scale;
    if power > 127 {
        return f32::from_bits(sign | 0x7f800000);
    }
    let keep = if power >= -126 {
        24
    } else {
        length + scale + 149
    };
    if keep < 0 {
        return f32::from_bits(sign);
    }
    let mut value = 0_u32;
    let mut guard = false;
    let mut sticky = false;
    for (i, bit) in bits().enumerate() {
        if (i as i64) < keep {
            value = (value << 1) | u32::from(bit);
        } else if i as i64 == keep {
            guard = bit != 0;
        } else {
            sticky |= bit != 0;
        }
    }
    if keep > length {
        value <<= keep - length;
    }
    if guard && (sticky || value & 1 != 0) {
        value += 1;
    }
    if power < -126 {
        return f32::from_bits(sign | value);
    }
    if value == 1 << 24 {
        value >>= 1;
        power += 1;
    }
    f32::from_bits(sign | (((power + 127) as u32) << 23) | (value & 0x7fffff))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn maps() -> ([i32; 256], [u8; 256]) {
        (
            std::array::from_fn(|i| (i as u8).to_ascii_lowercase() as i32),
            std::array::from_fn(|i| u8::from((i as u8).is_ascii_whitespace())),
        )
    }
    #[test]
    fn wave_sign_scale_declared_length_odd_tail_and_absolute_alignment() {
        let mut w = Workspace::default();
        w.input.resize(44, 0);
        w.input[40..44].copy_from_slice(&10_i32.to_le_bytes());
        for sample in [i16::MIN, -1, 0, 1, i16::MAX] {
            w.input.extend_from_slice(&sample.to_le_bytes());
        }
        w.input.push(17);
        w.wave(7).unwrap();
        assert_eq!(&w.payload[..4], &(5_i32 + (259 << 16)).to_le_bytes());
        assert_eq!(&w.payload[4..], &[130, 0, 0, 0, 127]);
        assert_eq!((7 + w.payload.len()) % 4, 0);
        w.input.truncate(44);
        w.wave(8).unwrap();
        assert_eq!(&w.payload[..4], &(5_i32 + (1 << 16)).to_le_bytes());
    }
    #[test]
    fn envelope_numeric_grammar_rounding_locale_and_uninitialized_c_guards() {
        let (_, space) = maps();
        assert_eq!(
            point(b" 12.5 -3.25 1 tail", &space, b'.').unwrap(),
            Some((12.5, -3.25, 1))
        );
        assert_eq!(
            point(b" 12,5 -3,25 -1", &space, b',').unwrap(),
            Some((12.5, -3.25, -1))
        );
        assert!(point(b"1e 2", &space, b'.').unwrap().is_none());
        assert_eq!(
            point(b"0x1.0000010000000001p0 0x1p-149", &space, b'.')
                .unwrap()
                .unwrap()
                .0
                .to_bits(),
            0x3f800001
        );
        assert_eq!(hex32(b"0x1p-149").to_bits(), 1);
        assert_eq!(hex32(b"0x1p-150").to_bits(), 0);
        assert_eq!(hex32(b"-0x1.000000000000001p-150").to_bits(), 0x80000001);
        let mut w = Workspace::default();
        w.input.extend_from_slice(b"ENVELOPE\n0 0 1\n100 100\n");
        w.envelope(&space, b'.').unwrap();
        assert_eq!(w.payload.len(), 128);
        assert_eq!(w.payload[0], 0);
        assert_eq!(w.payload[64], 127);
        for source in [
            b"ENVELOPE\n".as_slice(),
            b"ENVELOPE\n0 0\n",
            b"ENVELOPE\n0 nan\n100 1\n",
            b"ENVELOPE\n0 0 1\n0 1\n",
        ] {
            w.input.clear();
            w.input.extend_from_slice(source);
            assert_eq!(w.envelope(&space, b'.'), Err(INVALID));
        }
        w.input.clear();
        w.input.extend_from_slice(b"ENVELOPE\n");
        for _ in 0..20 {
            w.input.extend_from_slice(b"0 0\n");
        }
        assert_eq!(w.envelope(&space, b'.'), Err(INVALID));
    }
    struct Effects {
        generator: Wavegen,
        writes: Vec<(i32, Vec<u8>)>,
        errors: Vec<i32>,
        offset: i64,
        fail: i32,
        positions: usize,
    }
    #[test]
    fn reference_collisions_exact_names_cached_addresses_and_bounded_arena() {
        let (lower, space) = maps();
        let mut w = Workspace::default();
        let mut host = Effects {
            generator: Wavegen::default(),
            writes: Vec::new(),
            errors: Vec::new(),
            offset: 8,
            fail: 0,
            positions: 0,
        };
        let mut r = Request {
            source: b"missing",
            name: b"A",
            width: 4096,
            rate: 22050,
            control: 0,
            initial: Progress {
                address: 17,
                ..Progress::default()
            },
            mnemonic: 11,
            table: 7,
            lower: &lower,
            space: &space,
            decimal: b'.',
        };
        assert_eq!(hash(b"A", &lower), hash(b"a", &lower));
        assert_eq!(w.load(&mut host, &r).0, 0);
        r.name = b"a";
        r.initial.address = 23;
        assert_eq!(w.load(&mut host, &r).0, 0);
        r.name = b"A";
        r.initial = Progress::default();
        let (status, out) = w.load(&mut host, &r);
        assert_eq!(status, 0);
        assert_eq!(out.address, 17);
        assert_eq!(out.duplicates, 1);
        r.name = b"a";
        assert_eq!(w.load(&mut host, &r).1.address, 23);
        assert!(host.writes.is_empty());
        assert_eq!(host.positions, 0);
        r.name = b"A";
        r.initial.duplicates = i32::MAX;
        assert_eq!(w.load(&mut host, &r).0, INVALID);
        assert_eq!(w.references.len(), 4);
        r.initial = Progress {
            address: 99,
            ..Progress::default()
        };
        r.name = b"retained";
        while w.references.len() < MAX_REFERENCES {
            assert_eq!(w.load(&mut host, &r).0, 0);
        }
        assert_eq!(w.load(&mut host, &r).0, 12);
        assert_eq!(w.references.len(), MAX_REFERENCES);
        let capacity = w.capacities();
        w.clear();
        assert_eq!(w.capacities(), capacity);
        assert_eq!(w.load(&mut host, &r).0, 0);
        w.names.resize(MAX_NAMES, 0);
        r.name = b"another";
        assert_eq!(w.load(&mut host, &r).0, 12);
        assert_eq!(w.references.len(), 1);
    }
    impl Host for Effects {
        fn log(&mut self, _: &[u8]) {}
        fn error(&mut self, code: i32, _: i32, _: i32, _: &[u8]) {
            self.errors.push(code);
        }
        fn position(&mut self) -> Result<i64, i32> {
            self.positions += 1;
            Ok(self.offset)
        }
        fn generator(&mut self) -> &mut Wavegen {
            &mut self.generator
        }
        fn write(&mut self, destination: i32, bytes: &[u8]) -> Result<(), i32> {
            if self.fail != 0 {
                return Err(self.fail);
            }
            if destination == 0 {
                self.offset += bytes.len() as i64;
            }
            self.writes.push((destination, bytes.to_vec()));
            Ok(())
        }
    }
    #[test]
    fn actual_proactor_sources_owned_cache_failure_and_reused_arenas() {
        let (lower, space) = maps();
        let dir = std::env::temp_dir();
        let name = format!("espeak-resource-{}-safe", std::process::id());
        let path = dir.join(&name);
        let mut source = b"PITC".to_vec();
        source.extend_from_slice(&[0; 8]);
        source.extend(0..128_u8);
        std::fs::write(&path, &source).unwrap();
        let mut host = Effects {
            generator: Wavegen::default(),
            writes: Vec::new(),
            errors: Vec::new(),
            offset: 8,
            fail: 0,
            positions: 0,
        };
        let mut w = Workspace::default();
        let source = dir.to_str().unwrap().as_bytes();
        let mut r = Request {
            source,
            name: name.as_bytes(),
            width: 4096,
            rate: 22050,
            control: 0,
            initial: Progress::default(),
            mnemonic: 7,
            table: 2,
            lower: &lower,
            space: &space,
            decimal: b'.',
        };
        let (status, out) = w.load(&mut host, &r);
        assert_eq!(status, 0);
        assert_eq!(out.address, 8);
        assert_eq!(out.references, 1);
        assert_eq!(host.writes[0].1, (0..128_u8).collect::<Vec<_>>());
        assert_eq!(w.references[0]._mnemonic, 7);
        assert_eq!(w.references[0]._table, 2);
        std::fs::remove_file(&path).unwrap();
        r.initial = Progress { address: 0, ..out };
        let (status, out) = w.load(&mut host, &r);
        assert_eq!(status, 0);
        assert_eq!(out.duplicates, 1);
        assert_eq!(host.writes.len(), 2);
        assert_eq!(w.references.len(), 2);
        let capacities = w.capacities();
        w.clear();
        assert_eq!(w.capacities(), capacities);
        assert!(w.references.is_empty());
        r.initial = Progress::default();
        let (status, out) = w.load(&mut host, &r);
        assert_eq!(status, 2);
        assert_eq!(out.references, 1);
        assert_eq!(host.errors, [1]);
        assert!(w.references.is_empty());
        let file = std::fs::File::create(&path).unwrap();
        file.set_len((MAX_SOURCE + 1) as u64).unwrap();
        drop(file);
        let positions = host.positions;
        let writes = host.writes.len();
        assert_eq!(w.load(&mut host, &r).0, 22);
        assert_eq!(host.positions, positions);
        assert_eq!(host.writes.len(), writes);
        assert!(w.references.is_empty());
        std::fs::remove_file(&path).unwrap();
        r.name = b"NULL";
        assert_eq!(w.load(&mut host, &r), (0, Progress::default()));
        r.name = b"DFT";
        assert_eq!(w.load(&mut host, &r).1.address, 1);
        r.name = name.as_bytes();
        r.initial.references = i32::MAX;
        assert_eq!(w.load(&mut host, &r).0, INVALID);
    }
}
