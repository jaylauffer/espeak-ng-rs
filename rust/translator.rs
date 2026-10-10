//! Typed translator storage and ownership of pinned dictionary projections.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{dictionary_storage::Snapshot, language::Language, rules::MISSING};
use std::{
    alloc::{alloc, alloc_zeroed, Layout},
    io, ptr,
    sync::Arc,
};

#[cfg(windows)]
pub(crate) type Wide = u16;
#[cfg(not(windows))]
pub(crate) type Wide = u32;
pub(crate) type DictionaryHandle = Arc<Snapshot>;

/// Compatibility field layout. Static tables and pinned snapshot bytes are
/// views, never separately owned allocations. No reference crosses C effects.
#[repr(C)]
pub(crate) struct LanguageOptions {
    word_gap: i32,
    vowel_pause: i32,
    stress_rule: i32,
    stress_flags: i32,
    unstressed_wd1: i32,
    unstressed_wd2: i32,
    param: [i32; 18],
    length_mods: *const u8,
    length_mods0: *const u8,
    numbers: i32,
    numbers2: i32,
    break_numbers: u32,
    max_roman: i32,
    min_roman: i32,
    thousands_sep: i32,
    decimal_sep: i32,
    max_digits: i32,
    ordinal_indicator: *const u8,
    roman_suffix: *const u8,
    accents: i32,
    tone_language: i32,
    intonation_group: i32,
    tunes: [u8; 6],
    long_stop: i32,
    max_initial_consonants: u8,
    spelling_stress: u8,
    tone_numbers: u8,
    ideographs: u8,
    textmode: u8,
    dotless_i: u8,
    listx: i32,
    replace_chars: *const u8,
    our_alphabet: i32,
    alt_alphabet: i32,
    alt_alphabet_lang: i32,
    max_lengthmod: i32,
    lengthen_tonic: i32,
    suffix_add_e: i32,
    lowercase_sentence: u8,
}

