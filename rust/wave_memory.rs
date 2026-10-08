//! The synthesis command queue and the echo ring (`wavegen.c`'s `wcmdq` and
//! `echo_buf`).
//!
//! The command writers append entries at the tail; the wave generator
//! consumes them from the head. The ring holds one free entry, so a full
//! queue has `N_WCMDQ - 1` entries. Every synthesizer adds its output to the
//! echo ring and plays back what it held one delay earlier.
// Copyright (C) 2005 to 2013 Jonathan Duddington, (C) 2015-2016 Reece H. Dunn;
// Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later

pub const N_WCMDQ: i32 = 170;
pub const N_ECHO_BUF: i32 = 5500;

const WCMD_WAVE: isize = 6;
const WCMD_MARKER: isize = 10;
const WCMD_VOICE: isize = 11;
const WCMD_EMBEDDED: isize = 12;
const WCMD_MBROLA_DATA: isize = 13;
const WCMD_SONIC_SPEED: isize = 15;
const WCMD_PHONEME_ALIGNMENT: isize = 16;

/// The queue and echo ring, in the layout C addresses (`RustWaveMemory`).
#[repr(C)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaveMemory {
    pub queue: [[isize; 4]; N_WCMDQ as usize],
    pub head: i32,
    pub tail: i32,
    pub echo_buf: [i16; N_ECHO_BUF as usize],
    pub echo_head: i32,
    pub echo_tail: i32,
    pub echo_amp: i32,
}

impl Default for WaveMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl WaveMemory {
    pub const fn new() -> Self {
        Self {
            queue: [[0; 4]; N_WCMDQ as usize],
            head: 0,
            tail: 0,
            echo_buf: [0; N_ECHO_BUF as usize],
            echo_head: 0,
            echo_tail: 0,
            echo_amp: 0,
        }
    }

    /// `WcmdqFree`: entries that can still be added, plus the one kept free.
    pub fn free(&self) -> i32 {
        let free = self.head.wrapping_sub(self.tail);
        if free <= 0 {
            free.wrapping_add(N_WCMDQ)
        } else {
            free
        }
    }

    /// `WcmdqUsed`.
    pub fn used(&self) -> i32 {
        N_WCMDQ.wrapping_sub(self.free())
    }

    /// `WcmdqInc`: commits the entry at the tail.
    pub fn inc_tail(&mut self) {
        self.tail = next(self.tail, N_WCMDQ);
    }

    /// `WcmdqIncHead`: drops the entry at the head.
    pub fn inc_head(&mut self) {
        self.head = next(self.head, N_WCMDQ);
    }

    /// The queue part of `WcmdqStop`: empties the queue.
    pub fn stop(&mut self) {
        self.head = 0;
        self.tail = 0;
    }

    /// Writes `words` at their slots of the tail entry, leaving the others as
    /// they were, and commits it; returns the index written.
    pub fn write(&mut self, words: &[(usize, isize)]) -> i32 {
        let index = self.tail;
        if let Some(entry) = usize::try_from(index)
            .ok()
            .and_then(|index| self.queue.get_mut(index))
        {
            for &(slot, word) in words {
                entry[slot] = word;
            }
        }
        self.inc_tail();
        index
    }

    /// Writes `words[..count]` at the tail and commits it.
    pub fn push(&mut self, words: [isize; 4], count: usize) -> i32 {
        let index = self.tail;
        if let Some(entry) = usize::try_from(index)
            .ok()
            .and_then(|index| self.queue.get_mut(index))
        {
            let count = count.min(4);
            entry[..count].copy_from_slice(&words[..count]);
        }
        self.inc_tail();
        index
    }

    /// `DoMarker`: an event marker, when more than five entries are free.
    pub fn marker(&mut self, kind: i32, char_posn: i32, length: i32, value: i32) -> bool {
        if self.free() <= 5 {
            return false;
        }
        self.write(&[
            (0, WCMD_MARKER + (kind.wrapping_shl(8) as isize)),
            (1, marker_position(char_posn, length)),
            (2, value as isize),
        ]);
        true
    }

