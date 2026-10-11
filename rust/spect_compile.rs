//! Complete spectral-to-phondata packing over owned compiler sources.
// Copyright (C) 2005-2014 Jonathan Duddington, 2013-2017 Reece H. Dunn.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    spect_file::{self, LoadError, Sequence, NO_FRAMES, UNSUPPORTED},
    wavegen::Wavegen,
};
use std::path::Path;
pub const MAX_KEYFRAMES: usize = 25;
pub const MAX_BYTES: usize = 4 + MAX_KEYFRAMES * 64;
pub struct Payload {
    bytes: [u8; MAX_BYTES],
    length: usize,
    pub frames: usize,
    pub klatt: bool,
}
impl Default for Payload {
    fn default() -> Self {
        Self {
            bytes: [0; MAX_BYTES],
            length: 0,
            frames: 0,
            klatt: false,
        }
    }
}
impl Payload {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
    /// All records are initialized explicit little-endian bytes, not native structs.
    pub fn pack(
        &mut self,
        sequence: &mut Sequence,
        generator: &mut Wavegen,
        rate: i32,
        control: i32,
    ) -> Result<(), i32> {
        self.length = 0;
        self.frames = 0;
        self.klatt = false;
        self.bytes.fill(0);
        let amplitude = i32::from(sequence.amplitude);
        let frames = sequence.frames_mut();
        if frames.is_empty() {
            return Err(NO_FRAMES);
        }
        self.klatt = frames.iter().any(|f| f.klatt[5..].iter().any(|v| *v != 0));
        self.frames = frames.iter().filter(|f| f.keyframe != 0).count();
        if self.frames > MAX_KEYFRAMES {
            return Err(UNSUPPORTED);
        }
        let mut total = 0_f32;
        let mut center = 0;
        let mut count = 0;
        let mut marked = false;
        for (i, f) in frames.iter().enumerate() {
            if f.keyframe == 0 {
                continue;
            }
            if count == 1 {
                center = i;
            }
            marked |= f.markers & 2 != 0;
            count += 1;
            if i > 0 {
                total += frames[i - 1].length;
            }
        }
        let total = integer(f64::from(total))?;
        self.bytes[..2].copy_from_slice(&(total as i16).to_le_bytes());
        self.bytes[2] = self.frames as u8;
        if control & 1 != 0 && !marked {
            frames[center].markers |= 2;
        }
        let size = if self.klatt { 64 } else { 44 };
        let mut at = 4;
        for f in frames.iter_mut().filter(|f| f.keyframe != 0) {
            let record = &mut self.bytes[at..at + size];
            record[..2].copy_from_slice(&(f.markers | i16::from(self.klatt)).to_le_bytes());
            for (i, p) in f.peaks[..7].iter().enumerate() {
                record[2 + i * 2..4 + i * 2].copy_from_slice(&p.freq.to_le_bytes());
            }
            record[16] = integer(f64::from(f.length) + 0.5)?.min(255) as u8;
            f.rms = spect_file::frame_rms(&f.peaks, amplitude, f.amplitude, generator, rate)?;
            record[17] = integer(f.rms)?.min(255) as u8;
            if at == 4 + (self.frames.saturating_sub(1)) * size {
                record[16] = 0;
            }
            for (i, p) in f.peaks[..8].iter().enumerate() {
                let product = amplitude
                    .checked_mul(i32::from(f.amplitude))
                    .and_then(|v| v.checked_mul(i32::from(p.height)))
                    .ok_or(UNSUPPORTED)?;
                let height = ((product as f32) / 640000.0).min(255.0);
                record[18 + i] = integer(f64::from(height))? as u8;
                if i < 6 {
                    record[26 + i] = (i32::from(p.left) / 4).min(255) as u8;
                }
                if i < 3 {
                    record[32 + i] = (i32::from(p.right) / 4).min(255) as u8;
                }
                if i < 4 {
                    record[35 + i] = (i32::from(p.bandwidth) / 2).min(255) as u8;
                }
            }
            for (i, v) in f.klatt[..5].iter().enumerate() {
                record[39 + i] = *v as u8;
            }
            record[40] = (f.klatt[1] / 2) as u8;
            if self.klatt {
                for (i, v) in f.klatt[5..10].iter().enumerate() {
                    record[44 + i] = *v as u8;
                }
                for (i, p) in f.peaks[..7].iter().enumerate() {
                    record[49 + i] = p.amplitude as u8;
                    record[56 + i] = (i32::from(p.parallel_bandwidth) / 2).min(255) as u8;
                }
            }
            if record[36] == 0 {
                record[35..39].copy_from_slice(&[44, 45, 70, 130]);
            }
            at += size;
        }
        self.length = (at + 3) & !3;
        Ok(())
    }
}
fn integer(n: f64) -> Result<i32, i32> {
    if !n.is_finite() || !(-2147483648.0..2147483648.0).contains(&n) {
        return Err(UNSUPPORTED);
    }
    Ok(n as i32)
}
pub trait Host {
    fn log(&mut self, bytes: &[u8]);
    fn bad_source(&mut self, path: &[u8]);
    fn position(&mut self) -> Result<i64, i32>;
    fn generator(&mut self) -> &mut Wavegen;
    fn write(&mut self, bytes: &[u8]) -> Result<(), i32>;
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct ResultState {
    pub address: i32,
    pub count_frames: i32,
    pub has_address: u32,
}
#[derive(Default)]
pub struct Workspace {
    pub sequence: Sequence,
    pub payload: Payload,
}
pub struct Request<'a> {
    pub path: &'a Path,
    pub filename: &'a [u8],
    pub reference: &'a [u8],
    pub rate: i32,
    pub control: i32,
    pub initial_count: i32,
}
impl Workspace {
    /// Read source before borrowing a generator or delivering output. Stream
    /// position/count publication follows the old controller; write failures
    /// propagate rather than reporting success after an unchecked C fwrite.
    pub fn compile(&mut self, host: &mut impl Host, request: &Request<'_>) -> (i32, ResultState) {
        let Request {
            path,
            filename,
            reference,
            rate,
            control,
            initial_count,
        } = *request;
        let mut result = ResultState {
            count_frames: initial_count,
            ..ResultState::default()
        };
        if let Err(error) = self.sequence.load(path) {
            let status = match error {
                LoadError::Format(code) => {
                    if code == UNSUPPORTED {
                        host.log(b"Unsupported spectral file format.\n");
                    }
                    code
                }
                LoadError::Io(error) => {
                    let mut text = [0; 4120];
                    let prefix = b"Failed to open: '";
                    let n = filename.len().min(4096);
                    text[..prefix.len()].copy_from_slice(prefix);
                    text[prefix.len()..prefix.len() + n].copy_from_slice(&filename[..n]);
                    text[prefix.len() + n] = b'\'';
                    host.log(&text[..prefix.len() + n + 1]);
                    errno(&error)
                }
            };
            host.bad_source(reference);
            self.sequence.clear();
            return (status, result);
        }
        let position = match host.position() {
            Ok(v) if (0..=i64::from(i32::MAX)).contains(&v) => v as i32,
            Ok(_) => {
                self.sequence.clear();
                return (UNSUPPORTED, result);
            }
            Err(code) => {
                self.sequence.clear();
                return (code, result);
            }
        };
        result.address = position;
        result.has_address = 1;
        if let Err(code) = self
            .payload
            .pack(&mut self.sequence, host.generator(), rate, control)
        {
            self.sequence.clear();
            return (code, result);
        }
        let Some(count) = initial_count.checked_add(self.payload.frames as i32) else {
            self.sequence.clear();
            return (UNSUPPORTED, result);
        };
        result.count_frames = count;
        let status = host.write(self.payload.bytes()).err().unwrap_or(0);
        self.sequence.clear();
        (status, result)
    }
}
pub(crate) fn errno(error: &std::io::Error) -> i32 {
    #[cfg(unix)]
    if let Some(code) = error.raw_os_error() {
        return code;
    }
    match error.kind() {
        std::io::ErrorKind::NotFound => 2,
        std::io::ErrorKind::PermissionDenied => 13,
        std::io::ErrorKind::IsADirectory => 21,
        std::io::ErrorKind::NotADirectory => 20,
        std::io::ErrorKind::OutOfMemory => 12,
        std::io::ErrorKind::WouldBlock => 11,
        std::io::ErrorKind::StorageFull => 28,
        _ => 22,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn extended(n: f64) -> [u8; 10] {
        if n == 0.0 {
            return [0; 10];
        }
        let bits = n.to_bits();
        let e = ((bits >> 52) & 2047) as i32 - 1023 + 16383;
        let mut out = [0; 10];
        out[..2].copy_from_slice(&((e as u16) | ((bits >> 48) as u16 & 0x8000)).to_be_bytes());
        out[2..]
            .copy_from_slice(&(((bits & 0x000f_ffff_ffff_ffff) | (1 << 52)) << 11).to_be_bytes());
        out
    }
    fn source(count: i16, extended_format: bool, extra: bool) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(b"SPEC");
        b.extend_from_slice(if extended_format { b"TSQ2" } else { b"TSEQ" });
        b.extend_from_slice(&0_u32.to_le_bytes());
        for n in [count, 100, 100, 0] {
            b.extend_from_slice(&n.to_le_bytes());
        }
        for i in 0..count {
            for n in [f64::from(i) * 0.125, 90.0, 40.0, 10.0] {
                b.extend_from_slice(&extended(n));
            }
            for n in [0_i16, 0, 100] {
                b.extend_from_slice(&n.to_le_bytes());
            }
            if extended_format {
                b.extend_from_slice(&[0; 4]);
            }
            for p in 0..9 {
                for n in [p * 100, 70, p * 500, 64, 200, 250] {
                    b.extend_from_slice(&(n as i16).to_le_bytes());
                }
                if extended_format {
                    for n in [0_i16, 7, 110] {
                        b.extend_from_slice(&n.to_le_bytes());
                    }
                }
            }
            if extended_format {
                for n in [
                    59_i16,
                    513,
                    257,
                    2,
                    3,
                    if extra { 40 } else { 0 },
                    6,
                    7,
                    8,
                    9,
                    0,
                    0,
                    0,
                    0,
                ] {
                    b.extend_from_slice(&n.to_le_bytes());
                }
                if !extra {
                    let end = b.len();
                    b[end - 18..end].fill(0);
                }
            }
        }
        b
    }
    #[test]
    fn ordinary_and_klatt_records_keep_fields_defaults_last_length_and_center() {
        let mut sequence = Sequence::default();
        let mut payload = Payload::default();
        let mut generator = Wavegen::default();
        sequence.bind(&source(2, true, false)).unwrap();
        payload
            .pack(&mut sequence, &mut generator, 22050, 1)
            .unwrap();
        assert_eq!(payload.bytes().len(), 4 + 2 * 44);
        assert_eq!(&payload.bytes()[..4], &[40, 0, 2, 0]);
        let f = &payload.bytes()[4..48];
        assert_eq!(f[16], 40);
        assert_eq!(f[17], 0);
        assert_eq!(f[18], 1);
        assert_eq!(f[26], 50);
        assert_eq!(f[32], 62);
        assert_eq!(&f[35..39], &[44, 45, 70, 130]);
        assert_eq!(&f[39..44], &[59, 0, 1, 2, 3]);
        let last = &payload.bytes()[48..];
        assert_eq!(last[0], 2);
        assert_eq!(last[16], 0);
        sequence.bind(&source(2, true, true)).unwrap();
        payload
            .pack(&mut sequence, &mut generator, 22050, 0)
            .unwrap();
        assert_eq!(payload.bytes().len(), 132);
        assert_eq!(payload.bytes()[4], 1);
        assert_eq!(&payload.bytes()[48..53], &[40, 6, 7, 8, 9]);
        assert_eq!(&payload.bytes()[53..60], &[7; 7]);
        assert_eq!(&payload.bytes()[60..67], &[55; 7]);
        assert_eq!(payload.bytes()[67], 0);
        sequence.bind(&source(1, false, false)).unwrap();
        payload
            .pack(&mut sequence, &mut generator, 22050, 0)
            .unwrap();
        assert!(payload.klatt);
        assert_eq!(payload.bytes().len(), 68);
    }
    #[test]
    fn immediate_previous_lengths_zero_keyframes_markers_and_signed_narrowing() {
        let mut sequence = Sequence::default();
        sequence.bind(&source(3, true, false)).unwrap();
        let frames = sequence.frames_mut();
        frames[1].keyframe = 0;
        frames[1].length = 77.0;
        frames[0].length = -2.0;
        frames[0].peaks[0].freq = -200;
        frames[0].peaks[0].left = -8;
        frames[0].klatt[1] = -5;
        frames[0].markers = 2;
        let mut payload = Payload::default();
        payload
            .pack(&mut sequence, &mut Wavegen::default(), 22050, 1)
            .unwrap();
        assert_eq!(&payload.bytes()[..4], &[77, 0, 2, 0]);
        assert_eq!(payload.bytes()[20], 255);
        assert_eq!(payload.bytes()[30], 254);
        assert_eq!(payload.bytes()[44], 254);
        assert_eq!(
            i16::from_le_bytes(payload.bytes()[6..8].try_into().unwrap()),
            -200
        );
        assert_eq!(sequence.frames()[2].markers, 0);
        for f in sequence.frames_mut() {
            f.keyframe = 0;
            f.markers = 0;
        }
        payload
            .pack(&mut sequence, &mut Wavegen::default(), 22050, 1)
            .unwrap();
        assert_eq!(payload.bytes(), &[0; 4]);
        assert_eq!(sequence.frames()[0].markers, 2);
    }
    #[test]
    fn exact_keyframe_bound_and_numeric_guards_never_publish_partial_payloads() {
        let mut sequence = Sequence::default();
        let mut payload = Payload::default();
        let mut generator = Wavegen::default();
        sequence.bind(&source(25, true, true)).unwrap();
        payload
            .pack(&mut sequence, &mut generator, 22050, 0)
            .unwrap();
        assert_eq!(payload.bytes().len(), MAX_BYTES);
        sequence.bind(&source(26, true, true)).unwrap();
        assert_eq!(
            payload.pack(&mut sequence, &mut generator, 22050, 0),
            Err(UNSUPPORTED)
        );
        assert!(payload.bytes().is_empty());
        sequence.bind(&source(1, true, true)).unwrap();
        sequence.frames_mut()[0].length = f32::NAN;
        assert_eq!(
            payload.pack(&mut sequence, &mut generator, 22050, 0),
            Err(UNSUPPORTED)
        );
        assert!(payload.bytes().is_empty());
        sequence.bind(&source(1, true, true)).unwrap();
        sequence.amplitude = i16::MAX;
        sequence.frames_mut()[0].amplitude = i16::MAX;
        sequence.frames_mut()[0].peaks[0].height = i16::MAX;
        assert_eq!(
            payload.pack(&mut sequence, &mut generator, 22050, 0),
            Err(UNSUPPORTED)
        );
        assert!(payload.bytes().is_empty());
        assert!(integer(f64::INFINITY).is_err());
        assert!(integer(2147483648.0).is_err());
        assert_eq!(integer(-2147483648.0), Ok(i32::MIN));
    }
    #[derive(Default)]
    struct Effects {
        generator: Wavegen,
        calls: Vec<u8>,
        output: Vec<u8>,
        error: i32,
        position: i64,
    }
    impl Host for Effects {
        fn log(&mut self, _: &[u8]) {
            self.calls.push(1);
        }
        fn bad_source(&mut self, _: &[u8]) {
            self.calls.push(2);
        }
        fn position(&mut self) -> Result<i64, i32> {
            self.calls.push(3);
            Ok(self.position)
        }
        fn generator(&mut self) -> &mut Wavegen {
            self.calls.push(4);
            &mut self.generator
        }
        fn write(&mut self, b: &[u8]) -> Result<(), i32> {
            self.calls.push(5);
            if self.error != 0 {
                return Err(self.error);
            }
            self.output.extend_from_slice(b);
            Ok(())
        }
    }
    #[test]
    fn real_driver_orders_reads_position_generator_write_and_failure_publication() {
        let root =
            std::env::temp_dir().join(format!("espeak-spect-compile-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("input");
        let filename = path.to_string_lossy().as_bytes().to_vec();
        let request = Request {
            path: &path,
            filename: &filename,
            reference: b"input",
            rate: 22050,
            control: 1,
            initial_count: 10,
        };
        let mut workspace = Workspace::default();
        let mut host = Effects::default();
        let (status, result) = workspace.compile(&mut host, &request);
        assert_eq!(status, 2);
        assert_eq!(host.calls, [1, 2]);
        assert_eq!(result.has_address, 0);
        std::fs::write(&path, source(2, true, true)).unwrap();
        host.calls.clear();
        host.position = 7;
        let (status, result) = workspace.compile(&mut host, &request);
        assert_eq!(status, 0);
        assert_eq!(host.calls, [3, 4, 5]);
        assert_eq!(
            result,
            ResultState {
                address: 7,
                count_frames: 12,
                has_address: 1
            }
        );
        assert_eq!(host.output.len(), 132);
        assert!(workspace.sequence.frames().is_empty());
        host.calls.clear();
        host.error = 5;
        let (status, result) = workspace.compile(&mut host, &request);
        assert_eq!(status, 5);
        assert_eq!(host.calls, [3, 4, 5]);
        assert_eq!(result.count_frames, 12);
        host.calls.clear();
        host.position = i64::MAX;
        let (status, result) = workspace.compile(&mut host, &request);
        assert_eq!(status, UNSUPPORTED);
        assert_eq!(host.calls, [3]);
        assert_eq!(result.has_address, 0);
        assert!(workspace.sequence.frames().is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
