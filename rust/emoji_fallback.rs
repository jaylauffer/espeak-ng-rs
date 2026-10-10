//! Complete emoji base/modifier/tag replacement policy.
// Copyright (C) 2005-2017 Jonathan Duddington and Reece H. Dunn.
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    common_text, suffix,
    translate_word::{Error, PHONEMES, WORD},
    utf8,
};
pub const INPUT: usize = WORD + 2;
pub trait Host {
    fn byte(&self, offset: usize) -> Option<u8>;
    /// Owned initialized input, pronunciation, flags and copied selected text.
    /// Source edits and other nested owner/state effects remain live.
    fn list(
        &mut self,
        input: &mut [u8; INPUT],
        phonemes: &mut [u8; PHONEMES],
        flags: &mut [u32; 2],
        text: &mut [u8; INPUT],
    ) -> Result<(), Error>;
    /// Checked in-place live replacement writes; do not overwrite other effects.
    fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<(), Error>;
    fn tracing(&self) -> bool;
    fn snapshot(&self, replacement: &mut [u8; WORD]) -> Result<(), Error>;
    fn trace(&mut self, source: &[u8; WORD], replacement: &[u8; WORD]);
    fn finish(&mut self) -> Result<(), Error>;
}
fn byte(host: &impl Host, offset: usize) -> Result<u8, Error> {
    host.byte(offset).ok_or(Error::Source)
}
fn code(host: &impl Host, mut offset: usize) -> Result<utf8::Character, Error> {
    while byte(host, offset)? & 0xc0 == 0x80 {
        offset = offset.checked_add(1).ok_or(Error::Source)?;
    }
    // Preserve legacy UTF width, which excludes skipped continuation bytes.
    utf8::head(|i| offset.checked_add(i).and_then(|at| host.byte(at))).map_err(|_| Error::Source)
}
fn length(text: &[u8]) -> Result<usize, Error> {
    text.iter().position(|b| *b == 0).ok_or(Error::Source)
}
pub fn lookup(host: &mut impl Host) -> Result<bool, Error> {
    if !common_text::emoji(code(host, 0)?.code) {
        return Ok(false);
    }
    let mut stripped = [0; INPUT];
    let mut modifiers = [0u32; 8];
    let mut n_modifiers = 0;
    let mut removed = false;
    let mut used = 0usize;
    let mut cursor = 0usize;
    while !matches!(byte(host, cursor)?, 0 | b' ') {
        let ch = code(host, cursor)?;
        if common_text::emoji_modifier(ch.code) {
            removed = true;
            if n_modifiers < modifiers.len() {
                modifiers[n_modifiers] = ch.code;
                n_modifiers += 1;
            }
        } else if common_text::emoji_tag(ch.code) {
            removed = true;
        } else if used + ch.width < WORD {
            for slot in &mut stripped[used..used + ch.width] {
                *slot = byte(host, cursor)?;
                cursor = cursor.checked_add(1).ok_or(Error::Source)?;
            }
            used += ch.width;
            continue;
        }
        cursor = cursor.checked_add(ch.width).ok_or(Error::Source)?;
    }
    if !removed || used == 0 {
        return Ok(false);
    }
    stripped[used] = b' ';
    let mut phonemes = [0; PHONEMES];
    let mut flags = [0; 2];
    let mut text = [0; INPUT];
    host.list(&mut stripped, &mut phonemes, &mut flags, &mut text)?;
    if flags[0] & 0x2000_0000 == 0 {
        if n_modifiers > 0 {
            return Ok(false);
        }
        text = stripped;
    }
    let size = length(&text)?;
    if size > WORD - 4 {
        return Ok(false);
    }
    host.write(0, &[0, b' '])?;
    host.write(2, &text[..=size])?;
    let mut end = size + 2;
    for &modifier in &modifiers[..n_modifiers] {
        let (bytes, width) = suffix::encode(modifier);
        let mut input = [0; INPUT];
        input[..width].copy_from_slice(&bytes[..width]);
        input[width] = b' ';
        flags = [0; 2];
        host.list(&mut input, &mut phonemes, &mut flags, &mut text)?;
        if flags[0] & 0x2000_0000 == 0 {
            continue;
        }
        let size = length(&text)?;
        if end + size >= WORD - 1 {
            break;
        }
        host.write(end, &text[..=size])?;
        end += size;
    }
    if host.tracing() {
        let mut source = [0; WORD];
        for (i, slot) in source[..WORD - 1].iter_mut().enumerate() {
            let next = byte(host, i)?;
            if matches!(next, 0 | b' ') {
                break;
            }
            *slot = next;
        }
        let mut replacement = [0; WORD];
        host.snapshot(&mut replacement)?;
        length(&replacement[2..])?;
        host.trace(&source, &replacement);
    }
    host.finish()?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        source: Vec<u8>,
        owner: [u8; WORD],
        keys: Vec<Vec<u8>>,
        base: Option<Vec<u8>>,
        modifier: Option<Vec<u8>>,
        trace: Option<(Vec<u8>, Vec<u8>)>,
        done: bool,
        effects: usize,
        mutate: bool,
        fail: bool,
        fail_after: Option<usize>,
    }
    impl Fixture {
        fn new(source: &str) -> Self {
            let mut bytes = source.as_bytes().to_vec();
            bytes.push(0);
            Self {
                source: bytes,
                owner: [0x55; WORD],
                keys: vec![],
                base: Some(b"base ".into()),
                modifier: Some(b"tone ".into()),
                trace: None,
                done: false,
                effects: 0,
                mutate: false,
                fail: false,
                fail_after: None,
            }
        }
    }
    impl Host for Fixture {
        fn byte(&self, offset: usize) -> Option<u8> {
            self.source.get(offset).copied()
        }
        fn list(
            &mut self,
            input: &mut [u8; INPUT],
            _: &mut [u8; PHONEMES],
            flags: &mut [u32; 2],
            text: &mut [u8; INPUT],
        ) -> Result<(), Error> {
            assert_eq!(*flags, [0; 2]);
            let size = length(input)?;
            self.keys.push(input[..size].into());
            self.effects += 1;
            if self.keys.len() > 1 && self.mutate {
                self.owner[2] = b'X';
                self.owner[100] = 0x79;
                self.source[0] = b'!';
            }
            let chosen = if self.keys.len() == 1 {
                &self.base
            } else {
                &self.modifier
            };
            if let Some(chosen) = chosen {
                flags[0] = 0x2000_0000;
                text.fill(0);
                text[..chosen.len()].copy_from_slice(chosen);
            }
            if self.fail || self.fail_after == Some(self.keys.len()) {
                Err(Error::State)
            } else {
                Ok(())
            }
        }
        fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<(), Error> {
            self.owner
                .get_mut(offset..offset + bytes.len())
                .ok_or(Error::Capacity)?
                .copy_from_slice(bytes);
            Ok(())
        }
        fn tracing(&self) -> bool {
            true
        }
        fn snapshot(&self, out: &mut [u8; WORD]) -> Result<(), Error> {
            *out = self.owner;
            Ok(())
        }
        fn trace(&mut self, source: &[u8; WORD], replacement: &[u8; WORD]) {
            self.trace = Some((
                source[..length(source).unwrap()].into(),
                replacement[2..2 + length(&replacement[2..]).unwrap()].into(),
            ));
        }
        fn finish(&mut self) -> Result<(), Error> {
            self.done = true;
            Ok(())
        }
    }
    #[test]
    fn plain_nonemoji_or_unmodified_input_leaves_owner_unchanged() {
        for word in ["text", "🙂", "🏽"] {
            let mut f = Fixture::new(word);
            let previous = f.owner;
            assert_eq!(lookup(&mut f), Ok(false));
            assert_eq!(f.owner, previous);
            assert!(f.keys.is_empty());
        }
    }
    #[test]
    fn modifier_names_follow_the_copied_base_and_preserve_tail() {
        let mut f = Fixture::new("👩🏽‍🔬 tail");
        assert_eq!(lookup(&mut f), Ok(true));
        assert_eq!(f.keys, ["👩‍🔬 ".as_bytes(), "🏽 ".as_bytes()]);
        assert_eq!(&f.owner[..13], b"\0 base tone \0");
        assert_eq!(f.owner[159], 0x55);
        assert!(f.done);
    }
    #[test]
    fn missing_modifier_base_refuses_but_tag_only_uses_stripped_base() {
        let mut f = Fixture::new("👩🏽");
        f.base = None;
        let previous = f.owner;
        assert_eq!(lookup(&mut f), Ok(false));
        assert_eq!(f.owner, previous);
        assert_eq!(f.effects, 1);
        let mut f = Fixture::new("🏴\u{e0067}\u{e007f}");
        f.base = None;
        assert_eq!(lookup(&mut f), Ok(true));
        assert_eq!(f.trace.unwrap().1, "🏴 ".as_bytes());
    }
    #[test]
    fn nested_replacement_and_source_effects_are_fresh_in_trace() {
        let mut f = Fixture::new("👩🏽");
        f.mutate = true;
        assert_eq!(lookup(&mut f), Ok(true));
        assert_eq!(f.owner[100], 0x79);
        let trace = f.trace.unwrap();
        assert_eq!(trace.0[0], b'!');
        assert_eq!(trace.1, b"Xase tone ");
    }
    #[test]
    fn modifier_limit_and_join_capacity_stop_with_existing_replacement() {
        let mut f = Fixture::new(&format!("👩{}", "🏽".repeat(12)));
        assert_eq!(lookup(&mut f), Ok(true));
        assert_eq!(f.keys.len(), 9);
        let mut f = Fixture::new("👩🏽🏾");
        f.base = Some(vec![b'b'; 156]);
        f.modifier = Some(b"m".into());
        assert_eq!(lookup(&mut f), Ok(true));
        assert_eq!(f.keys.len(), 2);
        assert_eq!(f.owner[158], 0);
        let mut f = Fixture::new("👩🏽");
        f.base = Some(vec![b'b'; 157]);
        let previous = f.owner;
        assert_eq!(lookup(&mut f), Ok(false));
        assert_eq!(f.owner, previous);
    }
    #[test]
    fn missing_modifier_names_do_not_rewrite_nested_effects() {
        let mut f = Fixture::new("👩🏽");
        f.modifier = None;
        f.mutate = true;
        assert_eq!(lookup(&mut f), Ok(true));
        assert_eq!(f.trace.unwrap().1, b"Xase ");
    }
    #[test]
    fn primitive_failure_retains_effect_without_publishing_or_replay() {
        let mut f = Fixture::new("👩🏽");
        f.fail = true;
        let previous = f.owner;
        assert_eq!(lookup(&mut f), Err(Error::State));
        assert_eq!(f.effects, 1);
        assert_eq!(f.owner, previous);
        assert!(!f.done);
    }
    #[test]
    fn bounded_missing_source_refuses() {
        let mut f = Fixture::new("👩🏽");
        f.source.pop();
        assert_eq!(lookup(&mut f), Err(Error::Source));
        assert!(f.keys.is_empty());
    }
    #[test]
    fn stripped_capacity_skips_whole_characters_but_admits_later_narrow_bytes() {
        let mut f = Fixture::new(&format!("👩{}🔬y🏽", "x".repeat(154)));
        assert_eq!(lookup(&mut f), Ok(true));
        assert_eq!(f.keys[0], format!("👩{}y ", "x".repeat(154)).as_bytes());
        assert_eq!(f.keys[0].len(), WORD);
    }
    #[test]
    fn later_failure_preserves_published_base_and_nested_owner_changes() {
        let mut f = Fixture::new("👩🏽");
        f.fail_after = Some(2);
        f.mutate = true;
        assert_eq!(lookup(&mut f), Err(Error::State));
        assert_eq!(f.effects, 2);
        assert_eq!(&f.owner[2..8], b"Xase \0");
        assert_eq!(f.owner[100], 0x79);
        assert!(!f.done);
    }
    #[test]
    fn nonterminated_lookup_text_refuses_without_replacement_writes() {
        let mut f = Fixture::new("👩🏽");
        f.base = Some(vec![b'x'; INPUT]);
        let previous = f.owner;
        assert_eq!(lookup(&mut f), Err(Error::Source));
        assert_eq!(f.owner, previous);
        assert_eq!(f.effects, 1);
    }
}
