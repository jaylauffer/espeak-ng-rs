//! Compatibility `MakePhonemeList` over the engine's first-stage list, with
//! a host callback that selects phoneme tables.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::phoneme::Phoneme;
use crate::phoneme_list::{self, Error, Host, Options, Output, Replacement, Source, Table};
use std::{ffi::c_void, mem::size_of, slice};

// Matches PHONEME_LIST2, REPLACE_PHONEMES and RustPhonemeListOutput.
const _: () =
    assert!(size_of::<Source>() == 8 && size_of::<Replacement>() == 3 && size_of::<Output>() == 28);

type Select = unsafe extern "C" fn(*mut c_void, i32, *mut *const Phoneme) -> i32;
type Invalid = unsafe extern "C" fn(*mut c_void, *const Phoneme, u32);

#[repr(C)]
pub struct ListSettings {
    table: i32,
    regression: i32,
    reduction: i32,
    stress_flags: u32,
    vowel_pause: i32,
    word_gap: i32,
    option_wordgap: i32,
    post_pause: i32,
    klatt: u32,
    mbrola: u32,
    start_sentence: u32,
    replacements: *const Replacement,
    n_replacements: usize,
    programs: *const u8,
    programs_length: usize,
}

#[repr(C)]
pub struct ListHost {
    context: *mut c_void,
    select: Option<Select>,
    invalid_instruction: Option<Invalid>,
}

struct Callbacks {
    context: *mut c_void,
    select: Select,
    invalid_instruction: Invalid,
}

impl Host for Callbacks {
    fn select(&mut self, index: i32, table: &mut Table) -> Result<(), Error> {
        let mut slots = [std::ptr::null::<Phoneme>(); 256];
        // SAFETY: serialized owner callback filling 256 exclusive slots.
        if unsafe { (self.select)(self.context, index, slots.as_mut_ptr()) } != 0 {
            return Err(Error::Host);
        }
        for (record, slot) in table.iter_mut().zip(slots) {
            // SAFETY: nonnull slots point at resident phoneme records that
            // stay valid for this serialized call.
            *record = unsafe { slot.as_ref() }.copied();
        }
        Ok(())
    }
    fn invalid_instruction(&mut self, phoneme: &Phoneme, instruction: u16) {
        // SAFETY: serialized owner callback borrowing one record.
        unsafe { (self.invalid_instruction)(self.context, phoneme, u32::from(instruction)) }
    }
}

/// Returns 0, 1 for invalid admission, or 2 + the native error. On a nonzero
/// result `source` and `output` should be discarded.
#[no_mangle]
unsafe extern "C" fn espeak_rs_make_phoneme_list(
    source: *mut Source,
    source_length: usize,
    count: *mut usize,
    output: *mut Output,
    output_capacity: usize,
    settings: *const ListSettings,
    host: *const ListHost,
    produced: *mut usize,
) -> i32 {
    if source.is_null()
        || source_length == 0
        || source_length > phoneme_list::N_LIST
        || count.is_null()
        || output.is_null()
        || output_capacity == 0
        || output_capacity > phoneme_list::MAX_OUTPUT
        || settings.is_null()
        || host.is_null()
        || produced.is_null()
    {
        return 1;
    }
    // SAFETY: nonnull initialized settings and host descriptions.
    let (settings, host) = unsafe { (&*settings, &*host) };
    let (Some(select), Some(invalid_instruction)) = (host.select, host.invalid_instruction) else {
        return 1;
    };
    if (settings.replacements.is_null() && settings.n_replacements != 0)
        || settings.n_replacements > 256
        || settings.programs.is_null()
        || settings.programs_length > isize::MAX as usize
    {
        return 1;
    }
    let replacements: &[Replacement] = if settings.n_replacements == 0 {
        &[]
    } else {
        // SAFETY: owner retains n_replacements immutable records.
        unsafe { slice::from_raw_parts(settings.replacements, settings.n_replacements) }
    };
    // SAFETY: owner retains the immutable resident phoneme programs.
    let programs = unsafe { slice::from_raw_parts(settings.programs, settings.programs_length) };
    let options = Options {
        table: settings.table,
        regression: settings.regression,
        reduction: settings.reduction,
        stress_flags: settings.stress_flags,
        vowel_pause: settings.vowel_pause,
        word_gap: settings.word_gap,
        option_wordgap: settings.option_wordgap,
        klatt: settings.klatt != 0,
        mbrola: settings.mbrola != 0,
        post_pause: settings.post_pause,
        start_sentence: settings.start_sentence != 0,
        replacements,
        programs,
    };
    let mut callbacks = Callbacks {
        context: host.context,
        select,
        invalid_instruction,
    };
    // SAFETY: exclusive initialized source and output spans and count, all
    // disjoint from the settings data; callbacks never touch them.
    let (source, output, count) = unsafe {
        (
            slice::from_raw_parts_mut(source, source_length),
            slice::from_raw_parts_mut(output, output_capacity),
            &mut *count,
        )
    };
    match phoneme_list::make_phoneme_list(source, count, output, &options, &mut callbacks) {
        Ok(n) => {
            // SAFETY: exclusive output.
            unsafe { *produced = n };
            0
        }
        Err(error) => 2 + error as i32,
    }
}
