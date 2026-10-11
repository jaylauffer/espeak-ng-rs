//! Owned spectral compiler inputs: bounded retained frames, bins and source.
// Copyright (C) 2005-2007 Jonathan Duddington, 2013-2016 Reece H. Dunn.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    engine_io, ieee80,
    wavegen::{Peak as WavePeak, Wavegen, MAX_HARMONIC},
};
use std::{io, ops::Range, path::Path};
pub const UNSUPPORTED: i32 = 0x10000bff;
pub const NO_FRAMES: i32 = 0x10000cff;
pub const EOF: i32 = 0x100011ff;
pub const MAX_FILE: usize = 16 * 1024 * 1024;
pub const MAX_FRAMES: usize = i16::MAX as usize;
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Formant {
    pub freq: i16,
    pub width: i16,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Peak {
    pub freq: i16,
    pub height: i16,
    pub left: i16,
    pub right: i16,
    pub bandwidth: i16,
    pub amplitude: i16,
    pub parallel_bandwidth: i16,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub keyframe: i32,
    pub amplitude: i16,
    pub length_adjust: f32,
    pub rms: f64,
    pub time: f32,
    pub pitch: f32,
    pub length: f32,
    pub dx: f32,
    pub markers: i16,
    pub max_y: i32,
    pub klatt: [i16; 14],
    pub formants: [Formant; 9],
    pub peaks: [Peak; 9],
    pub bins: Range<usize>,
}
impl Default for Frame {
    fn default() -> Self {
        const FREQ: [i16; 9] = [200, 500, 1200, 3000, 3500, 4000, 6900, 7800, 9000];
        const WIDTH: [i16; 9] = [750, 500, 550, 550, 600, 700, 700, 700, 700];
        const BW: [i16; 9] = [89, 90, 140, 260, 260, 260, 500, 500, 500];
        let mut klatt = [0; 14];
        klatt[0] = 59;
        klatt[5] = 40;
        Self {
            keyframe: 0,
            amplitude: 100,
            length_adjust: 0.0,
            rms: 0.0,
            time: 0.0,
            pitch: 0.0,
            length: 0.0,
            dx: 0.0,
            markers: 0,
            max_y: 0,
            klatt,
            formants: [Formant::default(); 9],
            peaks: std::array::from_fn(|i| Peak {
                freq: FREQ[i],
                left: WIDTH[i],
                right: WIDTH[i],
                bandwidth: BW[i],
                parallel_bandwidth: BW[i],
                ..Peak::default()
            }),
            bins: 0..0,
        }
    }
}
/// RMS uses the live Rust wave generator, preserving its voice and side effects.
pub fn frame_rms(
    peaks: &[Peak; 9],
    amplitude: i32,
    adjust: i16,
    wavegen: &mut Wavegen,
    rate: i32,
) -> Result<f64, i32> {
    let mut converted = [WavePeak::default(); 9];
    for (out, p) in converted.iter_mut().zip(peaks) {
        let height = i32::from(p.height)
            .checked_mul(amplitude)
            .and_then(|n| n.checked_mul(i32::from(adjust)))
            .ok_or(UNSUPPORTED)?
            / 10000;
        out.height = height.wrapping_shl(8);
        out.freq = i32::from(p.freq).wrapping_shl(16);
        out.left = i32::from(p.left).wrapping_shl(16);
        out.right = i32::from(p.right).wrapping_shl(16);
    }
    let mut harmonics = [0; MAX_HARMONIC];
    let count = wavegen.peaks_to_harmspect(rate, &converted, 90 << 16, &mut harmonics, 0);
    let mut total = 0_f32;
    for h in harmonics.iter().take(count.max(0) as usize).skip(1) {
        total += (h.wrapping_mul(*h) >> 10) as f32;
    }
    Ok(f64::from(total).sqrt() / 7.25)
}
/// Legacy interpolation supports one through eight initialized points.
pub fn interpolate(xa: &[f32], ya: &[f32], x: f32) -> Result<f32, i32> {
    let n = xa.len();
    if n == 0 || n > 8 || ya.len() != n {
        return Err(UNSUPPORTED);
    }
    let mut c = [0_f32; 9];
    let mut d = c;
    let mut ns = 1_i32;
    let mut dif = (x - xa[0]).abs();
    for i in 1..=n {
        let next = (x - xa[i - 1]).abs();
        if next < dif {
            ns = i as i32;
            dif = next;
        }
        c[i] = ya[i - 1];
        d[i] = ya[i - 1];
    }
    ns -= 1;
    let mut y = ya[ns as usize];
    for m in 1..n {
        for i in 1..=n - m {
            let ho = xa[i - 1] - x;
            let hp = xa[i + m - 1] - x;
            let w = c[i + 1] - d[i];
            let den = ho - hp;
            if den == 0.0 {
                return Ok(ya[1]);
            }
            let den = w / den;
            d[i] = hp * den;
            c[i] = ho * den;
        }
        y += if 2 * ns < (n - m) as i32 {
            c[(ns + 1) as usize]
        } else {
            let value = d[ns as usize];
            ns -= 1;
            value
        };
    }
    Ok(y)
}
struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}
impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], i32> {
        let end = self.position.checked_add(n).ok_or(EOF)?;
        let bytes = self.bytes.get(self.position..end).ok_or(EOF)?;
        self.position = end;
        Ok(bytes)
    }
    fn short(&mut self) -> Result<i16, i32> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn word(&mut self) -> Result<u32, i32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn float(&mut self) -> Result<f32, i32> {
        let n = ieee80::extended_to_double(self.take(10)?.try_into().unwrap()) as f32;
        if !n.is_finite() {
            return Err(UNSUPPORTED);
        }
        Ok(n)
    }
}
/// Source, decoded frames and harmonic bins grow only to admitted high-water marks.
/// No frame owns a separate allocation. Failed parse results must not be consumed.
#[derive(Default)]
pub struct Sequence {
    bytes: Vec<u8>,
    scratch: Vec<u8>,
    frames: Vec<Frame>,
    bins: Vec<u16>,
    name: Range<usize>,
    pub amplitude: i16,
    pub max_y: i16,
    pub max_x: i32,
    pub format: i32,
    pub duration: i32,
}
impl Sequence {
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }
    pub fn bins(&self) -> &[u16] {
        &self.bins
    }
    pub fn name(&self) -> &[u8] {
        self.bytes.get(self.name.clone()).unwrap_or_default()
    }
    pub fn retained_capacities(&self) -> [usize; 3] {
        [
            self.bytes.capacity() + self.scratch.capacity(),
            self.frames.capacity(),
            self.bins.capacity(),
        ]
    }
    pub fn clear(&mut self) {
        self.bytes.clear();
        self.scratch.clear();
        self.frames.clear();
        self.bins.clear();
        self.name = 0..0;
        self.amplitude = 0;
        self.max_y = 1;
        self.max_x = 3000;
        self.format = 0;
        self.duration = 0;
    }
    pub fn load(&mut self, path: &Path) -> Result<(), LoadError> {
        engine_io::read_file_into(path, MAX_FILE, &mut self.scratch).map_err(LoadError::Io)?;
        std::mem::swap(&mut self.bytes, &mut self.scratch);
        self.scratch.clear();
        self.decode().map_err(LoadError::Format)
    }
    pub fn bind(&mut self, bytes: &[u8]) -> Result<(), i32> {
        if bytes.len() > MAX_FILE {
            return Err(UNSUPPORTED);
        }
        self.bytes.clear();
        self.bytes.try_reserve_exact(bytes.len()).map_err(|_| 12)?;
        self.bytes.extend_from_slice(bytes);
        self.decode()
    }
    fn decode(&mut self) -> Result<(), i32> {
        self.frames.clear();
        self.bins.clear();
        self.name = 0..0;
        self.amplitude = 0;
        self.max_y = 1;
        self.max_x = 3000;
        self.format = 0;
        self.duration = 0;
        let mut r = Reader {
            bytes: &self.bytes,
            position: 0,
        };
        let first = r.word()?;
        let second = r.word()?;
        if first != 0x43455053 {
            return Err(UNSUPPORTED);
        }
        self.format = match second {
            0x51455354 => 0,
            0x4b455354 => 1,
            0x32515354 => 2,
            _ => return Err(UNSUPPORTED),
        };
        let length = r.word()? as usize;
        let start = r.position;
        r.take(length)?;
        self.name = start..r.position;
        let count = r.short()?;
        self.amplitude = r.short()?;
        self.max_y = r.short()?;
        r.short()?;
        if count == 0 {
            return Err(NO_FRAMES);
        }
        if count < 0 {
            return Err(UNSUPPORTED);
        }
        let count = count as usize;
        self.frames.try_reserve_exact(count).map_err(|_| 12)?;
        // All bins are u16 words in this source; reserve one bounded arena.
        self.bins
            .try_reserve_exact(self.bytes.len() / 2)
            .map_err(|_| 12)?;
        let auto_y = self.max_y == 0;
        if auto_y {
            self.max_y = 1;
        }
        for _ in 0..count {
            let mut f = Frame {
                time: r.float()?,
                pitch: r.float()?,
                length: r.float()?,
                dx: r.float()?,
                ..Frame::default()
            };
            let nx = r.short()? as u16;
            f.markers = r.short()?;
            f.amplitude = r.short()?;
            // The retained C sample loop uses a signed-short cursor.
            if nx > i16::MAX as u16 {
                return Err(UNSUPPORTED);
            }
            if self.format == 2 {
                r.short()?;
                r.short()?;
            }
            for i in 0..9 {
                f.formants[i] = Formant {
                    freq: r.short()?,
                    width: r.short()?,
                };
                let p = &mut f.peaks[i];
                p.freq = r.short()?;
                p.height = r.short()?;
                p.left = r.short()?;
                p.right = r.short()?;
                if p.height > 0 {
                    f.keyframe = 1;
                }
                if self.format == 2 {
                    p.bandwidth = r.short()?;
                    p.amplitude = r.short()?;
                    p.parallel_bandwidth = r.short()?;
                }
            }
            if self.format > 0 {
                for p in &mut f.klatt {
                    *p = r.short()?;
                }
            }
            let data = r.take(usize::from(nx) * 2)?;
            let start = self.bins.len();
            for pair in data.chunks_exact(2) {
                let v = i16::from_le_bytes(pair.try_into().unwrap());
                f.max_y = f.max_y.max(i32::from(v));
                self.bins.push(v as u16);
            }
            f.bins = start..self.bins.len();
            if auto_y {
                self.max_y = self.max_y.max(f.max_y as i16);
            }
            let width = (nx as f32) * f.dx;
            if width > self.max_x as f32 {
                if width >= 2147483648.0 {
                    return Err(UNSUPPORTED);
                }
                self.max_x = width as i32;
            }
            self.frames.push(f);
        }
        let offset = self.frames[0].time;
        for f in &mut self.frames {
            f.time -= offset;
            if !f.time.is_finite() {
                return Err(UNSUPPORTED);
            }
        }
        let duration = self.frames.last().unwrap().time * 1000.0;
        if !duration.is_finite() || !(-2147483648.0..2147483648.0).contains(&f64::from(duration)) {
            return Err(UNSUPPORTED);
        }
        self.duration = duration as i32;
        self.max_x = 9000;
        self.max_y = if self.max_y < 400 { 200 } else { 29000 };
        for i in 0..self.frames.len() {
            if self.frames[i].keyframe == 0 {
                continue;
            }
            let length = if i == self.frames.len() - 1 {
                0.0
            } else {
                let mut next = i + 1;
                let mut adjust = 0_f32;
                while next < self.frames.len() - 1 {
                    if self.frames[next].keyframe != 0 {
                        break;
                    }
                    adjust += self.frames[next].length_adjust;
                    next += 1;
                }
                (f64::from(self.frames[next].time - self.frames[i].time) * 1000.0
                    + f64::from(adjust)) as f32
            };
            self.frames[i].length_adjust = self.frames[i].length - length;
        }
        Ok(())
    }
}
#[derive(Debug)]
pub enum LoadError {
    Io(io::Error),
    Format(i32),
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
    fn data(format: usize) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(b"SPEC");
        b.extend_from_slice([b"TSEQ", b"TSEK", b"TSQ2"][format]);
        b.extend_from_slice(&6_u32.to_le_bytes());
        b.extend_from_slice(b"owned\0");
        for n in [4_i16, 100, 0, 0] {
            b.extend_from_slice(&n.to_le_bytes());
        }
        for i in 0..4 {
            for n in [4.0 + i as f64 * 0.125, 90.0, 40.0, 10.0] {
                b.extend_from_slice(&extended(n));
            }
            for n in [3_i16, i as i16, 100] {
                b.extend_from_slice(&n.to_le_bytes());
            }
            if format == 2 {
                b.extend_from_slice(&[0; 4]);
            }
            for p in 0..9 {
                for n in [
                    p * 100,
                    70,
                    p * 150,
                    if i == 0 || i == 3 { p * 5 } else { 0 },
                    200,
                    250,
                ] {
                    b.extend_from_slice(&(n as i16).to_le_bytes());
                }
                if format == 2 {
                    for n in [80_i16, 9, 110] {
                        b.extend_from_slice(&n.to_le_bytes());
                    }
                }
            }
            if format > 0 {
                for n in 0..14 {
                    b.extend_from_slice(&(n as i16).to_le_bytes());
                }
            }
            for n in [200_i16, 32767, -1] {
                b.extend_from_slice(&n.to_le_bytes());
            }
        }
        b
    }
    #[test]
    fn all_formats_decode_fields_signed_bins_and_length_adjustments() {
        let mut s = Sequence::default();
        for format in 0..3 {
            s.bind(&data(format)).unwrap();
            assert_eq!(s.name(), b"owned\0");
            assert_eq!(s.frames().len(), 4);
            assert_eq!(
                (s.amplitude, s.max_x, s.max_y, s.duration, s.format),
                (100, 9000, 29000, 375, format as i32)
            );
            assert_eq!(s.bins(), [200, 32767, 65535].repeat(4));
            let f = &s.frames()[0];
            assert_eq!(f.time, 0.0);
            assert_eq!(f.length_adjust, -335.0);
            assert_eq!(f.keyframe, 1);
            assert_eq!(s.frames()[3].length_adjust, 40.0);
            assert_eq!(s.frames()[1].keyframe, 0);
            assert_eq!(
                f.formants[3],
                Formant {
                    freq: 300,
                    width: 70
                }
            );
            assert_eq!(f.peaks[3].freq, 450);
            assert_eq!(f.peaks[3].bandwidth, if format == 2 { 80 } else { 260 });
            assert_eq!(f.klatt[5], if format == 0 { 40 } else { 5 });
            assert_eq!(f.rms, 0.0);
        }
    }
    #[test]
    fn every_truncated_prefix_rejects_and_complete_arenas_are_reused() {
        let mut s = Sequence::default();
        for format in 0..3 {
            let b = data(format);
            s.bind(&b).unwrap();
            let capacity = s.retained_capacities();
            for end in 0..b.len() {
                assert_eq!(s.bind(&b[..end]), Err(EOF), "format {format} at {end}");
            }
            s.bind(&b).unwrap();
            assert_eq!(s.retained_capacities(), capacity);
            s.clear();
            assert!(s.frames().is_empty());
            assert_eq!(s.retained_capacities(), capacity);
        }
    }
    #[test]
    fn unsupported_counts_ids_and_nonfinite_fields_are_admitted_safely() {
        let mut s = Sequence::default();
        let mut b = data(2);
        b[0] = 0;
        assert_eq!(s.bind(&b), Err(UNSUPPORTED));
        let mut b = data(2);
        b[18..20].copy_from_slice(&0_i16.to_le_bytes());
        assert_eq!(s.bind(&b), Err(NO_FRAMES));
        b[18..20].copy_from_slice(&(-1_i16).to_le_bytes());
        assert_eq!(s.bind(&b), Err(UNSUPPORTED));
        let mut b = data(2);
        b[26..36].copy_from_slice(&[0x7f, 0xff, 0x80, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(s.bind(&b), Err(UNSUPPORTED));
        let mut b = data(2);
        b[66..68].copy_from_slice(&u16::MAX.to_le_bytes());
        assert_eq!(s.bind(&b), Err(UNSUPPORTED));
        let b = vec![0; MAX_FILE + 1];
        assert_eq!(s.bind(&b), Err(UNSUPPORTED));
    }
    #[test]
    fn real_repeated_reads_keep_buffers_and_failed_open_keeps_the_old_result() {
        let root = std::env::temp_dir().join(format!("espeak-spect-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("sequence");
        let b = data(2);
        std::fs::write(&path, &b).unwrap();
        let mut s = Sequence::default();
        s.load(&path).unwrap();
        s.load(&path).unwrap();
        let capacity = s.retained_capacities();
        let frames = s.frames().to_vec();
        for _ in 0..16 {
            s.load(&path).unwrap();
            assert_eq!(s.frames(), frames);
            assert_eq!(s.retained_capacities(), capacity);
        }
        assert!(matches!(
            s.load(&root.join("missing")),
            Err(LoadError::Io(_))
        ));
        assert_eq!(s.frames(), frames);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn interpolation_bounds_repeated_points_and_rms_checked_products() {
        assert_eq!(
            interpolate(&[0.0, 1.0, 2.0], &[0.0, 1.0, 4.0], 1.5),
            Ok(2.25)
        );
        assert_eq!(interpolate(&[0.0, 0.0], &[1.0, 3.0], 0.5), Ok(3.0));
        assert_eq!(interpolate(&[4.0], &[7.0], 9.0), Ok(7.0));
        assert!(interpolate(&[], &[], 0.0).is_err());
        assert!(interpolate(&[0.0; 9], &[1.0; 9], 0.0).is_err());
        let f = Frame::default();
        let mut w = Wavegen::default();
        assert_eq!(
            frame_rms(&f.peaks, 100, f.amplitude, &mut w, 22050),
            Ok(0.0)
        );
        let mut peaks = f.peaks;
        peaks[0].height = i16::MAX;
        assert!(frame_rms(&peaks, i32::MAX, i16::MAX, &mut w, 22050).is_err());
    }
}
