//! Bounded spectrum sequences borrowed from resident little-endian phondata.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2018 Reece H. Dunn;
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later

use crate::phoneme_data::InvalidPhonemeData as Error;

pub const MAX_FRAMES: usize = 25;
pub const ENVELOPE_LENGTH: usize = 128;

#[derive(Clone, Copy, Debug)]
pub struct Frame<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Frame<'a> {
    pub fn bytes(self) -> &'a [u8] {
        self.bytes
    }
    pub fn offset(self) -> usize {
        self.offset
    }
    pub fn flags(self) -> i16 {
        i16::from_le_bytes([self.bytes[0], self.bytes[1]])
    }
    pub fn length(self) -> u8 {
        self.bytes[16]
    }
    pub fn rms(self) -> u8 {
        self.bytes[17]
    }
    pub fn frequencies(self) -> [i16; 7] {
        std::array::from_fn(|i| i16::from_le_bytes([self.bytes[2 + i * 2], self.bytes[3 + i * 2]]))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Sequence<'a> {
    bytes: &'a [u8],
    offset: usize,
    stride: usize,
    count: usize,
}
impl<'a> Sequence<'a> {
    pub fn len(self) -> usize {
        self.count
    }
    pub fn is_empty(self) -> bool {
        self.count == 0
    }
    pub fn frame(self, index: usize) -> Option<Frame<'a>> {
        if index >= self.count {
            return None;
        }
        let start = 4 + index * self.stride;
        Some(Frame {
            bytes: &self.bytes[start..start + self.stride],
            offset: self.offset + start,
        })
    }
}

