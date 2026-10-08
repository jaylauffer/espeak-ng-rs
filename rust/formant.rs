//! Native formant transitions over reusable owner-retained frame storage.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2018 Reece H. Dunn;
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later

use crate::{
    phoneme_data::InvalidPhonemeData as Error,
    spectrum::{self, FrameRef},
};

pub const COPIED: u16 = 0x8000;
pub const MAX_POOL_FRAMES: usize = 170;

/// Full writable frame, including the optional Klatt extension.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct Frame {
    pub flags: i16,
    pub frequencies: [i16; 7],
    pub length: u8,
    pub rms: u8,
    pub heights: [u8; 8],
    pub widths: [u8; 6],
    pub right: [u8; 3],
    pub bandwidths: [u8; 4],
    pub klatt: [u8; 5],
    pub klatt2: [u8; 5],
    pub parallel_amplitudes: [u8; 7],
    pub parallel_bandwidths: [u8; 7],
    pub spare: u8,
}
impl Frame {
    pub const ZERO: Frame = Frame {
        flags: 0,
        frequencies: [0; 7],
        length: 0,
        rms: 0,
        heights: [0; 8],
        widths: [0; 6],
        right: [0; 3],
        bandwidths: [0; 4],
        klatt: [0; 5],
        klatt2: [0; 5],
        parallel_amplitudes: [0; 7],
        parallel_bandwidths: [0; 7],
        spare: 0,
    };

