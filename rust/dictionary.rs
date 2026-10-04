//! Bounds-checked views of eSpeak NG's compiled pronunciation dictionaries.
//! Contextual matching is in `lookup`; alphabet compression is in `word_key`.
// Copyright (C) 2005-2014 Jonathan Duddington; Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later

const BUCKETS: usize = 1024;

/// The exact byte hash used by `HashDictionary`, including NUL termination.
pub fn hash(word: &[u8]) -> usize {
    let mut hash = 0_u32;
    let mut chars = 0_u32;
    for &byte in word.iter().take_while(|&&byte| byte != 0) {
        hash = hash * 8 + u32::from(byte);
        hash = (hash & 0x3ff) ^ (hash >> 8);
        chars = chars.wrapping_add(1);
    }
    (hash.wrapping_add(chars) & 0x3ff) as usize
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct InvalidDictionary(pub &'static str);
impl std::fmt::Display for InvalidDictionary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for InvalidDictionary {}

/// A compiled dictionary borrowing its bytes. Parsing validates every record
/// and stores 1,024 bucket offsets once, rather than rescanning on each lookup.
pub struct Dictionary<'a> {
    bytes: &'a [u8],
    buckets: [usize; BUCKETS],
    rules_offset: usize,
}

impl<'a> Dictionary<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, InvalidDictionary> {
        let header = bytes
            .get(..8)
            .ok_or(InvalidDictionary("truncated dictionary header"))?;
        let bucket_count = u32::from_le_bytes(header[..4].try_into().expect("four bytes"));
        let rules_offset = u32::from_le_bytes(header[4..].try_into().expect("four bytes")) as usize;
        if bucket_count != BUCKETS as u32 {
            return Err(InvalidDictionary("dictionary must have 1024 hash buckets"));
        }
        if rules_offset < 8 + BUCKETS || rules_offset >= bytes.len() || rules_offset > 0x0800_0000 {
            return Err(InvalidDictionary(
                "dictionary rules offset is outside the data",
            ));
        }
        let mut buckets = [0; BUCKETS];
        let mut cursor = 8;
        for bucket in &mut buckets {
            *bucket = cursor;
            loop {
                let length = *bytes
                    .get(cursor)
                    .filter(|_| cursor < rules_offset)
                    .ok_or(InvalidDictionary("missing hash-bucket terminator"))?
                    as usize;
                if length == 0 {
                    cursor += 1;
                    break;
                }
                let end = cursor
                    .checked_add(length)
                    .filter(|end| *end <= rules_offset)
                    .ok_or(InvalidDictionary("dictionary record crosses into rules"))?;
                Entry::parse(&bytes[cursor..end])?;
                cursor = end;
            }
        }
        if cursor != rules_offset {
            return Err(InvalidDictionary("unused bytes between buckets and rules"));
        }
        Ok(Self {
            bytes,
            buckets,
            rules_offset,
        })
    }

    pub fn rules(&self) -> &'a [u8] {
        &self.bytes[self.rules_offset..]
    }

    pub fn rules_offset(&self) -> usize {
        self.rules_offset
    }
    pub fn bucket_offsets(&self) -> &[usize; BUCKETS] {
        &self.buckets
    }

    pub fn rule_index(&self) -> Result<crate::rules::RuleIndex, InvalidDictionary> {
        crate::rules::RuleIndex::parse(self.bytes, self.rules_offset)
    }

    /// Returns records in their original precedence order, without filtering.
    pub fn bucket(&self, word: &[u8]) -> Entries<'a> {
        Entries {
            remaining: &self.bytes[self.buckets[hash(word)]..self.rules_offset],
        }
    }
    /// Matches a prepared key without allocating. Compress with `Alphabet`
    /// when required; retain its unchanged hash tail and returned descriptor.
    pub fn lookup(
        &self,
        key: &[u8],
        descriptor: usize,
        next_words: &[u8],
        context: &crate::lookup::Context,
        words: Option<&[crate::lookup::WordInfo]>,
    ) -> Result<crate::lookup::Outcome<'a>, InvalidDictionary> {
        crate::lookup::lookup_bucket(
            &self.bytes[self.buckets[hash(key)]..self.rules_offset],
            key,
            descriptor,
            next_words,
            context,
            words,
        )
    }
}

