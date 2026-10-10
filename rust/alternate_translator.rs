//! Alternate-language translator admission, replacement, recovery and release.
// Copyright (C) 2005-2014 Jonathan Duddington; 2015-2017 Reece H. Dunn.
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
use crate::translate_word::Error;

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Delete,
    Clear,
    Table,
    Repeat,
    Select,
}
pub trait Host {
    fn present(&self, slot: u32) -> bool;
    fn cached(&self, slot: u32, index: usize) -> Option<u8>;
    fn cache(&mut self, slot: u32, index: usize, byte: u8) -> Result<(), Error>;
    fn lookup(&mut self, name: &[u8; 20]) -> Result<i32, Error>;
    /// Install a newly allocated translator or fail before dereferencing it.
    /// The primitive copies the name; it retains no pointer into Rust scratch.
    fn create(&mut self, slot: u32, name: &[u8; 20]) -> Result<(), Error>;
    fn load(&mut self, slot: u32) -> Result<i32, Error>;
    fn main_table(&self) -> i32;
    fn effect(&mut self, slot: u32, effect: Effect, value: i32) -> Result<(), Error>;
}
fn same(h: &impl Host, slot: u32, name: &[u8; 20]) -> Result<bool, Error> {
    for (i, &byte) in name.iter().enumerate() {
        if h.cached(slot, i).ok_or(Error::State)? != byte {
            return Ok(false);
        }
        if byte == 0 {
            return Ok(true);
        }
    }
    Err(Error::State)
}
pub fn set(h: &mut impl Host, slot: u32, language: &[u8]) -> Result<i32, Error> {
    if slot >= 2 || language.len() >= 20 || language.contains(&0) {
        return Err(Error::Source);
    }
    let mut name = [0; 20];
    name[..language.len()].copy_from_slice(language);
    let mut table = h.lookup(&name)?;
    if table >= 0 {
        if h.present(slot) && !same(h, slot, &name)? {
            h.effect(slot, Effect::Delete, 0)?;
            h.effect(slot, Effect::Clear, 0)?;
        }
        if !h.present(slot) {
            h.create(slot, &name)?;
            if !h.present(slot) {
                return Err(Error::State);
            }
            for (i, &byte) in name.iter().take(language.len() + 1).enumerate() {
                h.cache(slot, i, byte)?;
            }
            if h.load(slot)? != 0 {
                h.effect(slot, Effect::Select, h.main_table())?;
                table = -1;
                h.cache(slot, 0, 0)?;
            }
            h.effect(slot, Effect::Table, table)?;
        }
    }
    if h.present(slot) {
        h.effect(slot, Effect::Repeat, 0)?;
    }
    Ok(table)
}
pub fn free(h: &mut impl Host) -> Result<(), Error> {
    for slot in 0..2 {
        // DeleteTranslator accepts null; preserve both calls and their order.
        h.effect(slot, Effect::Delete, 0)?;
        h.effect(slot, Effect::Clear, 0)?;
        h.cache(slot, 0, 0)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Mock {
        present: [bool; 2],
        names: [[u8; 20]; 2],
        tables: [i32; 2],
        repeat: [u8; 2],
        lookup: i32,
        load: i32,
        main: i32,
        allocation_fails: bool,
        events: Vec<(u32, u32, i32)>,
    }
    impl Host for Mock {
        fn present(&self, s: u32) -> bool {
            self.present[s as usize]
        }
        fn cached(&self, s: u32, i: usize) -> Option<u8> {
            self.names[s as usize].get(i).copied()
        }
        fn cache(&mut self, s: u32, i: usize, b: u8) -> Result<(), Error> {
            self.events.push((5, s, i as i32));
            self.names[s as usize][i] = b;
            Ok(())
        }
        fn lookup(&mut self, _: &[u8; 20]) -> Result<i32, Error> {
            self.events.push((6, 0, self.lookup));
            Ok(self.lookup)
        }
        fn create(&mut self, s: u32, _: &[u8; 20]) -> Result<(), Error> {
            self.events.push((7, s, 0));
            if self.allocation_fails {
                return Err(Error::State);
            }
            self.present[s as usize] = true;
            Ok(())
        }
        fn load(&mut self, s: u32) -> Result<i32, Error> {
            self.events.push((8, s, self.load));
            self.main = 73;
            Ok(self.load)
        }
        fn main_table(&self) -> i32 {
            self.main
        }
        fn effect(&mut self, s: u32, e: Effect, v: i32) -> Result<(), Error> {
            self.events.push((e as u32, s, v));
            match e {
                Effect::Delete => {}
                Effect::Clear => self.present[s as usize] = false,
                Effect::Table => self.tables[s as usize] = v,
                Effect::Repeat => self.repeat[s as usize] = 0,
                Effect::Select => {}
            }
            Ok(())
        }
    }
    fn existing() -> Mock {
        let mut h = Mock {
            present: [true, false],
            lookup: 13,
            tables: [9, 0],
            repeat: [7, 8],
            ..Mock::default()
        };
        h.names[0][..3].copy_from_slice(b"en\0");
        h
    }
    #[test]
    fn cached_language_reuses_resource_without_reassigning_table() {
        let mut h = existing();
        assert_eq!(set(&mut h, 0, b"en"), Ok(13));
        assert_eq!(h.tables[0], 9);
        assert_eq!(h.repeat, [0, 8]);
        assert_eq!(h.events, [(6, 0, 13), (3, 0, 0)]);
    }
    #[test]
    fn replacement_preserves_cache_tail_and_ordered_publication() {
        let mut h = existing();
        h.names[0][..9].copy_from_slice(b"longtail\0");
        assert_eq!(set(&mut h, 0, b"fr"), Ok(13));
        assert_eq!(&h.names[0][..9], b"fr\0gtail\0");
        assert_eq!(
            h.events,
            [
                (6, 0, 13),
                (0, 0, 0),
                (1, 0, 0),
                (7, 0, 0),
                (5, 0, 0),
                (5, 0, 1),
                (5, 0, 2),
                (8, 0, 0),
                (2, 0, 13),
                (3, 0, 0)
            ]
        );
    }
    #[test]
    fn failed_load_keeps_new_resource_restores_fresh_main_table_then_retries() {
        let mut h = existing();
        h.load = 1;
        assert_eq!(set(&mut h, 1, b"ru"), Ok(-1));
        assert!(h.present[1]);
        assert_eq!(h.tables[1], -1);
        assert_eq!(&h.names[1][..3], b"\0u\0");
        assert_eq!(
            &h.events[h.events.len() - 4..],
            [(4, 1, 73), (5, 1, 0), (2, 1, -1), (3, 1, 0)]
        );
        h.events.clear();
        h.load = 0;
        assert_eq!(set(&mut h, 1, b"ru"), Ok(13));
        assert_eq!(
            &h.events[..4],
            [(6, 0, 13), (0, 1, 0), (1, 1, 0), (7, 1, 0)]
        );
        assert_eq!(h.names[0][..3], *b"en\0");
    }
    #[test]
    fn missing_table_preserves_cache_resource_and_resets_only_existing_repeat() {
        let mut h = existing();
        h.lookup = -5;
        let names = h.names;
        assert_eq!(set(&mut h, 0, b"xx"), Ok(-5));
        assert_eq!(h.names, names);
        assert_eq!(h.tables, [9, 0]);
        assert_eq!(h.events, [(6, 0, -5), (3, 0, 0)]);
        h.events.clear();
        assert_eq!(set(&mut h, 1, b"xx"), Ok(-5));
        assert_eq!(h.events, [(6, 0, -5)]);
    }
    #[test]
    fn release_calls_both_deleters_and_clears_only_cache_prefixes() {
        let mut h = existing();
        h.names[1][..5].copy_from_slice(b"tail\0");
        assert_eq!(free(&mut h), Ok(()));
        assert_eq!(h.present, [false, false]);
        assert_eq!(&h.names[0][..3], b"\0n\0");
        assert_eq!(&h.names[1][..5], b"\0ail\0");
        assert_eq!(
            h.events,
            [
                (0, 0, 0),
                (1, 0, 0),
                (5, 0, 0),
                (0, 1, 0),
                (1, 1, 0),
                (5, 1, 0)
            ]
        );
    }
    #[test]
    fn invalid_inputs_have_no_effect_and_failed_allocation_stops_before_load() {
        let mut h = existing();
        assert_eq!(set(&mut h, 2, b"en"), Err(Error::Source));
        assert_eq!(set(&mut h, 0, &[b'x'; 20]), Err(Error::Source));
        assert_eq!(set(&mut h, 0, b"en\0"), Err(Error::Source));
        assert!(h.events.is_empty());
        h.allocation_fails = true;
        assert_eq!(set(&mut h, 0, b"fr"), Err(Error::State));
        assert!(!h.present[0]);
        assert_eq!(&h.names[0][..3], b"en\0");
        assert_eq!(h.events, [(6, 0, 13), (0, 0, 0), (1, 0, 0), (7, 0, 0)]);
    }
}