    /// `DoPhonemeMarker`: a phoneme event with up to 8 bytes of its name,
    /// stored from the third word on (on 32-bit targets, the third and fourth).
    pub fn phoneme_marker(
        &mut self,
        kind: i32,
        char_posn: i32,
        length: i32,
        name: [u8; 8],
    ) -> bool {
        if self.free() <= 5 {
            return false;
        }
        let index = self.write(&[
            (0, WCMD_MARKER + (kind.wrapping_shl(8) as isize)),
            (1, marker_position(char_posn, length)),
        ]);
        if let Some(entry) = usize::try_from(index)
            .ok()
            .and_then(|index| self.queue.get_mut(index))
        {
            const WORD: usize = std::mem::size_of::<isize>();
            let mut bytes = [0u8; 2 * WORD];
            bytes[..WORD].copy_from_slice(&entry[2].to_ne_bytes());
            bytes[WORD..].copy_from_slice(&entry[3].to_ne_bytes());
            bytes[..8].copy_from_slice(&name);
            entry[2] = isize::from_ne_bytes(bytes[..WORD].try_into().unwrap_or_default());
            entry[3] = isize::from_ne_bytes(bytes[WORD..].try_into().unwrap_or_default());
        }
        true
    }

    /// `DoPhonemeAlignment`: a phoneme name for the alignment hook, which
    /// frees it.
    pub fn phoneme_alignment(&mut self, name: usize, kind: i32) {
        self.write(&[
            (0, WCMD_PHONEME_ALIGNMENT),
            (1, name as isize),
            (2, kind as isize),
        ]);
    }

    /// `DoSonicSpeed`: the speed-up factor times 1024.
    pub fn sonic_speed(&mut self, value: i32) {
        self.write(&[(0, WCMD_SONIC_SPEED), (1, value as isize)]);
    }

    /// `DoVoiceChange`: a voice copy, which the generator frees.
    pub fn voice_change(&mut self, voice: usize) {
        self.write(&[(0, WCMD_VOICE), (2, voice as isize)]);
    }

    /// MBROLA output of `length`.
    pub fn mbrola_data(&mut self, length: i32) {
        self.write(&[(0, WCMD_MBROLA_DATA), (1, length as isize)]);
    }

    /// A sound icon: 16-bit samples at `data`, amplitude 21.
    pub fn sound_icon(&mut self, length: i32, data: usize) {
        self.write(&[
            (0, WCMD_WAVE),
            (1, length as isize),
            (2, data as isize),
            (3, 0x1500),
        ]);
    }

    /// An embedded command for the generator.
    pub fn embedded(&mut self, command: i32, value: u32) {
        self.write(&[
            (0, WCMD_EMBEDDED),
            (1, command as isize),
            (2, value as isize),
        ]);
    }

    /// The entry at `index`; zeros outside the queue (C read past it).
    pub fn entry(&self, index: i32) -> [isize; 4] {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.queue.get(index))
            .copied()
            .unwrap_or([0; 4])
    }

    /// The echo ring's tail sample, advancing the tail.
    pub fn echo_take(&mut self) -> i32 {
        let value = usize::try_from(self.echo_tail)
            .ok()
            .and_then(|tail| self.echo_buf.get(tail))
            .map_or(0, |&sample| i32::from(sample));
        self.echo_tail = next(self.echo_tail, N_ECHO_BUF);
        value
    }

    /// The tail sample times the echo amplitude (1/256ths), advancing the
    /// tail: what each synthesizer adds, after `>> 8`.
    pub fn echo(&mut self) -> i32 {
        self.echo_take().wrapping_mul(self.echo_amp)
    }

    /// Stores a sample (as a short) at the head and advances it. A head past
    /// the ring, from a delay longer than the ring at a high sample rate, is
    /// not written; C wrote past the ring once there.
    pub fn echo_put(&mut self, sample: i32) {
        if let Some(slot) = usize::try_from(self.echo_head)
            .ok()
            .and_then(|head| self.echo_buf.get_mut(head))
        {
            *slot = sample as i16;
        }
        self.echo_head = next(self.echo_head, N_ECHO_BUF);
    }

    /// Clears the ring and sets its delay (as the head) and amplitude.
    pub fn echo_reset(&mut self, head: i32, amp: i32) {
        self.echo_buf.fill(0);
        self.echo_tail = 0;
        self.echo_head = head;
        self.echo_amp = amp;
    }
}

/// A marker's second word: the character position and, above it, the
/// length, computed as a C int.
fn marker_position(char_posn: i32, length: i32) -> isize {
    ((char_posn & 0xffffff) | length.wrapping_shl(24)) as isize
}