/// Same fields and ordering as the compatibility FMT_PARAMS record.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct Parameters {
    pub control: i32,
    pub use_vowel_in: i32,
    pub address: i32,
    pub length: i32,
    pub amplitude: i32,
    pub secondary_address: i32,
    pub secondary_adjust: i32,
    pub wave_address: i32,
    pub wave_amplitude: i32,
    pub transition0: i32,
    pub transition1: i32,
    pub standard_length: i32,
}
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct Settings {
    pub which: i32,
    pub is_vowel: u32,
    pub lengthened: u32,
    pub lengthen_length: i32,
}
/// A handle into owner-retained frame storage; ordinary frames need no copy.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct FrameRef<T> {
    pub length: i16,
    pub flags: i16,
    pub frame: T,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub start: usize,
    pub count: usize,
    pub length_adjust: i32,
}
/// Blending may update handles into an owner's reusable frame pool.
/// Calls are synchronous CPU work: no I/O, allocation or proactor dispatch.
pub trait Environment<T> {
    fn resident(&mut self, frame: Frame<'_>) -> T;
    fn transition(
        &mut self,
        _frames: &mut [FrameRef<T>],
        _count: &mut usize,
        _parameters: &Parameters,
        _which: i32,
        _length_adjust: &mut i32,
    ) -> Result<i32, Error> {
        Err(Error("spectrum blending requires an owner environment"))
    }
}
/// Zero-copy offsets for owners that do not request consonant blending.
pub struct Offsets;
impl Environment<usize> for Offsets {
    fn resident(&mut self, frame: Frame<'_>) -> usize {
        frame.offset()
    }
}

#[derive(Clone, Copy)]
pub struct SpectrumData<'a> {
    bytes: &'a [u8],
}
impl<'a> SpectrumData<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }
    pub fn envelope(self, address: usize) -> Result<&'a [u8; ENVELOPE_LENGTH], Error> {
        if address < 8 {
            return Err(Error("invalid envelope address"));
        }
        let end = address
            .checked_add(ENVELOPE_LENGTH)
            .ok_or(Error("envelope address overflow"))?;
        self.bytes
            .get(address..end)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(Error("truncated pitch/amplitude envelope"))
    }
    pub fn sequence(self, address: usize) -> Result<Sequence<'a>, Error> {
        if address < 8 || address % 4 != 0 {
            return Err(Error("invalid spectrum address"));
        }
        let bytes = self
            .bytes
            .get(address..)
            .ok_or(Error("spectrum address outside phondata"))?;
        let first = bytes.get(..6).ok_or(Error("truncated spectrum header"))?;
        let count = usize::from(first[2]);
        if count == 0 {
            return Err(Error("empty spectrum sequence"));
        }
        let stride = if first[4] & 1 != 0 { 64 } else { 44 };
        let bytes = bytes
            .get(..4 + count * stride)
            .ok_or(Error("truncated spectrum frames"))?;
        for frame in bytes[4..].chunks_exact(stride) {
            if frame[0] & 1 != first[4] & 1 || frame[1] & 0x80 != 0 {
                return Err(Error("inconsistent or writable resident spectrum frame"));
            }
        }
        Ok(Sequence {
            bytes,
            offset: address,
            stride,
            count,
        })
    }
    /// Reuses caller storage. On error, discard its partial contents.
    pub fn lookup<T: Copy, E: Environment<T>>(
        self,
        parameters: &Parameters,
        settings: Settings,
        output: &mut [FrameRef<T>; MAX_FRAMES],
        environment: &mut E,
    ) -> Result<Selection, Error> {
        let address =
            usize::try_from(parameters.address).map_err(|_| Error("negative spectrum address"))?;
        let main = self.sequence(address)?;
        let secondary = if parameters.secondary_address != 0 {
            Some(
                self.sequence(
                    usize::try_from(parameters.secondary_address)
                        .map_err(|_| Error("negative secondary spectrum address"))?,
                )?,
            )
        } else {
            None
        };
        let mut count = main.len().min(MAX_FRAMES - 1);
        let mut split = 0;
        for (index, slot) in output.iter_mut().take(count).enumerate() {
            let frame = main.frame(index).expect("validated frame count");
            *slot = FrameRef {
                length: i16::from(frame.length()),
                flags: frame.flags(),
                frame: environment.resident(frame),
            };
            if frame.flags() & 2 != 0 {
                split = index;
            }
        }
        let start = if split > 0 && settings.which != 1 {
            split
        } else {
            0
        };
        if split > 0 {
            if settings.which == 1 {
                count = split + 1;
            } else {
                count -= split;
            }
        }
        if secondary.is_some_and(|seq| start + count - 1 + seq.len() > MAX_FRAMES) {
            return Err(Error("combined spectrum exceeds frame capacity"));
        }
        let frames = &mut output[start..];
        let mut adjust = parameters
            .secondary_adjust
            .checked_add(parameters.length)
            .ok_or(Error("spectrum adjustment overflow"))?;
        if settings.is_vowel != 0 && secondary.is_none() && parameters.use_vowel_in != 0 {
            let added = environment.transition(
                frames,
                &mut count,
                parameters,
                settings.which,
                &mut adjust,
            )?;
            if count == 0 || count > frames.len() {
                return Err(Error("invalid blended spectrum count"));
            }
            adjust = adjust
                .checked_add(added)
                .ok_or(Error("blended spectrum adjustment overflow"))?;
        }
        let prefix_count = count - 1;
        let length1: i32 = frames[..prefix_count]
            .iter()
            .map(|frame| i32::from(frame.length))
            .sum();
        if let Some(secondary) = secondary {
            count -= 1;
            for index in 0..secondary.len() {
                let frame = secondary
                    .frame(index)
                    .expect("validated secondary frame count");
                frames[count].length = i16::from(frame.length());
                if index > 0 {
                    frames[count].frame = environment.resident(frame);
                    frames[count].flags = frame.flags();
                }
                count += 1;
            }
        }
        if length1 > 0 {
            let scaled = |value: i32| {
                value
                    .checked_mul(256)
                    .map(|v| v / length1)
                    .ok_or(Error("spectrum length factor overflow"))
            };
            let factor = if settings.which == 2 {
                let mut standard = parameters
                    .standard_length
                    .checked_add(adjust)
                    .and_then(|v| v.checked_sub(45))
                    .ok_or(Error("standard spectrum length overflow"))?
                    .max(10);
                if settings.lengthened != 0 {
                    standard = settings
                        .lengthen_length
                        .checked_mul(2)
                        .and_then(|v| standard.checked_add(v))
                        .ok_or(Error("lengthened spectrum overflow"))?;
                }
                Some(scaled(standard)?)
            } else {
                if settings.which == 1
                    && parameters.control == 1
                    && parameters.standard_length < 130
                {
                    frames[0].length = (i32::from(frames[0].length)
                        .checked_mul(parameters.standard_length)
                        .ok_or(Error("vowel front length overflow"))?
                        / 130) as i16;
                } else if settings.which != 1 && parameters.standard_length > 0 {
                    adjust = parameters
                        .standard_length
                        .checked_sub(length1)
                        .and_then(|v| adjust.checked_add(v))
                        .ok_or(Error("consonant length adjustment overflow"))?;
                }
                if adjust == 0 {
                    None
                } else {
                    Some(scaled(
                        length1
                            .checked_add(adjust)
                            .ok_or(Error("adjusted spectrum length overflow"))?,
                    )?)
                }
            };
            if let Some(factor) = factor {
                for frame in &mut frames[..prefix_count] {
                    frame.length = (i32::from(frame.length)
                        .checked_mul(factor)
                        .ok_or(Error("spectrum frame length overflow"))?
                        / 256) as i16;
                }
            }
        }
        Ok(Selection {
            start,
            count,
            length_adjust: adjust,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn append(bytes: &mut Vec<u8>, lengths: &[u8], klatt: bool, split: usize) -> usize {
        let address = bytes.len();
        bytes.extend_from_slice(&[0, 0, lengths.len() as u8, 0]);
        for (index, &length) in lengths.iter().enumerate() {
            let mut frame = vec![0; if klatt { 64 } else { 44 }];
            frame[0] = u8::from(klatt) | if index == split { 2 } else { 0 };
            frame[2..4].copy_from_slice(&(-12_i16).to_le_bytes());
            frame[16] = length;
            bytes.extend_from_slice(&frame);
        }
        address
    }
    #[test]
    fn mixed_frame_formats_split_append_and_scale_without_copying_frames() {
        let mut bytes = vec![0; 8];
        let main = append(&mut bytes, &[20, 30, 40, 0], false, 2);
        let suffix = append(&mut bytes, &[12, 0], true, 99);
        let data = SpectrumData::new(&bytes);
        assert_eq!(
            data.sequence(main).unwrap().frame(0).unwrap().frequencies()[0],
            -12
        );
        assert_eq!(
            data.sequence(suffix)
                .unwrap()
                .frame(0)
                .unwrap()
                .bytes()
                .len(),
            64
        );
        let mut out = [FrameRef::default(); MAX_FRAMES];
        let parameters = Parameters {
            address: main as i32,
            secondary_address: suffix as i32,
            standard_length: 145,
            ..Parameters::default()
        };
        let selected = data
            .lookup(
                &parameters,
                Settings {
                    which: 2,
                    ..Settings::default()
                },
                &mut out,
                &mut Offsets,
            )
            .unwrap();
        assert_eq!(
            selected,
            Selection {
                start: 2,
                count: 3,
                length_adjust: 0
            }
        );
        assert_eq!(out[2].length, 100);
        assert_eq!(out[3].length, 12);
        assert_eq!(out[3].frame, main + 4 + 3 * 44); // first suffix only sets terminal length
        assert_eq!(out[4].frame, suffix + 4 + 64);
        let selected = data
            .lookup(
                &Parameters {
                    secondary_address: 0,
                    control: 1,
                    standard_length: 65,
                    ..parameters
                },
                Settings {
                    which: 1,
                    ..Settings::default()
                },
                &mut out,
                &mut Offsets,
            )
            .unwrap();
        assert_eq!(selected.count, 3);
        assert_eq!(out[0].length, 10);
        assert_eq!(out[1].length, 30);
    }
    #[test]
    fn malformed_records_envelopes_and_combined_capacity_are_bounded() {
        let mut bytes = vec![0; 8];
        let main = append(&mut bytes, &[1; 25], false, 99);
        let suffix = append(&mut bytes, &[1; 3], false, 99);
        let data = SpectrumData::new(&bytes);
        let mut out = [FrameRef::default(); MAX_FRAMES];
        assert_eq!(
            data.lookup(
                &Parameters {
                    address: main as i32,
                    ..Parameters::default()
                },
                Settings::default(),
                &mut out,
                &mut Offsets,
            )
            .unwrap()
            .count,
            24
        ); // preserve C's reserved transition slot
        let parameters = Parameters {
            address: main as i32,
            secondary_address: suffix as i32,
            ..Parameters::default()
        };
        assert!(data
            .lookup(&parameters, Settings::default(), &mut out, &mut Offsets)
            .is_err());
        assert!(data.sequence(main + 1).is_err());
        assert!(data.sequence(0).is_err());
        assert!(SpectrumData::new(&bytes[..bytes.len() - 1])
            .sequence(suffix)
            .is_err());
        assert!(SpectrumData::new(&[0; 12]).sequence(8).is_err());
        assert!(data.envelope(usize::MAX).is_err());
        assert!(data.envelope(bytes.len() - 127).is_err());
        assert_eq!(data.envelope(8).unwrap().len(), 128);
        assert!(data.envelope(0).is_err());
        assert!(data
            .lookup(
                &Parameters {
                    secondary_address: 0,
                    standard_length: i32::MAX,
                    ..parameters
                },
                Settings {
                    which: 2,
                    ..Settings::default()
                },
                &mut out,
                &mut Offsets
            )
            .is_err());
        let mut inconsistent = bytes.clone();
        inconsistent[main + 4 + 44] |= 1;
        assert!(SpectrumData::new(&inconsistent).sequence(main).is_err());
        inconsistent[main + 4 + 44] &= !1;
        inconsistent[main + 5] |= 0x80;
        assert!(SpectrumData::new(&inconsistent).sequence(main).is_err());
    }
    #[test]
    fn owner_blending_updates_count_handles_and_adjustment_before_scaling() {
        struct Blend;
        impl Environment<usize> for Blend {
            fn resident(&mut self, frame: Frame<'_>) -> usize {
                frame.offset()
            }
            fn transition(
                &mut self,
                frames: &mut [FrameRef<usize>],
                count: &mut usize,
                _: &Parameters,
                _: i32,
                adjust: &mut i32,
            ) -> Result<i32, Error> {
                frames[*count] = FrameRef {
                    frame: 999,
                    length: 0,
                    flags: 16,
                };
                frames[*count - 1].length = 40;
                *count += 1;
                *adjust += 4;
                Ok(6)
            }
        }
        let mut bytes = vec![0; 8];
        append(&mut bytes, &[20, 0], false, 99);
        let parameters = Parameters {
            address: 8,
            use_vowel_in: 1,
            standard_length: 95,
            ..Parameters::default()
        };
        let settings = Settings {
            which: 2,
            is_vowel: 1,
            ..Settings::default()
        };
        let mut out = [FrameRef::default(); MAX_FRAMES];
        let result = SpectrumData::new(&bytes)
            .lookup(&parameters, settings, &mut out, &mut Blend)
            .unwrap();
        assert_eq!(result.count, 3);
        assert_eq!(result.length_adjust, 10);
        assert_eq!(out[2].frame, 999);
        assert_eq!(out[0].length, 20);
        assert_eq!(out[1].length, 40);
        assert!(SpectrumData::new(&bytes)
            .lookup(&parameters, settings, &mut out, &mut Offsets)
            .is_err());
    }
}
