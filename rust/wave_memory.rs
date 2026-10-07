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
