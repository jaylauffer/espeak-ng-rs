//! The synthesis event list (`speech.c`'s `event_list`): word, sentence,
//! mark, audio, phoneme and end events recorded while a buffer is filled,
//! with their positions in the output, handed to the caller with the audio.
// Copyright (C) 2005 to 2013 Jonathan Duddington, (C) 2015-2016 Reece H. Dunn;
// Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::wavegen::to_int;
use std::ffi::c_long;
use std::ptr;

pub const EVENT_LIST_TERMINATED: i32 = 0;
pub const EVENT_MARK: i32 = 3;
pub const EVENT_PLAY: i32 = 4;
pub const EVENT_MSG_TERMINATED: i32 = 6;
pub const EVENT_PHONEME: i32 = 7;

/// The layout of `espeak_EVENT`; `id` is the union of a number, a name
/// pointer and 8 phoneme-name bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Event {
    pub kind: i32,
    pub unique_identifier: u32,
    pub text_position: i32,
    pub length: i32,
    pub audio_position: i32,
    pub sample: i32,
    pub user_data: usize,
    pub id: [u8; 8],
}

/// What a marker's position is measured from (`MarkerEvent`'s statics).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EventSettings {
    pub unique_identifier: u32,
    pub user_data: usize,
    /// Samples emitted by earlier buffers.
    pub count_samples: c_long,
    /// MBROLA's synchronization delay, in samples (0 without MBROLA).
    pub mbrola_delay: i32,
    pub samplerate: i32,
    /// `namedata`, which mark and audio events point into.
    pub names: usize,
}

/// The list (`RustEventList`): `count` events recorded of `capacity`, in a
/// buffer owned here.
#[repr(C)]
#[derive(Debug)]
pub struct EventList {
    pub events: *mut Event,
    pub count: i32,
    pub capacity: i32,
}

impl Default for EventList {
    fn default() -> Self {
        Self::new()
    }
}

impl EventList {
    pub const fn new() -> Self {
        Self {
            events: ptr::null_mut(),
            count: 0,
            capacity: 0,
        }
    }

    fn slice(&mut self) -> &mut [Event] {
        if self.events.is_null() || self.capacity <= 0 {
            return &mut [];
        }
        // SAFETY: `events` holds `capacity` events allocated in `reserve`.
        unsafe { std::slice::from_raw_parts_mut(self.events, self.capacity as usize) }
    }

    /// Resizes the list to `capacity` events, keeping those that fit (C's
    /// `realloc`); new ones are zero. False when the allocation fails or the
    /// capacity is negative, keeping the old list.
    pub fn reserve(&mut self, capacity: i32) -> bool {
        let Ok(size) = usize::try_from(capacity) else {
            return false;
        };
        let mut events = Vec::new();
        if events.try_reserve_exact(size).is_err() {
            return false;
        }
        events.resize(size, Event::default());
        let keep = self.slice().len().min(size);
        events[..keep].copy_from_slice(&self.slice()[..keep]);
        let count = self.count;
        self.release();
        self.events = Box::into_raw(events.into_boxed_slice()).cast::<Event>();
        self.capacity = capacity;
        self.count = count;
        true
    }

    /// Frees the list.
    pub fn release(&mut self) {
        if !self.events.is_null() {
            let slice = ptr::slice_from_raw_parts_mut(self.events, self.capacity.max(0) as usize);
            // SAFETY: allocated as a boxed slice of `capacity` events in
            // `reserve`, freed once.
            drop(unsafe { Box::from_raw(slice) });
        }
        self.events = ptr::null_mut();
        self.count = 0;
        self.capacity = 0;
    }

    /// `MarkerEvent`: records an event at `offset` output bytes into the
    /// current buffer, unless the list is full (two entries are kept for the
    /// terminators). Only the union bytes the event's kind uses are written;
    /// the rest keep what an earlier event left there, as in C.
    pub fn marker(
        &mut self,
        settings: &EventSettings,
        kind: i32,
        char_position: u32,
        value: i32,
        value2: i32,
        offset: isize,
    ) -> bool {
        let index = self.count;
        if self.events.is_null() || index >= self.capacity.wrapping_sub(2) || index < 0 {
            return false;
        }
        self.count += 1;
        let Some(event) = self.slice().get_mut(index as usize) else {
            return false;
        };
        event.kind = kind;
        event.unique_identifier = settings.unique_identifier;
        event.user_data = settings.user_data;
        event.text_position = (char_position & 0xffffff) as i32;
        event.length = (char_position >> 24) as i32;
        // C sums in long (with the pointer difference), then narrows
        #[allow(clippy::useless_conversion)] // c_long is 32-bit on Windows
        let sample = i64::from(settings.count_samples)
            .wrapping_add(i64::from(settings.mbrola_delay))
            .wrapping_add(offset as i64 / 2);
        event.audio_position = to_int((sample as f64 * 1000.0) / f64::from(settings.samplerate));
        event.sample = sample as i32;
        match kind {
            EVENT_MARK | EVENT_PLAY => {
                // a pointer: on 32-bit targets only the first four bytes
                let name = settings
                    .names
                    .wrapping_add_signed(value as isize)
                    .to_ne_bytes();
                event.id[..name.len()].copy_from_slice(&name);
            }
            EVENT_PHONEME => {
                event.id[..4].copy_from_slice(&value.to_ne_bytes());
                event.id[4..].copy_from_slice(&value2.to_ne_bytes());
            }
            _ => event.id[..4].copy_from_slice(&value.to_ne_bytes()),
        }
        true
    }

