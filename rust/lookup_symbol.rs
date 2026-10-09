//! Symbol dictionary lookup and translated-name control.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::lookup_list::{self, Error, ALLOW_TEXT, TEXT_MODE};
use crate::number_lookup::PHONEME_BYTES;

pub const SOURCE_BYTES: usize = 80;
pub const SYMBOL: u32 = 0x4000_0000;

/// Serialized projections; no translator or replacement-source borrow survives
/// nested translation. The child scopes/restores its rule source to owned text.
pub trait Host {
    type List: lookup_list::Host;
    fn list(&mut self) -> &mut Self::List;
    fn byte(&self, position: usize) -> Option<u8>;
    fn say_as(&self) -> i32;
    fn set_say_as(&mut self, value: i32);
    fn translate(
        &mut self,
        text: &mut [u8; SOURCE_BYTES],
        phonemes: &mut [u8; PHONEME_BYTES],
    ) -> i32;
}

/// Returned flags retain their signed C bit pattern. A zero result may carry
/// pronunciation: callers must not infer empty output from flags alone.
/// Publication is atomic on source/phoneme/capacity failure; already-executed
/// dictionary/translation effects remain live. Say-as restores on child return,
/// including invalid pronunciation and publication failure.
pub fn symbol(host: &mut impl Host, output: &mut [u8]) -> Result<i32, Error> {
    if output.is_empty() || output.len() > PHONEME_BYTES {
        return Err(Error::Capacity);
    }
    let mut phonemes = [0; PHONEME_BYTES];
    let mut flags = [0, SYMBOL];
    let found = lookup_list::dictionary_list(
        host.list(),
        ALLOW_TEXT,
        &mut flags,
        &mut phonemes[..output.len()],
    )?;
    let mut result = if found { flags[0] as i32 } else { 0 };
    if flags[0] & TEXT_MODE != 0 {
        let mut text = [0; SOURCE_BYTES];
        text[1] = b' ';
        text[2] = b' ';
        // Original strncpy0 copies at most 76 source bytes and zero-pads the
        // remaining 77-byte destination, including its forced final NUL.
        for (position, slot) in text[3..SOURCE_BYTES - 1].iter_mut().enumerate() {
            let byte = host.byte(position).ok_or(Error::Source)?;
            if byte == 0 {
                break;
            }
            *slot = byte;
        }
        let say_as = host.say_as();
        host.set_say_as(0);
        result = host.translate(&mut text, &mut phonemes);
        host.set_say_as(say_as);
    }
    let length = phonemes
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(Error::Phonemes)?;
    if length >= output.len() {
        return Err(Error::Capacity);
    }
    output[..=length].copy_from_slice(&phonemes[..=length]);
    Ok(result)
}

