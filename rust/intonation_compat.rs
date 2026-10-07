//! Compatibility `CalcPitches` over a copied phoneme-list snapshot.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::intonation::{self, Entry, Settings, Tunes};
use crate::phoneme::Phoneme;
use std::{mem::size_of, slice};

// Matches RustPitchEntry and RustPitchSettings in rust_data.h.
const _: () = assert!(size_of::<Entry>() == 14 && size_of::<Settings>() == 72);

/// Returns 0, 1 for invalid admission, or 2 + the native error. On any
/// nonzero result the entries are unchanged.
#[no_mangle]
unsafe extern "C" fn espeak_rs_calc_pitches(
    entries: *mut Entry,
    length: usize,
    table: *const *const Phoneme,
    table_length: usize,
    tunes: *const u8,
    tunes_length: usize,
    settings: *const Settings,
    clause_type: i32,
) -> i32 {
    if (entries.is_null() && length != 0)
        || length > intonation::MAX_ENTRIES
        || (tunes.is_null() && tunes_length != 0)
        || tunes_length > isize::MAX as usize
        || settings.is_null()
    {
        return 1;
    }
    // SAFETY: owner retains table_length initialized pointer slots and their
    // records for this serialized call.
    let Some(records) = (unsafe { super::borrowed_phonemes(table, table_length) }) else {
        return 1;
    };
    let entries: &mut [Entry] = if length == 0 {
        &mut []
    } else {
        // SAFETY: length initialized exclusive entries, disjoint from the
        // immutable tune bytes, table records and settings.
        unsafe { slice::from_raw_parts_mut(entries, length) }
    };
    let tunes = if tunes_length == 0 {
        &[][..]
    } else {
        // SAFETY: owner retains tunes_length initialized immutable bytes.
        unsafe { slice::from_raw_parts(tunes, tunes_length) }
    };
    // SAFETY: nonnull initialized settings copy.
    let settings = unsafe { &*settings };
    match intonation::calc_pitches(
        entries,
        &records[..table_length],
        Tunes::new(tunes),
        settings,
        clause_type,
    ) {
        Ok(()) => 0,
        Err(error) => 2 + error as i32,
    }
}