fn next(index: i32, size: i32) -> i32 {
    let index = index.wrapping_add(1);
    if index >= size {
        0
    } else {
        index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_counts_wrap() {
        let mut memory = WaveMemory::new();
        assert_eq!((memory.free(), memory.used()), (N_WCMDQ, 0));
        for _ in 0..N_WCMDQ - 1 {
            memory.inc_tail();
        }
        assert_eq!((memory.free(), memory.used()), (1, N_WCMDQ - 1));
        memory.inc_tail(); // C does not stop a writer here: the queue reads empty
        assert_eq!((memory.tail, memory.used()), (0, 0));
        memory.head = 5;
        memory.tail = 3;
        assert_eq!(memory.free(), 2);
        for _ in 0..N_WCMDQ - 5 {
            memory.inc_head();
        }
        assert_eq!(memory.head, 0);
        memory.stop();
        assert_eq!((memory.head, memory.tail), (0, 0));
        assert_eq!(memory.entry(-1), [0; 4]);
        assert_eq!(memory.entry(N_WCMDQ), [0; 4]);
    }

    #[test]
    fn writers_set_only_their_words() {
        let mut memory = WaveMemory::new();
        memory.queue[0] = [7, 7, 7, 7];
        memory.voice_change(0x1000);
        assert_eq!(memory.queue[0], [WCMD_VOICE, 7, 0x1000, 7]);
        memory.queue[1] = [7; 4];
        memory.sonic_speed(2048);
        assert_eq!(memory.queue[1], [WCMD_SONIC_SPEED, 2048, 7, 7]);
        memory.queue[2] = [7; 4];
        assert!(memory.marker(3, 0x1234567, 200, -5));
        let position = (0x234567i32 | 200i32.wrapping_shl(24)) as isize;
        assert_eq!(memory.queue[2], [WCMD_MARKER + (3 << 8), position, -5, 7]);
        memory.queue[3] = [7; 4];
        assert!(memory.phoneme_marker(7, 9, 1, *b"abcdefgh"));
        let entry = memory.queue[3];
        let bytes: Vec<u8> = entry[2..].iter().flat_map(|w| w.to_ne_bytes()).collect();
        assert_eq!(&bytes[..8], b"abcdefgh");
        assert_eq!(
            entry[..2],
            [WCMD_MARKER + (7 << 8), (9 | (1 << 24)) as isize]
        );
        memory.queue[4] = [7; 4];
        memory.embedded(0x45, 0xffffff);
        assert_eq!(memory.queue[4], [WCMD_EMBEDDED, 0x45, 0xffffff, 7]);
        memory.sound_icon(100, 0x2000);
        assert_eq!(memory.queue[5], [WCMD_WAVE, 100, 0x2000, 0x1500]);
        memory.mbrola_data(500);
        memory.phoneme_alignment(0x3000, 2);
        assert_eq!(memory.queue[6][..2], [WCMD_MBROLA_DATA, 500]);
        assert_eq!(memory.queue[7][..3], [WCMD_PHONEME_ALIGNMENT, 0x3000, 2]);
        assert_eq!(memory.push([1, 2, 3, 4], 2), 8);
        assert_eq!(memory.tail, 9);
        // markers need more than five free entries
        memory.head = 15;
        assert_eq!(memory.free(), 6);
        assert!(memory.marker(1, 0, 0, 0));
        assert!(!memory.marker(1, 0, 0, 0));
        assert!(!memory.phoneme_marker(1, 0, 0, [0; 8]));
        assert_eq!(memory.tail, 10);
    }

    #[test]
    fn echo_ring_delays_and_scales() {
        let mut memory = WaveMemory::new();
        memory.echo_buf[0] = 9;
        memory.echo_reset(3, 128);
        assert_eq!(memory.echo_buf[0], 0);
        for sample in [100, -200, 300, 70000] {
            memory.echo_put(sample);
        }
        assert_eq!((memory.echo(), memory.echo(), memory.echo()), (0, 0, 0));
        assert_eq!(memory.echo(), 100 * 128);
        assert_eq!(memory.echo_take(), -200);
        // stored as a short
        memory.echo_take();
        assert_eq!(memory.echo_take(), i32::from(70000i32 as i16));
        // both ends wrap
        memory.echo_head = N_ECHO_BUF - 1;
        memory.echo_put(1);
        assert_eq!(
            (memory.echo_head, memory.echo_buf[N_ECHO_BUF as usize - 1]),
            (0, 1)
        );
        memory.echo_tail = N_ECHO_BUF - 1;
        assert_eq!((memory.echo_take(), memory.echo_tail), (1, 0));
        // a head past the ring is skipped, then wraps
        memory.echo_head = N_ECHO_BUF + 10;
        let before = memory.echo_buf;
        memory.echo_put(5);
        assert_eq!((memory.echo_buf, memory.echo_head), (before, 0));
    }
}
