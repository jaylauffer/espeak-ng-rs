//! The PCM output buffer the synthesizers fill (`speech.c`'s `outbuf` with
//! `wavegen.c`'s `out_ptr`/`out_end`), and the pool of modified spectrum
//! frames the command queue refers to (`synthesize.c`'s frame pool).
//!
//! [`Output`] keeps C's raw cursor layout because Klatt, speechPlayer,
//! MBROLA, sonic and the event code still advance and read it in place. The
//! buffer it owns is allocated here; a cursor may also be pointed at a buffer
//! the caller owns (as the oracle tests do).
// Copyright (C) 2005 to 2013 Jonathan Duddington, (C) 2015-2016 Reece H. Dunn;
// Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::formant::Frame;
use std::ptr;

/// The output cursor over the owned buffer (`RustOutput`): samples are
/// written at `ptr` up to `end`; `start` is the beginning of the current
/// fill.
#[repr(C)]
#[derive(Debug)]
pub struct Output {
    pub start: *mut u8,
    pub ptr: *mut u8,
    pub end: *mut u8,
    buffer: *mut u8,
    size: usize,
}

impl Default for Output {
    fn default() -> Self {
        Self::new()
    }
}

impl Output {
    pub const fn new() -> Self {
        Self {
            start: ptr::null_mut(),
            ptr: ptr::null_mut(),
            end: ptr::null_mut(),
            buffer: ptr::null_mut(),
            size: 0,
        }
    }

    /// The owned buffer, null before [`Output::reserve`].
    pub fn buffer(&self) -> *mut u8 {
        self.buffer
    }

    pub fn size(&self) -> usize {
        self.size
    }

    /// Replaces the owned buffer with `size` zeroed bytes; false when the
    /// allocation fails, keeping the old one (C's `realloc`). The cursor is
    /// reset to the empty start of the new buffer.
    pub fn reserve(&mut self, size: usize) -> bool {
        let mut bytes = Vec::new();
        if bytes.try_reserve_exact(size).is_err() {
            return false;
        }
        bytes.resize(size, 0u8);
        self.release();
        self.buffer = Box::into_raw(bytes.into_boxed_slice()).cast::<u8>();
        self.size = size;
        self.start = self.buffer;
        self.ptr = self.buffer;
        self.end = self.buffer;
        true
    }

    /// Frees the owned buffer.
    pub fn release(&mut self) {
        if !self.buffer.is_null() {
            let slice = ptr::slice_from_raw_parts_mut(self.buffer, self.size);
            // SAFETY: `buffer` came from a boxed slice of `size` bytes in
            // `reserve` and is freed once.
            drop(unsafe { Box::from_raw(slice) });
        }
        // field by field: assigning a new value would drop (release) this one
        self.start = ptr::null_mut();
        self.ptr = ptr::null_mut();
        self.end = ptr::null_mut();
        self.buffer = ptr::null_mut();
        self.size = 0;
    }

    /// Starts a fill of the whole owned buffer.
    pub fn begin(&mut self) {
        self.start = self.buffer;
        self.ptr = self.buffer;
        self.end = self.buffer.wrapping_add(self.size);
    }

    /// Bytes left (`out_end - out_ptr`).
    pub fn room(&self) -> isize {
        (self.end as isize).wrapping_sub(self.ptr as isize)
    }

    /// Bytes written since `start`.
    pub fn filled(&self) -> isize {
        (self.ptr as isize).wrapping_sub(self.start as isize)
    }

    /// Writes a sample's two bytes, low first, and advances.
    ///
    /// # Safety
    /// `ptr` and the byte after it lie in a live writable buffer, as C
    /// assumed after checking for room.
    pub unsafe fn write(&mut self, sample: i32) {
        // SAFETY: caller contract.
        unsafe {
            *self.ptr = sample as u8;
            *self.ptr.add(1) = (sample >> 8) as u8;
            self.ptr = self.ptr.add(2);
        }
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        self.release();
    }
}

pub const N_FRAME_POOL: usize = 170;

/// Copies of spectrum frames that synthesis modified, used round-robin: the
/// pool is as large as the queue, so a frame is reused only after its
/// command has played (`RustFramePool`).
#[repr(C)]
#[derive(Clone, Debug)]
pub struct FramePool {
    pub frames: [Frame; N_FRAME_POOL],
    pub cursor: i32,
}

impl Default for FramePool {
    fn default() -> Self {
        Self::new()
    }
}

impl FramePool {
    pub const fn new() -> Self {
        Self {
            frames: [Frame::ZERO; N_FRAME_POOL],
            cursor: 0,
        }
    }

    /// The next frame slot (C advanced before using it, so the first is 1).
    pub fn allocate(&mut self) -> &mut Frame {
        self.cursor = self.cursor.wrapping_add(1);
        if !(0..N_FRAME_POOL as i32).contains(&self.cursor) {
            self.cursor = 0;
        }
        &mut self.frames[self.cursor as usize]
    }

    /// Whether `frame` is the start of one of the pool's frames, so it may
    /// be modified in place.
    pub fn owns(&self, frame: *const Frame) -> bool {
        let base = self.frames.as_ptr() as usize;
        let address = frame as usize;
        address >= base
            && address - base < std::mem::size_of_val(&self.frames)
            && (address - base) % std::mem::size_of::<Frame>() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_fills_and_reports_room() {
        let mut output = Output::new();
        assert_eq!((output.room(), output.buffer().is_null()), (0, true));
        assert!(output.reserve(6));
        output.begin();
        assert_eq!((output.room(), output.filled()), (6, 0));
        // SAFETY: room for two samples.
        unsafe {
            output.write(0x1234);
            output.write(-2);
        }
        assert_eq!((output.room(), output.filled()), (2, 4));
        // SAFETY: four bytes were written.
        let bytes = unsafe { std::slice::from_raw_parts(output.start, 4) };
        assert_eq!(bytes, [0x34, 0x12, 0xfe, 0xff]);
        // a new buffer starts empty
        assert!(output.reserve(10));
        assert_eq!((output.size(), output.room()), (10, 0));
        output.begin();
        assert_eq!(output.room(), 10);
        assert!(!output.reserve(usize::MAX));
        assert_eq!(output.size(), 10);
        output.release();
        assert!(output.buffer().is_null());
    }

    #[test]
    fn frame_pool_is_round_robin() {
        let mut pool = Box::new(FramePool::new());
        let first = pool.allocate() as *mut Frame;
        assert_eq!(pool.cursor, 1);
        assert!(pool.owns(first));
        for _ in 1..N_FRAME_POOL {
            pool.allocate();
        }
        assert_eq!(pool.cursor, 0);
        assert_eq!(pool.allocate() as *mut Frame, first);
        // inside a frame, or past the pool, is not a pool frame
        assert!(!pool.owns(first.cast::<u8>().wrapping_add(2).cast()));
        assert!(!pool.owns(pool.frames.as_ptr().wrapping_add(N_FRAME_POOL)));
    }
}