/// Rule-prefix attribute queries use ordinary lookup context and retain flags
/// even when list policy returns false for text replacement or a lookup miss.
pub fn flags(host: &mut impl lookup_list::Host) -> Result<[u32; 2], Error> {
    let mut flags = [0; 2];
    let mut phonemes = [0; 100];
    lookup_list::dictionary_list(host, 0, &mut flags, &mut phonemes)?;
    Ok(flags)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lookup_list::{Host as ListHost, WORD_BYTES};
    struct Fixture {
        source: Vec<u8>,
        name: Vec<u8>,
        direct: Vec<u8>,
        flags: u32,
        captured_flags: Vec<[u32; 2]>,
        say_as: i32,
        query_say_as: Option<i32>,
        calls: Vec<i32>,
        texts: Vec<[u8; SOURCE_BYTES]>,
        child_flags: i32,
        child: Vec<u8>,
        bad_source: bool,
    }
    impl Fixture {
        fn new() -> Self {
            Self {
                source: b"_name\0".to_vec(),
                name: b"name\0".to_vec(),
                direct: b"xy\0".to_vec(),
                flags: 0x8000_0000,
                captured_flags: vec![],
                say_as: 7,
                query_say_as: None,
                calls: vec![],
                texts: vec![],
                child_flags: 0,
                child: b"dna\0".to_vec(),
                bad_source: false,
            }
        }
    }
    impl ListHost for Fixture {
        fn byte(&self, position: usize) -> Option<u8> {
            self.source.get(position).copied()
        }
        fn lookup(
            &mut self,
            _: &[u8; WORD_BYTES],
            _: usize,
            flags: &mut [u32; 2],
            phonemes: &mut [u8; PHONEME_BYTES],
        ) -> Result<Option<usize>, Error> {
            self.captured_flags.push(*flags);
            *flags = [self.flags, 0];
            let bytes = if self.flags & TEXT_MODE != 0 {
                &self.name
            } else {
                &self.direct
            };
            phonemes[..bytes.len()].copy_from_slice(bytes);
            if let Some(value) = self.query_say_as {
                self.say_as = value;
            }
            Ok(Some(6))
        }
        fn repeat(&self, _: &mut [u8; 20]) -> i32 {
            0
        }
        fn set_repeat(&mut self, _: &[u8; 20], _: i32) {}
        fn text_mode(&self) -> bool {
            false
        }
        fn skip_words(&mut self, _: i32) {
            panic!("unexpected abbreviation")
        }
        fn accent(&mut self, _: u32, _: usize, _: &mut [u8; PHONEME_BYTES]) {
            panic!("unexpected accent")
        }
        fn replacement(&mut self, text: &[u8; WORD_BYTES]) {
            self.name = text[2..].to_vec();
        }
        fn trace_replacement(&mut self, _: usize) {}
    }
    impl Host for Fixture {
        type List = Self;
        fn list(&mut self) -> &mut Self {
            self
        }
        fn byte(&self, position: usize) -> Option<u8> {
            if self.bad_source {
                None
            } else {
                self.name.get(position).copied()
            }
        }
        fn say_as(&self) -> i32 {
            self.say_as
        }
        fn set_say_as(&mut self, value: i32) {
            self.calls.push(value);
            self.say_as = value;
        }
        fn translate(
            &mut self,
            text: &mut [u8; SOURCE_BYTES],
            out: &mut [u8; PHONEME_BYTES],
        ) -> i32 {
            assert_eq!(self.say_as, 0);
            self.texts.push(*text);
            self.say_as = 42;
            out[..self.child.len()].copy_from_slice(&self.child);
            self.child_flags
        }
    }
    #[test]
    fn direct_lookup_retains_signed_flags_and_exact_prefix_without_translation() {
        let mut host = Fixture::new();
        let mut out = [0x97; 4];
        assert_eq!(symbol(&mut host, &mut out), Ok(i32::MIN));
        assert_eq!(out, [b'x', b'y', 0, 0x97]);
        assert_eq!(host.captured_flags, [[0, SYMBOL]]);
        assert!(host.texts.is_empty());
        assert!(host.calls.is_empty());
        assert_eq!(host.say_as, 7);
    }
    #[test]
    fn zero_child_flags_keep_pronunciation_and_restore_fresh_say_as() {
        let mut host = Fixture::new();
        host.flags = TEXT_MODE;
        host.query_say_as = Some(9);
        let mut out = [0x97; 5];
        assert_eq!(symbol(&mut host, &mut out), Ok(0));
        assert_eq!(out, [b'd', b'n', b'a', 0, 0x97]);
        assert_eq!(host.calls, [0, 9]);
        assert_eq!(host.say_as, 9);
        assert_eq!(&host.texts[0][..9], b"\0  name \0");
        assert!(host.texts[0][9..].iter().all(|b| *b == 0));
    }
    #[test]
    fn replacement_source_truncation_matches_the_original_77_byte_destination() {
        let mut host = Fixture::new();
        host.flags = TEXT_MODE;
        host.name = vec![b'a'; 100];
        host.name.push(0);
        assert_eq!(symbol(&mut host, &mut [0; 4]), Ok(0));
        assert_eq!(&host.texts[0][..3], b"\0  ");
        assert!(host.texts[0][3..79].iter().all(|b| *b == b'a'));
        assert_eq!(host.texts[0][79], 0);
    }
    #[test]
    fn invalid_child_and_short_output_restore_say_as_and_preserve_output() {
        for child in [vec![0x82; PHONEME_BYTES], b"long\0".to_vec()] {
            let mut host = Fixture::new();
            host.flags = TEXT_MODE;
            host.child = child;
            let mut out = [0x97; 4];
            assert!(symbol(&mut host, &mut out).is_err());
            assert_eq!(out, [0x97; 4]);
            assert_eq!(host.calls, [0, 7]);
            assert_eq!(host.say_as, 7);
            assert_eq!(host.texts.len(), 1);
        }
    }
    #[test]
    fn direct_pronunciation_failure_does_not_replay_dictionary_or_change_say_as() {
        let mut host = Fixture::new();
        let mut out = [0x97; 2];
        assert_eq!(symbol(&mut host, &mut out), Err(Error::Capacity));
        assert_eq!(out, [0x97; 2]);
        assert_eq!(host.captured_flags.len(), 1);
        assert!(host.calls.is_empty());
    }
    #[test]
    fn invalid_replacement_source_preserves_output_before_say_as_changes() {
        let mut host = Fixture::new();
        host.flags = TEXT_MODE;
        host.bad_source = true;
        let mut out = [0x97; 4];
        assert_eq!(symbol(&mut host, &mut out), Err(Error::Source));
        assert_eq!(out, [0x97; 4]);
        assert!(host.calls.is_empty());
        assert!(host.texts.is_empty());
        assert_eq!(host.say_as, 7);
    }
    #[test]
    fn prefix_attribute_query_keeps_text_flags_without_symbol_or_translation_policy() {
        let mut host = Fixture::new();
        host.flags = TEXT_MODE | 0x8000_0000;
        assert_eq!(flags(&mut host), Ok([TEXT_MODE | 0x8000_0000, 0]));
        assert_eq!(host.captured_flags, [[0, 0]]);
        assert!(host.calls.is_empty());
        assert!(host.texts.is_empty());
        assert_eq!(host.name, b"name\0");
    }
    #[test]
    fn prefix_attribute_failure_retains_executed_query_without_translation_or_replay() {
        let mut host = Fixture::new();
        host.direct = vec![b'a'; 100];
        host.direct.push(0);
        assert_eq!(flags(&mut host), Err(Error::Capacity));
        assert_eq!(host.captured_flags.len(), 1);
        assert!(host.calls.is_empty());
        assert!(host.texts.is_empty());
    }
}