/// Rust allocation owns this complete record and its unique small snapshot
/// handle. C temporarily accesses the same typed layout under serialized ABI
/// admission; callers must drain all views before replacement or destruction.
#[repr(C)]
pub struct Translator {
    langopts: LanguageOptions,
    translator_name: i32,
    transpose_max: i32,
    transpose_min: i32,
    transpose_map: *const u8,
    dictionary_name: [u8; 40],
    phonemes_repeat: [u8; 20],
    phonemes_repeat_count: i32,
    phoneme_tab_ix: i32,
    stress_amps: [u8; 8],
    stress_lengths: [i16; 8],
    dict_condition: i32,
    dict_min_size: i32,
    encoding: i32,
    char_plus_apostrophe: *const Wide,
    punct_within_word: *const Wide,
    chars_ignore: *const u16,
    letter_bits: [u8; 256],
    letter_bits_offset: i32,
    letter_groups: [*const Wide; 8],
    letter_group_lengths: [usize; 8],
    punct_to_tone: [[u8; 6]; 8],
    data_dictrules: *mut u8,
    data_dictlist: *mut u8,
    data_dict_size: usize,
    rust_dictionary_owner: *mut DictionaryHandle,
    rule_text_base: *const u8,
    rule_text_length: usize,
    rust_list_replacement: [u8; 160],
    rust_emoji_replacement: [u8; 160],
    dict_hashtab: [*mut u8; 1024],
    letter_groups_rules: [*mut u8; 95],
    groups1: [*mut u8; 256],
    groups3: [*mut u8; 128],
    groups2: [*mut u8; 120],
    groups2_name: [u32; 120],
    n_groups2: i32,
    groups2_count: [u8; 256],
    groups2_start: [u8; 256],
    frequent_pairs: *const i16,
    expect_verb: i32,
    expect_past: i32,
    expect_verb_s: i32,
    expect_noun: i32,
    prev_last_stress: i32,
    clause_end: *mut u8,
    word_vowel_count: i32,
    word_stressed_count: i32,
    clause_upper_count: i32,
    clause_lower_count: i32,
    prepause_timeout: i32,
    end_stressed_vowel: i32,
    prev_dict_flags: [i32; 2],
    clause_terminator: i32,
}
impl Translator {
    #[cfg(feature = "c-abi")]
    pub(crate) fn letter_value(&self, field: u32) -> i32 {
        if field == 0 {
            self.translator_name
        } else {
            self.langopts.accents
        }
    }
    #[cfg(feature = "c-abi")]
    pub(crate) fn list_repeat(&self) -> ([u8; 20], i32) {
        (self.phonemes_repeat, self.phonemes_repeat_count)
    }
    #[cfg(feature = "c-abi")]
    pub(crate) fn list_set_repeat(&mut self, text: &[u8; 20], count: i32) {
        self.phonemes_repeat = *text;
        self.phonemes_repeat_count = count;
    }
    #[cfg(feature = "c-abi")]
    pub(crate) fn list_mode(&self) -> bool {
        self.langopts.textmode != 0
    }
    #[cfg(feature = "c-abi")]
    pub(crate) fn list_source(&self) -> *const u8 {
        self.rust_list_replacement.as_ptr().wrapping_add(2)
    }
    #[cfg(feature = "c-abi")]
    pub(crate) fn list_replace(&mut self, text: &[u8; 160]) -> *mut u8 {
        self.rust_list_replacement = *text;
        self.rust_list_replacement.as_mut_ptr().wrapping_add(2)
    }
    #[cfg(feature = "c-abi")]
    pub(crate) fn rule_set_source(&mut self, base: *const u8, length: usize) {
        self.rule_text_base = base;
        self.rule_text_length = length;
    }
    #[cfg(feature = "c-abi")]
    pub(crate) fn lookup_alphabet(&self) -> (i32, i32, *const u8, *const i16) {
        (
            self.transpose_min,
            self.transpose_max,
            self.transpose_map,
            self.frequent_pairs,
        )
    }
    #[cfg(feature = "c-abi")]
    pub(crate) fn lookup_bucket(&self, hash: usize) -> (*const u8, *const u8) {
        (
            self.dict_hashtab[hash].cast_const(),
            self.data_dictrules.cast_const(),
        )
    }
    #[cfg(feature = "c-abi")]
    pub(crate) fn lookup_clause(&self, next: *const u8) -> (u32, usize, u32) {
        (
            u32::from(self.clause_terminator & 0x80000 != 0),
            (self.clause_end as usize).saturating_sub(next as usize),
            u32::from(self.langopts.textmode != 0),
        )
    }
    #[cfg(feature = "c-abi")]
    pub(crate) fn lookup_context(
        &self,
        end: u32,
        word: u32,
        symbol: u32,
        native: bool,
    ) -> crate::lookup::Context {
        crate::lookup::Context {
            conditions: self.dict_condition as u32,
            end_flags: end,
            word_flags: word,
            lookup_symbol: symbol,
            language: self.translator_name as u32,
            previous_flags: self.prev_dict_flags[0] as u32,
            expect_verb: self.expect_verb,
            expect_verb_s: self.expect_verb_s,
            expect_past: self.expect_past,
            expect_noun: self.expect_noun,
            native_translator: u32::from(native),
            ..Default::default()
        }
    }
    #[cfg(feature = "c-abi")]
    pub(crate) fn match_dictionary(&self) -> (*mut u8, usize) {
        (self.data_dictlist, self.data_dict_size)
    }
    #[cfg(feature = "c-abi")]
    pub(crate) fn match_patterns(&self, group: u32) -> Option<(*const u8, usize)> {
        let &patterns = self.letter_groups_rules.get(group as usize)?;
        if patterns.is_null() {
            return None;
        }
        let offset = (patterns as usize).checked_sub(self.data_dictlist as usize)?;
        (offset < self.data_dict_size)
            .then(|| (patterns.cast_const(), self.data_dict_size - offset))
    }
    #[cfg(feature = "c-abi")]
    pub(crate) fn match_context(
        &self,
        word_flags: u32,
        dictionary_flags: u32,
        trace: u32,
        signed_bytes: u32,
    ) -> crate::rule_match::Context {
        crate::rule_match::Context {
            conditions: self.dict_condition as u32,
            word_flags,
            dictionary_flags,
            vowel_count: self.word_vowel_count,
            stressed_count: self.word_stressed_count,
            expect_verb: self.expect_verb,
            tone_numbers: self.langopts.tone_numbers as std::ffi::c_char as i32,
            suffix_options: self.langopts.param[15],
            trace,
            word_start: 0,
            signed_bytes,
        }
    }
    #[cfg(feature = "c-abi")]
    pub(crate) fn match_letters(&self) -> crate::ffi::RawLetters {
        crate::ffi::RawLetters {
            bits: ptr::addr_of!(self.letter_bits),
            groups: ptr::addr_of!(self.letter_groups).cast(),
            lengths: ptr::addr_of!(self.letter_group_lengths),
            offset: self.letter_bits_offset,
            wide_bytes: std::mem::size_of::<Wide>() as u32,
        }
    }
    #[cfg(feature = "c-abi")]
    pub(crate) fn rule_value(&self, field: u32, index: u32) -> i32 {
        match field {
            0 => i32::from(!self.data_dictrules.is_null()),
            1 => self.letter_bits_offset,
            2 => self.langopts.tone_numbers as std::ffi::c_char as i32,
            4 => self.langopts.param[0],
            5 => self.langopts.alt_alphabet,
            6 => self.langopts.alt_alphabet_lang,
            8 => self.langopts.param[17],
            9 => self.langopts.param[12],
            11 => self
                .groups2_count
                .get(index as usize)
                .map_or(-1, |&v| i32::from(v)),
            12 => self
                .groups2_start
                .get(index as usize)
                .map_or(-1, |&v| i32::from(v)),
            13 => self
                .groups2_name
                .get(index as usize)
                .map_or(-1, |&v| v as i32),
            _ => 0,
        }
    }
    #[cfg(feature = "c-abi")]
    pub(crate) fn rule_store(&mut self, field: u32, value: i32) {
        match field {
            0 => self.word_vowel_count = value,
            1 => self.word_stressed_count = value,
            2 => self.phonemes_repeat_count = value,
            _ => (),
        }
    }
    #[cfg(feature = "c-abi")]
    pub(crate) fn rule_source(&self) -> (*const u8, usize) {
        (self.rule_text_base, self.rule_text_length)
    }
    #[cfg(feature = "c-abi")]
    pub(crate) fn rule_group(&self, kind: u32, index: u32) -> usize {
        let group = match kind {
            0 => self.groups3.get(index as usize),
            1 => self.groups2.get(index as usize),
            2 => self.groups1.get(index as usize),
            _ => None,
        };
        let Some(&group) = group else {
            return usize::MAX - 1;
        };
        if group.is_null() {
            return usize::MAX;
        }
        (group as usize)
            .checked_sub(self.data_dictlist as usize)
            .filter(|&offset| offset < self.data_dict_size)
            .unwrap_or(usize::MAX - 1)
    }
    pub fn new(name: &[u8]) -> io::Result<Box<Self>> {
        let language =
            Language::new(name).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        // SAFETY: every field is an integer/array/raw pointer and admits zero;
        // matching allocation layout is retained by the resulting Box. This
        // avoids a large transient stack copy and handles allocation failure.
        let allocation = unsafe { alloc_zeroed(Layout::new::<Self>()) }.cast::<Self>();
        if allocation.is_null() {
            return Err(io::ErrorKind::OutOfMemory.into());
        }
        // SAFETY: nonnull properly aligned unique allocation, all fields valid,
        // no snapshot handle installed yet; Box now owns it exactly once.
        let mut owner = unsafe { Box::from_raw(allocation) };
        owner.initialize(&language);
        Ok(owner)
    }
    fn initialize(&mut self, l: &Language) {
        let p = l.preset;
        let f = &l.settings;
        let v = &l.options;
        let o = &mut self.langopts;
        o.word_gap = v.word_gap;
        o.vowel_pause = v.vowel_pause;
        o.stress_rule = v.stress_rule;
        o.stress_flags = v.stress_flags as i32;
        o.unstressed_wd1 = v.unstressed_single;
        o.unstressed_wd2 = v.unstressed_multiple;
        o.param = v.parameters;
        o.numbers = v.numbers as i32;
        o.numbers2 = v.numbers2 as i32;
        o.thousands_sep = v.thousands_separator;
        o.decimal_sep = v.decimal_separator;
        o.intonation_group = v.intonation_group;
        o.lowercase_sentence = u8::from(v.lowercase_sentence != 0);
        o.spelling_stress = u8::from(v.spelling_stress != 0);
        o.tunes = v.tunes;
        o.break_numbers = f.break_numbers;
        o.max_roman = f.max_roman;
        o.min_roman = f.min_roman;
        o.max_digits = f.max_digits;
        o.accents = f.accents;
        o.tone_language = f.tone_language;
        o.long_stop = f.long_stop;
        o.max_initial_consonants = f.max_initial_consonants as u8;
        o.tone_numbers = f.tone_numbers as u8;
        o.ideographs = f.ideographs as u8;
        o.textmode = f.textmode as u8;
        o.dotless_i = f.dotless_i as u8;
        o.listx = f.listx;
        o.our_alphabet = f.our_alphabet;
        o.alt_alphabet = f.alt_alphabet;
        o.alt_alphabet_lang = f.alt_alphabet_lang;
        o.max_lengthmod = f.max_lengthmod;
        o.lengthen_tonic = f.lengthen_tonic;
        o.suffix_add_e = f.suffix_add_e;
        o.length_mods = p.lengths.as_ptr();
        o.length_mods0 = p.last_lengths.as_ptr();
        o.ordinal_indicator = p.ordinal.map_or(ptr::null(), |v| v.as_ptr());
        o.roman_suffix = p.roman.as_ptr();
        self.dict_min_size = v.dictionary_minimum;
        self.dict_condition = v.dictionary_conditions as i32;
        self.stress_lengths = v.stress_lengths;
        self.stress_amps = v.stress_amplitudes;
        self.translator_name = l.selector as i32;
        self.transpose_min = f.transpose_min;
        self.transpose_max = f.transpose_max;
        self.transpose_map = p.transpose_map.map_or(ptr::null(), |v| v.as_ptr());
        self.frequent_pairs = p.pairs.map_or(ptr::null(), |v| v.as_ptr());
        self.encoding = f.encoding;
        self.letter_bits_offset = f.letter_bits_offset;
        self.chars_ignore = p.ignored.as_ptr();
        self.dictionary_name[..l.dictionary().len()].copy_from_slice(l.dictionary());
        self.letter_bits = *p.letter_bits;
        for (row, source) in self
            .punct_to_tone
            .iter_mut()
            .zip(p.punct_to_tone.chunks_exact(6))
        {
            row.copy_from_slice(source);
        }
        #[cfg(windows)]
        let (apostrophe, punctuation, groups) =
            (p.apostrophe_wide, p.punctuation_wide, p.groups_wide);
        #[cfg(not(windows))]
        let (apostrophe, punctuation, groups) = (p.apostrophe, p.punctuation, p.groups);
        self.char_plus_apostrophe = apostrophe.as_ptr();
        self.punct_within_word = punctuation.as_ptr();
        self.letter_groups = groups.map(|v| v.map_or(ptr::null(), |v| v.as_ptr()));
        self.letter_group_lengths = groups.map(|v| v.map_or(0, |v| v.len() - 1));
    }
    pub fn dictionary_name(&self) -> &[u8] {
        &self.dictionary_name[..self
            .dictionary_name
            .iter()
            .position(|&v| v == 0)
            .unwrap_or(40)]
    }
    pub fn dictionary(&self) -> Option<&Snapshot> {
        if self.rust_dictionary_owner.is_null() {
            return None;
        }
        // SAFETY: this private unique handle is installed only by bind; its
        // Arc pins immutable bytes for this borrow and until replacement/drop.
        Some(unsafe { &**self.rust_dictionary_owner })
    }
    pub fn bind(&mut self, snapshot: Arc<Snapshot>, name: &[u8]) -> io::Result<()> {
        let bytes = snapshot.bytes();
        let rules = snapshot.rules();
        let length = bytes.len();
        if name.len() >= 40
            || name.contains(&0)
            || snapshot.rules_offset() > length
            || rules.pair_count > 120
            || snapshot.buckets().iter().any(|&offset| offset >= length)
            || rules
                .singles
                .iter()
                .chain(&rules.offsets)
                .chain(&rules.pairs)
                .chain(&rules.letters)
                .chain(std::iter::once(&rules.replacements))
                .any(|&offset| offset != MISSING && offset >= length)
        {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let base = bytes.as_ptr().cast_mut();
        // SAFETY: all numeric indices above are in the pinned initialized byte
        // extent; optional views use the missing sentinel. C treats these raw
        // projections as immutable bytes. No foreign callback occurs here.
        let offset = |i: usize| {
            if i == MISSING {
                ptr::null_mut()
            } else {
                // SAFETY: this offset was checked against the pinned byte span.
                unsafe { base.add(i) }
            }
        };
        // Allocate the small handle before publishing any replacement views.
        // SAFETY: allocation uses the exact small handle layout; no owner
        // mutation occurs if it fails. Initialize before Box-compatible use.
        let handle = unsafe { alloc(Layout::new::<DictionaryHandle>()) }.cast::<DictionaryHandle>();
        if handle.is_null() {
            return Err(io::ErrorKind::OutOfMemory.into());
        }
        // SAFETY: nonnull aligned exclusive storage receives its first value.
        unsafe {
            handle.write(Arc::clone(&snapshot));
        }
        let previous = std::mem::replace(&mut self.rust_dictionary_owner, handle);
        self.data_dictlist = base;
        self.data_dict_size = length;
        self.data_dictrules = offset(snapshot.rules_offset());
        self.n_groups2 = rules.pair_count as i32;
        self.groups1 = rules.singles.map(offset);
        self.groups3 = rules.offsets.map(offset);
        self.groups2 = rules.pairs.map(offset);
        self.groups2_name = rules.pair_names;
        self.groups2_start = rules.pair_starts;
        self.groups2_count = rules.pair_counts;
        self.letter_groups_rules = rules.letters.map(offset);
        self.langopts.replace_chars = offset(rules.replacements);
        self.dict_hashtab = snapshot.buckets().map(offset);
        self.dictionary_name[..name.len()].copy_from_slice(name);
        self.dictionary_name[name.len()] = 0;
        if !previous.is_null() {
            // SAFETY: previous unique handle was installed by this owner; all
            // projections have rebound before its Arc is released exactly once.
            unsafe {
                drop(Box::from_raw(previous));
            }
        }
        Ok(())
    }
}
impl Drop for Translator {
    fn drop(&mut self) {
        let handle = std::mem::replace(&mut self.rust_dictionary_owner, ptr::null_mut());
        if !handle.is_null() {
            // SAFETY: unique handle installed by bind, caller drained all views
            // before owner teardown. Remaining fields contain non-owning views.
            unsafe {
                drop(Box::from_raw(handle));
            }
        }
    }
}

#[cfg(feature = "c-abi")]
pub(crate) fn layout(kind: u32, index: usize) -> usize {
    use std::mem::{align_of, offset_of, size_of};
    match kind {
        0 => size_of::<Translator>(),
        1 => align_of::<Translator>(),
        2 => size_of::<LanguageOptions>(),
        3 => align_of::<LanguageOptions>(),
        4 => [
            offset_of!(Translator, langopts),
            offset_of!(Translator, translator_name),
            offset_of!(Translator, transpose_max),
            offset_of!(Translator, transpose_min),
            offset_of!(Translator, transpose_map),
            offset_of!(Translator, dictionary_name),
            offset_of!(Translator, phonemes_repeat),
            offset_of!(Translator, phonemes_repeat_count),
            offset_of!(Translator, phoneme_tab_ix),
            offset_of!(Translator, stress_amps),
            offset_of!(Translator, stress_lengths),
            offset_of!(Translator, dict_condition),
            offset_of!(Translator, dict_min_size),
            offset_of!(Translator, encoding),
            offset_of!(Translator, char_plus_apostrophe),
            offset_of!(Translator, punct_within_word),
            offset_of!(Translator, chars_ignore),
            offset_of!(Translator, letter_bits),
            offset_of!(Translator, letter_bits_offset),
            offset_of!(Translator, letter_groups),
            offset_of!(Translator, letter_group_lengths),
            offset_of!(Translator, punct_to_tone),
            offset_of!(Translator, data_dictrules),
            offset_of!(Translator, data_dictlist),
            offset_of!(Translator, data_dict_size),
            offset_of!(Translator, rust_dictionary_owner),
            offset_of!(Translator, rule_text_base),
            offset_of!(Translator, rule_text_length),
            offset_of!(Translator, rust_list_replacement),
            offset_of!(Translator, rust_emoji_replacement),
            offset_of!(Translator, dict_hashtab),
            offset_of!(Translator, letter_groups_rules),
            offset_of!(Translator, groups1),
            offset_of!(Translator, groups3),
            offset_of!(Translator, groups2),
            offset_of!(Translator, groups2_name),
            offset_of!(Translator, n_groups2),
            offset_of!(Translator, groups2_count),
            offset_of!(Translator, groups2_start),
            offset_of!(Translator, frequent_pairs),
            offset_of!(Translator, expect_verb),
            offset_of!(Translator, expect_past),
            offset_of!(Translator, expect_verb_s),
            offset_of!(Translator, expect_noun),
            offset_of!(Translator, prev_last_stress),
            offset_of!(Translator, clause_end),
            offset_of!(Translator, word_vowel_count),
            offset_of!(Translator, word_stressed_count),
            offset_of!(Translator, clause_upper_count),
            offset_of!(Translator, clause_lower_count),
            offset_of!(Translator, prepause_timeout),
            offset_of!(Translator, end_stressed_vowel),
            offset_of!(Translator, prev_dict_flags),
            offset_of!(Translator, clause_terminator),
        ]
        .get(index)
        .copied()
        .unwrap_or(usize::MAX),
        5 => [
            offset_of!(LanguageOptions, word_gap),
            offset_of!(LanguageOptions, vowel_pause),
            offset_of!(LanguageOptions, stress_rule),
            offset_of!(LanguageOptions, stress_flags),
            offset_of!(LanguageOptions, unstressed_wd1),
            offset_of!(LanguageOptions, unstressed_wd2),
            offset_of!(LanguageOptions, param),
            offset_of!(LanguageOptions, length_mods),
            offset_of!(LanguageOptions, length_mods0),
            offset_of!(LanguageOptions, numbers),
            offset_of!(LanguageOptions, numbers2),
            offset_of!(LanguageOptions, break_numbers),
            offset_of!(LanguageOptions, max_roman),
            offset_of!(LanguageOptions, min_roman),
            offset_of!(LanguageOptions, thousands_sep),
            offset_of!(LanguageOptions, decimal_sep),
            offset_of!(LanguageOptions, max_digits),
            offset_of!(LanguageOptions, ordinal_indicator),
            offset_of!(LanguageOptions, roman_suffix),
            offset_of!(LanguageOptions, accents),
            offset_of!(LanguageOptions, tone_language),
            offset_of!(LanguageOptions, intonation_group),
            offset_of!(LanguageOptions, tunes),
            offset_of!(LanguageOptions, long_stop),
            offset_of!(LanguageOptions, max_initial_consonants),
            offset_of!(LanguageOptions, spelling_stress),
            offset_of!(LanguageOptions, tone_numbers),
            offset_of!(LanguageOptions, ideographs),
            offset_of!(LanguageOptions, textmode),
            offset_of!(LanguageOptions, dotless_i),
            offset_of!(LanguageOptions, listx),
            offset_of!(LanguageOptions, replace_chars),
            offset_of!(LanguageOptions, our_alphabet),
            offset_of!(LanguageOptions, alt_alphabet),
            offset_of!(LanguageOptions, alt_alphabet_lang),
            offset_of!(LanguageOptions, max_lengthmod),
            offset_of!(LanguageOptions, lengthen_tonic),
            offset_of!(LanguageOptions, suffix_add_e),
            offset_of!(LanguageOptions, lowercase_sentence),
        ]
        .get(index)
        .copied()
        .unwrap_or(usize::MAX),
        _ => usize::MAX,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{dictionary::BUCKETS, dictionary_storage::Cache};
    use std::path::Path;
    fn bytes(version: u8) -> Vec<u8> {
        let mut bytes = vec![0; 8 + BUCKETS];
        bytes[..4].copy_from_slice(&(BUCKETS as u32).to_le_bytes());
        bytes[4..8].copy_from_slice(&((8 + BUCKETS) as u32).to_le_bytes());
        bytes.extend_from_slice(&[7, version]);
        bytes
    }
    #[cfg(feature = "c-abi")]
    #[test]
    fn lookup_context_preserves_grammar_bits_and_clamps_clause_distance() {
        let mut owner = Translator::new(b"en").unwrap();
        owner.dict_condition = i32::MIN;
        owner.prev_dict_flags[0] = -1;
        owner.expect_verb = -3;
        owner.expect_verb_s = 7;
        owner.expect_past = 11;
        owner.expect_noun = 13;
        let c = owner.lookup_context(u32::MAX, 0x80000000, 0x40000000, true);
        assert_eq!((c.conditions, c.previous_flags), (0x80000000, u32::MAX));
        assert_eq!(
            (c.end_flags, c.word_flags, c.lookup_symbol),
            (u32::MAX, 0x80000000, 0x40000000)
        );
        assert_eq!(
            (c.expect_verb, c.expect_verb_s, c.expect_past, c.expect_noun),
            (-3, 7, 11, 13)
        );
        assert_eq!(c.native_translator, 1);
        let source = *b" abc \0";
        owner.clause_end = source.as_ptr().wrapping_add(4).cast_mut();
        owner.clause_terminator = 0x80000;
        owner.langopts.textmode = 1;
        assert_eq!(
            owner.lookup_clause(source.as_ptr().wrapping_add(1)),
            (1, 3, 1)
        );
        assert_eq!(
            owner.lookup_clause(source.as_ptr().wrapping_add(5)),
            (1, 0, 1)
        );
        let (base, rules) = owner.lookup_bucket(crate::dictionary::hash(b"cat"));
        assert!(base.is_null() && rules.is_null());
    }
    #[cfg(feature = "c-abi")]
    #[test]
    fn matcher_projections_bound_patterns_and_preserve_scalar_bits() {
        let mut owner = Translator::new(b"en").unwrap();
        let mut dictionary = [0; 64];
        owner.data_dictlist = dictionary.as_mut_ptr();
        owner.data_dict_size = 64;
        owner.letter_groups_rules[94] = dictionary.as_mut_ptr().wrapping_add(63);
        owner.letter_groups_rules[93] = dictionary.as_mut_ptr().wrapping_add(64);
        owner.letter_groups_rules[92] = dictionary.as_mut_ptr().wrapping_sub(1);
        assert_eq!(
            owner.match_patterns(94),
            Some((dictionary.as_ptr().wrapping_add(63), 1))
        );
        for group in [0, 92, 93, 95, u32::MAX] {
            assert!(owner.match_patterns(group).is_none());
        }
        owner.dict_condition = -17;
        owner.expect_verb = 13;
        owner.word_vowel_count = 21;
        owner.word_stressed_count = 23;
        owner.langopts.tone_numbers = 255;
        owner.langopts.param[15] = -31;
        let context = owner.match_context(0x8000_0013, 0xf000_0007, 1, 1);
        assert_eq!(context.conditions, (-17_i32) as u32);
        assert_eq!(
            (context.word_flags, context.dictionary_flags),
            (0x8000_0013, 0xf000_0007)
        );
        assert_eq!(
            (
                context.vowel_count,
                context.stressed_count,
                context.expect_verb
            ),
            (21, 23, 13)
        );
        assert_eq!(context.tone_numbers, 255_u8 as std::ffi::c_char as i32);
        assert_eq!(
            (context.suffix_options, context.trace, context.signed_bytes),
            (-31, 1, 1)
        );
        let letters = owner.match_letters();
        assert_eq!(letters.bits, ptr::addr_of!(owner.letter_bits));
        assert_eq!(letters.lengths, ptr::addr_of!(owner.letter_group_lengths));
        assert_eq!(
            letters.groups.cast::<[*const Wide; 8]>(),
            ptr::addr_of!(owner.letter_groups)
        );
        assert_eq!(letters.wide_bytes, std::mem::size_of::<Wide>() as u32);
    }
    #[cfg(feature = "c-abi")]
    #[test]
    fn rule_projections_check_groups_and_read_current_scalar_state() {
        let mut owner = Translator::new(b"en").unwrap();
        let mut dictionary = [0; 64];
        owner.data_dictlist = dictionary.as_mut_ptr();
        owner.data_dict_size = 64;
        owner.data_dictrules = dictionary.as_mut_ptr();
        owner.groups1[255] = dictionary.as_mut_ptr().wrapping_add(63);
        owner.groups2[119] = dictionary.as_mut_ptr().wrapping_add(64);
        owner.groups3[127] = dictionary.as_mut_ptr().wrapping_sub(1);
        assert_eq!(owner.rule_group(2, 255), 63);
        assert_eq!(owner.rule_group(2, 254), usize::MAX);
        for (kind, index) in [
            (1, 119),
            (0, 127),
            (0, 128),
            (1, 120),
            (2, 256),
            (3, 0),
            (2, u32::MAX),
        ] {
            assert_eq!(owner.rule_group(kind, index), usize::MAX - 1);
        }
        assert_eq!(owner.rule_value(0, 0), 1);
        owner.langopts.tone_numbers = 255;
        assert_eq!(owner.rule_value(2, 0), 255_u8 as std::ffi::c_char as i32);
        owner.groups2_count[255] = 231;
        owner.groups2_name[119] = 0x8000_0013;
        assert_eq!(owner.rule_value(11, 255), 231);
        assert_eq!(owner.rule_value(13, 119), 0x8000_0013_u32 as i32);
        assert_eq!(owner.rule_value(11, 256), -1);
        assert_eq!(owner.rule_value(12, u32::MAX), -1);
        assert_eq!(owner.rule_value(13, 120), -1);
        owner.rule_store(0, -12);
        owner.rule_store(1, 23);
        owner.rule_store(2, 34);
        owner.rule_store(99, 123);
        assert_eq!(
            (
                owner.word_vowel_count,
                owner.word_stressed_count,
                owner.phonemes_repeat_count
            ),
            (-12, 23, 34)
        );
    }
    #[test]
    fn construction_initializes_native_language_and_all_runtime_storage() {
        let owner = Translator::new(b"hu").unwrap();
        let profile = Language::new(b"hu").unwrap();
        assert_eq!(owner.dictionary_name(), profile.dictionary());
        assert_eq!(owner.translator_name as u32, profile.selector);
        assert_eq!(owner.langopts.numbers as u32, profile.options.numbers);
        assert_eq!(owner.stress_lengths, profile.options.stress_lengths);
        assert!(owner.dictionary().is_none());
        assert!(owner.data_dictlist.is_null());
        assert!(owner.dict_hashtab.iter().all(|p| p.is_null()));
        assert_eq!(owner.rust_list_replacement, [0; 160]);
        assert_eq!(owner.rust_emoji_replacement, [0; 160]);
        assert_eq!(owner.prev_dict_flags, [0; 2]);
        assert_eq!(owner.phonemes_repeat, [0; 20]);
        assert_eq!(owner.clause_terminator, 0);
    }
    #[test]
    fn construction_rejects_invalid_names_and_admits_the_initialized_limit() {
        assert!(Translator::new(&[b'a'; 39]).is_ok());
        assert_eq!(
            Translator::new(&[b'a'; 40]).err().unwrap().kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            Translator::new(b"en\0ru").err().unwrap().kind(),
            io::ErrorKind::InvalidInput
        );
    }
    #[test]
    fn dictionary_pins_survive_cache_teardown_and_drop_with_last_owner() {
        let mut cache = Cache::new(3120).unwrap();
        let snapshot = cache.resident(Path::new("en"), &bytes(0)).unwrap();
        let weak = Arc::downgrade(&snapshot);
        let mut first = Translator::new(b"en").unwrap();
        let mut second = Translator::new(b"en").unwrap();
        first.bind(Arc::clone(&snapshot), b"en").unwrap();
        second.bind(Arc::clone(&snapshot), b"en").unwrap();
        assert_eq!(first.data_dictlist, second.data_dictlist);
        assert_ne!(first.rust_dictionary_owner, second.rust_dictionary_owner);
        assert_eq!(Arc::strong_count(&snapshot), 4);
        drop(snapshot);
        drop(cache);
        assert_eq!(weak.strong_count(), 2);
        assert_eq!(first.dictionary().unwrap().bytes(), bytes(0));
        drop(first);
        assert_eq!(weak.strong_count(), 1);
        assert_eq!(second.dictionary().unwrap().bytes(), bytes(0));
        drop(second);
        assert!(weak.upgrade().is_none());
    }
    #[test]
    fn replacement_rebinds_all_views_preserves_grammar_and_releases_old_pin() {
        let mut cache = Cache::new(3120).unwrap();
        let first = cache.resident(Path::new("en"), &bytes(0)).unwrap();
        let second = cache.resident(Path::new("en"), &bytes(1)).unwrap();
        let weak = Arc::downgrade(&first);
        let mut owner = Translator::new(b"en").unwrap();
        owner.bind(first, b"long-name").unwrap();
        owner.expect_verb = 17;
        owner.prev_dict_flags = [31, 62];
        owner.langopts.numbers = 73;
        let previous = owner.data_dictlist;
        drop(cache);
        assert_eq!(weak.strong_count(), 1);
        owner.bind(second, b"fr").unwrap();
        assert_ne!(owner.data_dictlist, previous);
        assert!(weak.upgrade().is_none());
        assert_eq!(&owner.dictionary_name[..10], b"fr\0g-name\0");
        assert_eq!(owner.expect_verb, 17);
        assert_eq!(owner.prev_dict_flags, [31, 62]);
        assert_eq!(owner.langopts.numbers, 73);
        assert!(owner
            .groups1
            .iter()
            .chain(&owner.groups2)
            .chain(&owner.groups3)
            .all(|p| p.is_null()));
        assert_eq!(owner.groups2_start, [255; 256]);
        assert_eq!(owner.groups2_count, [0; 256]);
        assert_eq!(owner.n_groups2, 0);
        let snapshot = owner.dictionary().unwrap();
        assert_eq!(snapshot.bytes(), bytes(1));
        for (&pointer, &offset) in owner.dict_hashtab.iter().zip(snapshot.buckets()) {
            assert_eq!(pointer as usize, owner.data_dictlist as usize + offset);
        }
    }
    #[test]
    fn rejected_rebind_preserves_owner_views_name_and_handle() {
        let mut cache = Cache::new(3120).unwrap();
        let first = cache.resident(Path::new("en"), &bytes(0)).unwrap();
        let second = cache.resident(Path::new("en"), &bytes(1)).unwrap();
        let mut owner = Translator::new(b"en").unwrap();
        owner.bind(first, b"en").unwrap();
        let previous = (
            owner.data_dictlist,
            owner.rust_dictionary_owner,
            owner.dictionary_name,
        );
        for name in [&[b'a'; 40][..], &b"fr\0ru"[..]] {
            assert_eq!(
                owner.bind(Arc::clone(&second), name).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
            assert_eq!(
                (
                    owner.data_dictlist,
                    owner.rust_dictionary_owner,
                    owner.dictionary_name
                ),
                previous
            );
            assert_eq!(owner.dictionary().unwrap().bytes(), bytes(0));
        }
    }
}