    /// Decode exactly the flagged record size; ordinary padding stays zero.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let flags = bytes.get(..2).ok_or(Error("truncated formant frame"))?;
        let flags = i16::from_le_bytes([flags[0], flags[1]]);
        let size = if flags & 1 != 0 { 64 } else { 44 };
        let bytes = bytes.get(..size).ok_or(Error("truncated formant frame"))?;
        let mut frame = Self {
            flags,
            frequencies: std::array::from_fn(|i| {
                i16::from_le_bytes([bytes[2 + i * 2], bytes[3 + i * 2]])
            }),
            length: bytes[16],
            rms: bytes[17],
            ..Self::default()
        };
        frame.heights.copy_from_slice(&bytes[18..26]);
        frame.widths.copy_from_slice(&bytes[26..32]);
        frame.right.copy_from_slice(&bytes[32..35]);
        frame.bandwidths.copy_from_slice(&bytes[35..39]);
        frame.klatt.copy_from_slice(&bytes[39..44]);
        if size == 64 {
            frame.klatt2.copy_from_slice(&bytes[44..49]);
            frame.parallel_amplitudes.copy_from_slice(&bytes[49..56]);
            frame.parallel_bandwidths.copy_from_slice(&bytes[56..63]);
            frame.spare = bytes[63];
        }
        Ok(frame)
    }
    pub fn set_rms(&mut self, new_rms: i32, klatt: bool) -> Result<(), Error> {
        const SQRT: [i16; 200] = [
            0, 64, 90, 110, 128, 143, 156, 169, 181, 192, 202, 212, 221, 230, 239, 247, 256, 263,
            271, 278, 286, 293, 300, 306, 313, 320, 326, 332, 338, 344, 350, 356, 362, 367, 373,
            378, 384, 389, 394, 399, 404, 409, 414, 419, 424, 429, 434, 438, 443, 448, 452, 457,
            461, 465, 470, 474, 478, 483, 487, 491, 495, 499, 503, 507, 512, 515, 519, 523, 527,
            531, 535, 539, 543, 546, 550, 554, 557, 561, 565, 568, 572, 576, 579, 583, 586, 590,
            593, 596, 600, 603, 607, 610, 613, 617, 620, 623, 627, 630, 633, 636, 640, 643, 646,
            649, 652, 655, 658, 662, 665, 668, 671, 674, 677, 680, 683, 686, 689, 692, 695, 698,
            701, 704, 706, 709, 712, 715, 718, 721, 724, 726, 729, 732, 735, 738, 740, 743, 746,
            749, 751, 754, 757, 759, 762, 765, 768, 770, 773, 775, 778, 781, 783, 786, 789, 791,
            794, 796, 799, 801, 804, 807, 809, 812, 814, 817, 819, 822, 824, 827, 829, 832, 834,
            836, 839, 841, 844, 846, 849, 851, 853, 856, 858, 861, 863, 865, 868, 870, 872, 875,
            877, 879, 882, 884, 886, 889, 891, 893, 896, 898, 900, 902,
        ];
        if klatt {
            if new_rms == -1 {
                self.klatt[0] = 50;
            }
            return Ok(());
        }
        if self.rms == 0 {
            return Ok(());
        }
        if new_rms < 0 {
            return Err(Error("negative ordinary-frame RMS"));
        }
        let ratio =
            new_rms.checked_mul(64).ok_or(Error("frame RMS overflow"))? / i32::from(self.rms);
        let scale = i32::from(SQRT[ratio.min(199) as usize]);
        for height in &mut self.heights {
            *height = (i32::from(*height) * scale / 512) as u8;
        }
        Ok(())
    }
    fn adjust(&mut self, shape: Shape, factor: i32, klatt: bool) -> Result<(), Error> {
        let target = shape
            .target
            .checked_mul(factor)
            .ok_or(Error("formant factor overflow"))?
            / 256;
        let delta = ((target - i32::from(self.frequencies[2])) / 2)
            .min(shape.max)
            .max(shape.min);
        self.frequencies[2] = (i32::from(self.frequencies[2]) + delta) as i16;
        self.frequencies[3] = (i32::from(self.frequencies[3]) + shape.third) as i16;
        let third = if shape.flags & 32 != 0 {
            -shape.third
        } else {
            shape.third
        };
        for index in [4, 5] {
            self.frequencies[index] = (i32::from(self.frequencies[index]) + third) as i16;
        }
        let first = i32::from(self.frequencies[1]);
        let delta = match shape.first {
            1 => (235 - first).clamp(-100, -60),
            2 => (235 - first).clamp(-300, -150),
            3 => {
                let mut delta = (100 - first).max(-400);
                if delta > -300 {
                    delta = -400;
                }
                delta
            }
            _ => 0,
        };
        self.frequencies[1] = (first + delta) as i16;
        if matches!(shape.first, 2 | 3) {
            self.frequencies[0] = (i32::from(self.frequencies[0]) + delta) as i16;
        }
        if !klatt {
            for height in &mut self.heights[2..] {
                *height = (i32::from(*height) * shape.high / 100) as u8;
            }
        }
        Ok(())
    }
    fn closeness(self) -> i32 {
        match self.frequencies[1] {
            ..=299 => 3,
            300..=399 => 2,
            400..=499 => 1,
            _ => 0,
        }
    }
}
#[derive(Clone, Copy)]
struct Shape {
    target: i32,
    min: i32,
    max: i32,
    first: u32,
    third: i32,
    high: i32,
    flags: u32,
}