    /// Marks the end of the events at `index`: the terminator kind, message
    /// and user data, leaving the entry's other fields as they were.
    pub fn terminate(&mut self, index: i32, unique_identifier: u32, user_data: usize) {
        if let Some(event) = usize::try_from(index)
            .ok()
            .and_then(|ix| self.slice().get_mut(ix))
        {
            event.kind = EVENT_LIST_TERMINATED;
            event.unique_identifier = unique_identifier;
            event.user_data = user_data;
        }
    }

    /// The end of a message: a cleared message terminator, then the list
    /// terminator (`sync_espeak_terminated_msg`).
    pub fn terminated_message(&mut self, unique_identifier: u32, user_data: usize) {
        let events = self.slice();
        if events.len() < 2 {
            return;
        }
        for (event, kind) in events
            .iter_mut()
            .zip([EVENT_MSG_TERMINATED, EVENT_LIST_TERMINATED])
        {
            *event = Event {
                kind,
                unique_identifier,
                user_data,
                ..Event::default()
            };
        }
    }

    /// `RescaleEventSamples`: maps this buffer's event positions from the
    /// `length_pre` samples generated onto the `length_post` that libsonic
    /// produced.
    pub fn rescale(
        &mut self,
        length_pre: i32,
        length_post: i32,
        count_samples: c_long,
        mbrola_delay: i32,
        samplerate: i32,
    ) {
        if self.events.is_null() || length_pre <= 0 || length_pre == length_post {
            return;
        }
        // samples before this buffer, as MarkerEvent counted them (an int)
        #[allow(clippy::useless_conversion)] // c_long is 32-bit on Windows
        let base = i64::from(count_samples).wrapping_add(i64::from(mbrola_delay)) as i32;
        let count = self.count.clamp(0, self.capacity.max(0)) as usize;
        for event in &mut self.slice()[..count] {
            let mut offset = event.sample.wrapping_sub(base);
            if offset <= 0 {
                continue;
            }
            offset = offset.min(length_pre);
            offset = ((i64::from(offset) * i64::from(length_post)) / i64::from(length_pre)) as i32;
            event.sample = base.wrapping_add(offset);
            event.audio_position =
                to_int((f64::from(event.sample) * 1000.0) / f64::from(samplerate));
        }
    }
}

impl Drop for EventList {
    fn drop(&mut self) {
        self.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> EventSettings {
        EventSettings {
            unique_identifier: 7,
            user_data: 0x1234,
            count_samples: 1000,
            mbrola_delay: 0,
            samplerate: 22050,
            names: 0x5000,
        }
    }

    fn events(list: &mut EventList) -> Vec<Event> {
        list.slice().to_vec()
    }

    #[test]
    fn markers_record_positions_and_ids() {
        let mut list = EventList::new();
        assert!(!list.marker(&settings(), 1, 0, 0, 0, 0));
        assert!(list.reserve(5));
        // a word: position, length and number; the union's high bytes stay
        list.slice()[0].id = [9; 8];
        assert!(list.marker(&settings(), 1, (3 << 24) | 0x123456, 42, 0, 2 * 2205));
        let first = events(&mut list)[0];
        assert_eq!(
            (first.text_position, first.length, first.sample),
            (0x123456, 3, 3205)
        );
        assert_eq!(first.audio_position, (3205.0f64 * 1000.0 / 22050.0) as i32);
        assert_eq!(first.id[..4], 42i32.to_ne_bytes());
        assert_eq!(first.id[4..], [9; 4]);
        // a mark points into the names; a phoneme holds both values
        assert!(list.marker(&settings(), EVENT_MARK, 0, 16, 0, 0));
        assert!(list.marker(&settings(), EVENT_PHONEME, 0, 0x6261, 0x64, 0));
        let all = events(&mut list);
        assert_eq!(
            usize::from_ne_bytes(
                all[1].id[..std::mem::size_of::<usize>()]
                    .try_into()
                    .unwrap()
            ),
            0x5010
        );
        assert_eq!(all[2].id, [0x61, 0x62, 0, 0, 0x64, 0, 0, 0]);
        // two entries are kept for terminators
        assert!(!list.marker(&settings(), 1, 0, 0, 0, 0));
        assert_eq!(list.count, 3);
        list.terminate(3, 8, 9);
        let end = events(&mut list)[3];
        assert_eq!(
            (end.kind, end.unique_identifier, end.user_data),
            (EVENT_LIST_TERMINATED, 8, 9)
        );
        // growing keeps the events
        assert!(list.reserve(8));
        let grown = events(&mut list);
        assert_eq!((&grown[..3], grown[3], list.count), (&all[..3], end, 3));
        list.terminated_message(1, 2);
        let message = events(&mut list);
        assert_eq!(
            (message[0].kind, message[1].kind, message[0].id),
            (EVENT_MSG_TERMINATED, EVENT_LIST_TERMINATED, [0; 8])
        );
        assert!(!list.reserve(-1));
        list.release();
        assert!(list.events.is_null());
    }

    #[test]
    fn rescaling_maps_onto_the_shorter_buffer() {
        let mut list = EventList::new();
        assert!(list.reserve(6));
        for offset in [0, 100, 400, 1000] {
            list.marker(&settings(), 1, 0, 0, 0, offset);
        }
        list.rescale(400, 200, 1000, 0, 22050);
        let samples: Vec<i32> = events(&mut list)[..4].iter().map(|e| e.sample).collect();
        // before the buffer stays; inside is halved; past the end is clamped
        assert_eq!(samples, [1000, 1025, 1100, 1200]);
        assert_eq!(
            events(&mut list)[3].audio_position,
            (1200.0f64 * 1000.0 / 22050.0) as i32
        );
        // nothing to do for an unchanged length
        list.rescale(200, 200, 1000, 0, 22050);
        assert_eq!(events(&mut list)[1].sample, 1025);
    }
}
