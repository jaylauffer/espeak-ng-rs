//! Compatibility `CalcLengths` over a copied phoneme-list snapshot, with
//! engine callbacks for embedded speed commands and tone envelopes.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::lengths::{self, Entry, Error, Host, Settings};
use std::{ffi::c_void, mem::size_of, slice};

// Matches RustLengthEntry and RustLengthSettings in rust_data.h.
const _: () = assert!(size_of::<Entry>() == 28 && size_of::<Settings>() == 260);

type Embedded = unsafe extern "C" fn(*mut c_void, *mut i32) -> i32;
type ToneEnvelope = unsafe extern "C" fn(*mut c_void, usize, *mut u8) -> i32;

#[repr(C)]
pub struct LengthHost {
    context: *mut c_void,
    embedded: Option<Embedded>,
    tone_envelope: Option<ToneEnvelope>,
}

struct Callbacks {
    context: *mut c_void,
    embedded: Embedded,
    tone_envelope: ToneEnvelope,
}

impl Host for Callbacks {
    fn embedded(&mut self) -> Result<[i32; 3], Error> {
        let mut speeds = [0; 3];
        // SAFETY: serialized owner callback; writes three exclusive ints.
        match unsafe { (self.embedded)(self.context, speeds.as_mut_ptr()) } {
            0 => Ok(speeds),
            _ => Err(Error::Host),
        }
    }
    fn tone_envelope(&mut self, index: usize) -> Result<u8, Error> {
        let mut first = 0;
        // SAFETY: serialized owner callback; writes one exclusive byte.
        match unsafe { (self.tone_envelope)(self.context, index, &mut first) } {
            0 => Ok(first),
            _ => Err(Error::Host),
        }
    }
}

/// Returns 0, 1 for invalid admission, or 2 + the native error. On a nonzero
/// result the entries and `more_syllables` should be discarded/are unchanged.
#[no_mangle]
unsafe extern "C" fn espeak_rs_calc_lengths(
    entries: *mut Entry,
    length: usize,
    count: usize,
    settings: *const Settings,
    more_syllables: *mut i32,
    host: *const LengthHost,
    bad_envelopes: *mut u32,
) -> i32 {
    if (entries.is_null() && length != 0)
        || length > lengths::MAX_ENTRIES
        || settings.is_null()
        || more_syllables.is_null()
        || host.is_null()
        || bad_envelopes.is_null()
    {
        return 1;
    }
    // SAFETY: nonnull initialized host description.
    let host = unsafe { &*host };
    let (Some(embedded), Some(tone_envelope)) = (host.embedded, host.tone_envelope) else {
        return 1;
    };
    let mut callbacks = Callbacks {
        context: host.context,
        embedded,
        tone_envelope,
    };
    let entries: &mut [Entry] = if length == 0 {
        &mut []
    } else {
        // SAFETY: length initialized exclusive entries, disjoint from the
        // settings and outputs; callbacks never touch this snapshot.
        unsafe { slice::from_raw_parts_mut(entries, length) }
    };
    // SAFETY: nonnull initialized settings copy and exclusive outputs.
    let (settings, more_syllables) = unsafe { (&*settings, &mut *more_syllables) };
    match lengths::calc_lengths(entries, count, settings, more_syllables, &mut callbacks) {
        Ok(bad) => {
            // SAFETY: exclusive output.
            unsafe { *bad_envelopes = bad };
            0
        }
        Err(error) => 2 + error as i32,
    }
}