/// Read snapshots, mutate writable pool frames, and reserve bounded capacity.
/// No operation may allocate, block, or invalidate queued handles here.
pub trait Storage<T: Copy> {
    fn read(&self, handle: T) -> Result<Frame, Error>;
    fn writable(&self, handle: T) -> bool;
    fn write(&mut self, handle: T, frame: Frame) -> Result<(), Error>;
    fn allocate(&mut self, frame: Frame) -> Result<T, Error>;
    fn reserve(&self, _count: usize) -> Result<(), Error> {
        Ok(())
    }
}
pub fn copy_frame<T: Copy, S: Storage<T>>(
    storage: &mut S,
    handle: T,
    force: bool,
) -> Result<T, Error> {
    let mut frame = storage.read(handle)?;
    if !force && storage.writable(handle) && frame.flags as u16 & COPIED != 0 {
        return Ok(handle);
    }
    storage.reserve(1)?;
    frame.length = 0;
    frame.flags = (frame.flags as u16 | COPIED) as i16;
    storage.allocate(frame)
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct Settings {
    pub which: i32,
    pub klatt: u32,
    pub formant_factor: i32,
    pub other_glottal: u32,
    pub length_adjust: i32,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct Effects {
    pub length_adjust: i32,
    pub modulation: i32,
    pub has_modulation: u32,
    pub pause: u32,
    pub return_length: i32,
}
/// Apply a transition; return queue effects for the caller to submit.
/// Capacity/factor validation precedes mutations. Storage errors invalidate
/// partial results; owners must retain every admitted handle until consumption.
pub fn transition<T: Copy, S: Storage<T>>(
    storage: &mut S,
    sequence: &mut [FrameRef<T>],
    count: &mut usize,
    data1: u32,
    data2: u32,
    settings: Settings,
) -> Result<Effects, Error> {
    let mut effects = Effects {
        length_adjust: settings.length_adjust,
        ..Effects::default()
    };
    if *count > sequence.len() || *count > spectrum::MAX_FRAMES {
        return Err(Error("invalid formant sequence count"));
    }
    if *count < 2 {
        return Ok(effects);
    }
    let length = (data1 & 63) as i32 * 2;
    let mut rms = ((data1 >> 6) & 63) as i32;
    let flags = (data1 >> 12) | if settings.other_glottal != 0 { 8 } else { 0 };
    let shape = Shape {
        target: (data2 & 63) as i32 * 50,
        min: (((data2 >> 6) & 31) as i32 - 15) * 50,
        max: (((data2 >> 11) & 31) as i32 - 15) * 50,
        third: (((data2 >> 16) & 31) as i32 - 15) * 50,
        high: ((data2 >> 21) & 31) as i32 * 8,
        first: (data2 >> 26) & 7,
        flags,
    };
    let colour = data2 >> 29;
    let active = shape.target != 0 || flags != 0;
    let extends = settings.which != 1 && active && flags & 8 == 0;
    if extends && *count == sequence.len() {
        return Err(Error("formant transition exceeds frame capacity"));
    }
    if shape.target != 0 && (settings.which == 1 || flags & 8 == 0) {
        shape
            .target
            .checked_mul(settings.formant_factor)
            .ok_or(Error("formant factor overflow"))?;
    }
    if extends && length > 36 {
        effects.length_adjust = effects
            .length_adjust
            .checked_add(length - 36)
            .ok_or(Error("formant length adjustment overflow"))?;
    }
    // Reserve all necessary copies before changing any frame or sequence entry.
    let needs_copy = |storage: &S, handle| -> Result<usize, Error> {
        Ok(usize::from(
            !storage.writable(handle) || storage.read(handle)?.flags as u16 & COPIED == 0,
        ))
    };
    let last = *count - 1;
    let mut needed = if settings.which == 1 {
        needs_copy(storage, sequence[0].frame)?
    } else if active && flags & 8 != 0 {
        needs_copy(storage, sequence[last].frame)?
    } else {
        usize::from(extends)
    };
    if settings.which != 1 && active && (1..=2).contains(&colour) {
        for entry in &sequence[..*count] {
            needed += needs_copy(storage, entry.frame)?;
        }
        if flags & 8 != 0 {
            needed -= needs_copy(storage, sequence[last].frame)?;
        }
    }
    storage.reserve(needed)?;
    let mut changed = None;
    if settings.which == 1 {
        let handle = copy_frame(storage, sequence[0].frame, false)?;
        sequence[0].frame = handle;
        sequence[0].length = if length > 0 { length as i16 } else { 50 };
        sequence[0].flags |= 0x4000;
        let mut frame = storage.read(handle)?;
        frame.flags |= 0x4000;
        let next = storage.read(sequence[1].frame)?;
        if settings.klatt != 0 {
            frame.klatt[0] = next.klatt[0].wrapping_sub(4);
        }
        if shape.target != 0 {
            if rms & 32 != 0 {
                frame.set_rms(i32::from(next.rms) * (rms & 31) / 30, settings.klatt != 0)?;
            }
            frame.adjust(shape, settings.formant_factor, settings.klatt != 0)?;
            if rms & 32 == 0 {
                frame.set_rms(rms * 2, settings.klatt != 0)?;
            }
        } else {
            frame.set_rms(
                if flags & 8 != 0 {
                    i32::from(next.rms) * 24 / 32
                } else {
                    28
                },
                settings.klatt != 0,
            )?;
        }
        if flags & 8 != 0 {
            effects.modulation = 0x800 + (frame.closeness() << 8);
            effects.has_modulation = 1;
        }
        storage.write(handle, frame)?;
        changed = Some(handle);
    } else if active {
        rms *= 2;
        let handle = if flags & 8 != 0 {
            let handle = copy_frame(storage, sequence[last].frame, false)?;
            sequence[last].frame = handle;
            rms = 35;
            effects.modulation = 0x400 + (storage.read(handle)?.closeness() << 8);
            effects.has_modulation = 1;
            handle
        } else {
            let handle = copy_frame(storage, sequence[last].frame, true)?;
            sequence[last].length = length as i16;
            sequence[*count].frame = handle;
            sequence[*count].length = 0; // Preserve the legacy reference flag slot.
            *count += 1;
            let mut frame = storage.read(handle)?;
            if shape.target != 0 {
                frame.adjust(shape, settings.formant_factor, settings.klatt != 0)?;
            }
            storage.write(handle, frame)?;
            handle
        };
        let mut frame = storage.read(handle)?;
        frame.set_rms(rms, settings.klatt != 0)?;
        storage.write(handle, frame)?;
        changed = Some(handle);
        if (1..=2).contains(&colour) {
            const COLOURS: [[i32; 5]; 2] = [[243, 272, 256, 256, 256], [256, 256, 240, 240, 240]];
            for entry in &mut sequence[..*count] {
                let handle = copy_frame(storage, entry.frame, false)?;
                entry.frame = handle;
                let mut frame = storage.read(handle)?;
                for (frequency, factor) in frame.frequencies[1..6]
                    .iter_mut()
                    .zip(COLOURS[colour as usize - 1])
                {
                    *frequency = (i32::from(*frequency) * factor / 256) as i16;
                }
                storage.write(handle, frame)?;
                changed = Some(handle); // C applies terminal flags to the last coloured frame.
            }
        }
    }
    if let Some(handle) = changed {
        let mut frame = storage.read(handle)?;
        if flags & 4 != 0 {
            frame.flags |= 32;
        }
        if flags & 2 != 0 {
            frame.flags |= 16;
        }
        storage.write(handle, frame)?;
    }
    effects.pause = if flags & 64 != 0 { 20 } else { 0 };
    effects.return_length = if flags & 16 != 0 { length } else { 0 };
    Ok(effects)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    Resident(usize),
    Pool { slot: usize, generation: u64 },
}
impl Default for Handle {
    fn default() -> Self {
        Self::Resident(0)
    }
}
#[derive(Default)]
struct Slot {
    frame: Frame,
    generation: u64,
    occupied: bool,
}
/// Allocated once at engine initialization. Slots stay admitted until release.
pub struct Pool {
    slots: Vec<Slot>,
    cursor: usize,
    free: usize,
}
impl Pool {
    pub fn new(capacity: usize) -> Result<Self, Error> {
        if capacity == 0 || capacity > MAX_POOL_FRAMES {
            return Err(Error("formant pool capacity must be 1..=170"));
        }
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(capacity)
            .map_err(|_| Error("cannot allocate formant pool"))?;
        slots.resize_with(capacity, Slot::default);
        Ok(Self {
            slots,
            cursor: 0,
            free: capacity,
        })
    }
    pub fn available(&self) -> usize {
        self.free
    }
    fn slot(&self, handle: Handle) -> Result<&Slot, Error> {
        let Handle::Pool { slot, generation } = handle else {
            return Err(Error("resident frame is not writable"));
        };
        self.slots
            .get(slot)
            .filter(|slot| slot.occupied && slot.generation == generation)
            .ok_or(Error("stale pooled formant handle"))
    }
    pub fn frame(&self, handle: Handle) -> Result<&Frame, Error> {
        Ok(&self.slot(handle)?.frame)
    }
    pub fn release(&mut self, handle: Handle) -> Result<(), Error> {
        self.slot(handle)?;
        let Handle::Pool { slot, .. } = handle else {
            unreachable!()
        };
        self.slots[slot].occupied = false;
        self.free += 1;
        Ok(())
    }
}
/// Native spectrum environment borrowing resident bytes and a reusable pool.
/// Queue effects are returned in `effects`; the host owns submission/draining.
pub struct ResidentPool<'a> {
    pub bytes: &'a [u8],
    pub pool: &'a mut Pool,
    pub settings: Settings,
    pub effects: Effects,
}
impl ResidentPool<'_> {
    /// Clear pending queue effects, then select/blend into reusable references.
    pub fn lookup(
        &mut self,
        parameters: &spectrum::Parameters,
        settings: spectrum::Settings,
        frames: &mut [FrameRef<Handle>; spectrum::MAX_FRAMES],
    ) -> Result<spectrum::Selection, Error> {
        self.effects = Effects::default();
        spectrum::SpectrumData::new(self.bytes).lookup(parameters, settings, frames, self)
    }
}
impl Storage<Handle> for ResidentPool<'_> {
    fn read(&self, handle: Handle) -> Result<Frame, Error> {
        match handle {
            Handle::Resident(offset) => {
                if offset < 8 {
                    return Err(Error("invalid resident formant offset"));
                }
                let frame = Frame::decode(
                    self.bytes
                        .get(offset..)
                        .ok_or(Error("resident formant outside phondata"))?,
                )?;
                if frame.flags as u16 & COPIED != 0 {
                    return Err(Error("resident formant marked writable"));
                }
                Ok(frame)
            }
            _ => Ok(*self.pool.frame(handle)?),
        }
    }
    fn writable(&self, handle: Handle) -> bool {
        self.pool.slot(handle).is_ok()
    }
    fn write(&mut self, handle: Handle, frame: Frame) -> Result<(), Error> {
        self.pool.slot(handle)?;
        let Handle::Pool { slot, .. } = handle else {
            unreachable!()
        };
        self.pool.slots[slot].frame = frame;
        Ok(())
    }
    fn reserve(&self, count: usize) -> Result<(), Error> {
        if count > self.pool.available() {
            Err(Error("formant pool is full; drain/release queued frames"))
        } else {
            Ok(())
        }
    }
    fn allocate(&mut self, frame: Frame) -> Result<Handle, Error> {
        self.reserve(1)?;
        let capacity = self.pool.slots.len();
        for step in 0..capacity {
            let index = (self.pool.cursor + step) % capacity;
            let slot = &mut self.pool.slots[index];
            if slot.occupied {
                continue;
            }
            slot.generation = slot
                .generation
                .checked_add(1)
                .ok_or(Error("formant generation exhausted"))?;
            slot.frame = frame;
            slot.occupied = true;
            self.pool.free -= 1;
            self.pool.cursor = (index + 1) % capacity;
            return Ok(Handle::Pool {
                slot: index,
                generation: slot.generation,
            });
        }
        Err(Error("formant pool is full"))
    }
}
impl spectrum::Environment<Handle> for ResidentPool<'_> {
    fn resident(&mut self, frame: spectrum::Frame<'_>) -> Handle {
        Handle::Resident(frame.offset())
    }
    fn transition(
        &mut self,
        frames: &mut [FrameRef<Handle>],
        count: &mut usize,
        parameters: &spectrum::Parameters,
        which: i32,
        adjust: &mut i32,
    ) -> Result<i32, Error> {
        self.effects = transition(
            self,
            frames,
            count,
            parameters.transition0 as u32,
            parameters.transition1 as u32,
            Settings {
                which,
                length_adjust: *adjust,
                ..self.settings
            },
        )?;
        *adjust = self.effects.length_adjust;
        Ok(self.effects.return_length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn data() -> Vec<u8> {
        let mut bytes = vec![0; 8];
        for _ in 0..2 {
            let mut frame = [0; 44];
            frame[17] = 100;
            frame[18..26].fill(80);
            frame[4..6].copy_from_slice(&500_i16.to_le_bytes());
            bytes.extend_from_slice(&frame);
        }
        bytes
    }
    #[test]
    fn pool_backpressure_preserves_queued_frames_and_rejects_stale_handles() {
        let bytes = data();
        let mut pool = Pool::new(1).unwrap();
        let mut store = ResidentPool {
            bytes: &bytes,
            pool: &mut pool,
            settings: Settings::default(),
            effects: Effects::default(),
        };
        let handle = copy_frame(&mut store, Handle::Resident(8), false).unwrap();
        assert_eq!(store.pool.available(), 0);
        assert_eq!(copy_frame(&mut store, handle, false).unwrap(), handle);
        assert!(copy_frame(&mut store, handle, true).is_err());
        store.pool.release(handle).unwrap();
        let new = copy_frame(&mut store, Handle::Resident(8), true).unwrap();
        assert_ne!(new, handle);
        assert!(store.read(handle).is_err());
        assert!(store.pool.release(handle).is_err());
        assert!(Pool::new(0).is_err());
        assert!(Pool::new(171).is_err());
    }
    #[test]
    fn transitions_reserve_capacity_before_mutation_and_report_queue_effects() {
        let bytes = data();
        let mut pool = Pool::new(2).unwrap();
        let mut store = ResidentPool {
            bytes: &bytes,
            pool: &mut pool,
            settings: Settings::default(),
            effects: Effects::default(),
        };
        let mut refs = [FrameRef {
            length: 20,
            flags: 0,
            frame: Handle::Resident(8),
        }; 3];
        refs[1].frame = Handle::Resident(52);
        let mut count = 2;
        let settings = Settings {
            which: 2,
            formant_factor: 256,
            ..Settings::default()
        };
        assert!(transition(
            &mut store,
            &mut refs,
            &mut count,
            25,
            30 | (1 << 29),
            settings
        )
        .is_err()); // colour requires 3 copies
        assert_eq!(count, 2);
        assert_eq!(store.pool.available(), 2);
        assert!(transition(
            &mut store,
            &mut refs,
            &mut count,
            25,
            63,
            Settings {
                formant_factor: i32::MAX,
                ..settings
            }
        )
        .is_err());
        assert_eq!(store.pool.available(), 2);
        let result = transition(
            &mut store,
            &mut refs,
            &mut count,
            25 | ((8 | 16 | 64) << 12),
            0,
            settings,
        )
        .unwrap();
        assert_eq!(result.return_length, 50);
        assert_eq!(result.pause, 20);
        assert_eq!(result.modulation, 0x400);
        assert_eq!(result.has_modulation, 1);
        assert_eq!(store.read(refs[1].frame).unwrap().frequencies[1], 500);
        let mut one = 1;
        assert_eq!(
            transition(
                &mut store,
                &mut refs,
                &mut one,
                u32::MAX,
                u32::MAX,
                settings
            )
            .unwrap()
            .pause,
            0
        );
    }
    #[test]
    fn frame_decode_and_rms_keep_signed_frequencies_and_klatt_rules() {
        assert_eq!(std::mem::size_of::<Frame>(), 64);
        let bytes = data();
        let mut frame = Frame::decode(&bytes[8..52]).unwrap();
        assert_eq!(frame.klatt2, [0; 5]);
        assert!(Frame::decode(&bytes[8..51]).is_err());
        frame.set_rms(100, false).unwrap();
        assert_eq!(frame.heights, [80; 8]);
        assert!(frame.set_rms(-1, false).is_err());
        frame.set_rms(-1, true).unwrap();
        assert_eq!(frame.klatt[0], 50);
        frame.set_rms(20, true).unwrap();
        assert_eq!(frame.heights, [80; 8]);
    }
}