/// Resident dictionary with indices built once at initialization/on a worker.
/// Owns the original byte allocation; lookup does not rebuild indices or allocate.
pub struct OwnedDictionary {
    bytes: Vec<u8>,
    buckets: [usize; BUCKETS],
    rules_offset: usize,
    rules: crate::rules::RuleIndex,
}
impl OwnedDictionary {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, InvalidDictionary> {
        let view = Dictionary::parse(&bytes)?;
        let buckets = *view.bucket_offsets();
        let rules_offset = view.rules_offset();
        let rules = view.rule_index()?;
        Ok(Self {
            bytes,
            buckets,
            rules_offset,
            rules,
        })
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn rule_index(&self) -> &crate::rules::RuleIndex {
        &self.rules
    }
    pub fn bucket(&self, word: &[u8]) -> Entries<'_> {
        Entries {
            remaining: &self.bytes[self.buckets[hash(word)]..self.rules_offset],
        }
    }
    /// Matches a prepared key using cached indices and immutable context.
    pub fn lookup(
        &self,
        key: &[u8],
        descriptor: usize,
        next_words: &[u8],
        context: &crate::lookup::Context,
        words: Option<&[crate::lookup::WordInfo]>,
    ) -> Result<crate::lookup::Outcome<'_>, InvalidDictionary> {
        crate::lookup::lookup_bucket(
            &self.bytes[self.buckets[hash(key)]..self.rules_offset],
            key,
            descriptor,
            next_words,
            context,
            words,
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Entry<'a> {
    pub word: &'a [u8],
    pub compressed: bool,
    pub phonemes: Option<&'a [u8]>,
    /// Raw contextual flags and any multiword suffix, interpreted by `lookup`.
    pub flags: &'a [u8],
}
impl<'a> Entry<'a> {
    pub(crate) fn parse(bytes: &'a [u8]) -> Result<Self, InvalidDictionary> {
        let descriptor = *bytes.get(1).ok_or(InvalidDictionary(
            "dictionary record is shorter than its header",
        ))?;
        let end_word = 2 + usize::from(descriptor & 0x3f);
        let word = bytes
            .get(2..end_word)
            .ok_or(InvalidDictionary("dictionary word is truncated"))?;
        let tail = &bytes[end_word..];
        let (phonemes, flags) = if descriptor & 0x80 != 0 {
            (None, tail)
        } else {
            let end = tail
                .iter()
                .position(|c| *c == 0)
                .ok_or(InvalidDictionary("dictionary phonemes lack a terminator"))?;
            (Some(&tail[..end]), &tail[end + 1..])
        };
        Ok(Self {
            word,
            compressed: descriptor & 0x40 != 0,
            phonemes,
            flags,
        })
    }
}

pub struct Entries<'a> {
    remaining: &'a [u8],
}
impl<'a> Iterator for Entries<'a> {
    type Item = Entry<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let length = usize::from(*self.remaining.first()?);
        if length == 0 {
            return None;
        }
        let (record, rest) = self.remaining.split_at(length); // validated by parse
        self.remaining = rest;
        Some(Entry::parse(record).expect("dictionary records already validated"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(record: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0; 8];
        bytes[..4].copy_from_slice(&(BUCKETS as u32).to_le_bytes());
        for bucket in 0..BUCKETS {
            if bucket == hash(b"cat") {
                bytes.extend_from_slice(record);
            }
            bytes.push(0);
        }
        let offset = bytes.len() as u32;
        bytes[4..8].copy_from_slice(&offset.to_le_bytes());
        bytes.extend_from_slice(&[6, 7, 0]);
        bytes
    }
    #[test]
    fn parses_word_phonemes_and_raw_flags_in_order() {
        let bytes = fixture(&[9, 3, b'c', b'a', b't', 42, 43, 0, 10]);
        let dict = Dictionary::parse(&bytes).unwrap();
        let entries: Vec<_> = dict.bucket(b"cat").collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].word, b"cat");
        assert_eq!(entries[0].phonemes, Some([42, 43].as_slice()));
        assert_eq!(entries[0].flags, [10]);
        assert_eq!(dict.rules(), [6, 7, 0]);
        assert_eq!(hash(b"cat\0ignored"), hash(b"cat"));
    }
    #[test]
    fn rejects_truncation_missing_terminators_and_bad_offsets() {
        let valid = fixture(&[9, 3, b'c', b'a', b't', 42, 43, 0, 10]);
        for length in 0..valid.len() - 3 {
            assert!(Dictionary::parse(&valid[..length]).is_err());
        }
        assert!(Dictionary::parse(&fixture(&[2, 3])).is_err());
        assert!(Dictionary::parse(&fixture(&[5, 3, b'c', b'a', b't'])).is_err());
        let mut bytes = valid;
        bytes[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Dictionary::parse(&bytes).is_err());
    }
}
