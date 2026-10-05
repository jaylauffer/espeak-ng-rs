//! Bounded voice candidate ordering and property selection.
//!
//! Hosts prepare a roster during initialization/offload and retain its metadata.
//! A reusable workspace reserves all candidate storage once; selection has no
//! allocation, I/O, scheduling or process-global state.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    phoneme_data::InvalidPhonemeData as Error,
    voice_selection::{self, Selector, Voice},
};
use std::cmp::Ordering;

pub const MAX_VOICES: usize = 499;
const MAX_VARIANTS: usize = 12;

/// Roster values must remain stable during a selection. Accessors perform no I/O.
pub trait Roster {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn voice(&self, index: usize) -> Option<Voice<'_>>;
    fn previous_score(&self, _index: usize) -> i32 {
        0
    }
}
impl Roster for [Voice<'_>] {
    fn len(&self) -> usize {
        <[Voice<'_>]>::len(self)
    }
    fn voice(&self, index: usize) -> Option<Voice<'_>> {
        self.get(index).copied()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Properties<'a> {
    pub name: Option<&'a [u8]>,
    pub language: Option<&'a [u8]>,
    pub identifier: Option<&'a [u8]>,
    pub gender: u8,
    pub age: u8,
    pub variant: u8,
}
#[derive(Clone, Copy, Debug)]
pub struct Ranked {
    pub index: usize,
    pub score: i32,
    pub update_score: bool,
}
#[derive(Clone, Copy, Debug)]
struct Candidate {
    index: usize,
    variant: u8,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Selection {
    pub index: usize,
    pub suffix: [u8; 40],
    pub found: bool,
}

#[derive(Debug)]
pub struct Filter {
    pub language: [u8; 80],
    pub length: usize,
    pub parts: i32,
    pub all: bool,
}
impl Filter {
    pub fn new(
        language: Option<&[u8]>,
        include_mbrola: bool,
        directory: bool,
        separator: u8,
    ) -> Result<Self, Error> {
        let mut filter = Self {
            language: [0; 80],
            length: 0,
            parts: 0,
            all: language.is_none_or(|value| value.starts_with(b"all")),
        };
        let language = language.unwrap_or_default();
        if language.len() > 79 {
            return Err(Error("voice language selector exceeds 79 bytes"));
        }
        for (slot, byte) in filter.language.iter_mut().zip(language) {
            *slot = byte.to_ascii_lowercase();
        }
        filter.length = language.len();
        if !language.is_empty() {
            filter.parts = 1 + language.iter().filter(|byte| **byte == b'-').count() as i32;
        }
        if filter.parts == 1 && include_mbrola {
            if &filter.language[..filter.length] == b"mbrola" {
                filter.length = 2;
                filter.language[2] = 0;
            }
            if directory {
                if filter.length >= 79 {
                    return Err(Error("voice directory selector exceeds bound"));
                }
                filter.language[filter.length] = separator;
                filter.length += 1;
                filter.language[filter.length] = 0;
                filter.parts = -1;
            }
        }
        Ok(filter)
    }
    pub fn bytes(&self) -> &[u8] {
        &self.language[..self.length]
    }
}

pub fn name_order(a: Voice<'_>, b: Voice<'_>) -> Ordering {
    fn primary(voice: Voice<'_>) -> &[u8] {
        let text = voice.languages.get(1..).unwrap_or_default();
        &text[..text
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(text.len())]
    }
    primary(a)
        .cmp(primary(b))
        .then_with(|| {
            (a.languages.first().copied().unwrap_or(0) as std::ffi::c_char)
                .cmp(&(b.languages.first().copied().unwrap_or(0) as std::ffi::c_char))
        })
        .then_with(|| a.name.cmp(b.name))
}

/// Unfiltered public catalogues omit zero-priority, variant and MBROLA entries.
/// A property query may still rank those records explicitly.
pub fn visible(voice: Voice<'_>, separator: u8) -> bool {
    if voice
        .languages
        .first()
        .is_none_or(|priority| *priority == 0)
    {
        return false;
    }
    let primary = voice.languages.get(1..).unwrap_or_default();
    let length = primary
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(primary.len());
    &primary[..length] != b"variant" && !voice.identifier.starts_with(&[b'm', b'b', separator])
}

pub struct Workspace {
    capacity: usize,
    ranked: Vec<Ranked>,
    candidates: Vec<Candidate>,
}
impl Workspace {
    pub fn new(capacity: usize) -> Result<Self, Error> {
        if !(1..=MAX_VOICES).contains(&capacity) {
            return Err(Error("voice capacity must be 1..=499"));
        }
        Ok(Self {
            capacity,
            ranked: Vec::with_capacity(capacity),
            candidates: Vec::with_capacity(capacity + MAX_VARIANTS),
        })
    }
    pub fn ranked(&self) -> &[Ranked] {
        &self.ranked
    }
    pub fn rank<R: Roster + ?Sized>(
        &mut self,
        roster: &R,
        spec: Selector<'_>,
        filter: &Filter,
        include_mbrola: bool,
    ) -> Result<&[Ranked], Error> {
        self.ranked.clear();
        self.candidates.clear();
        if roster.len() > self.capacity {
            return Err(Error("voice roster exceeds workspace admission"));
        }
        for index in 0..roster.len() {
            let voice = roster
                .voice(index)
                .ok_or(Error("missing voice roster entry"))?;
            if !include_mbrola && voice.identifier.starts_with(b"mb/") {
                continue;
            }
            let score = if filter.all {
                roster.previous_score(index)
            } else {
                voice_selection::score(spec, filter.bytes(), filter.parts, voice)?
            };
            if filter.all || score > 0 {
                self.ranked.push(Ranked {
                    index,
                    score,
                    update_score: !filter.all,
                });
            }
        }
        self.ranked.sort_unstable_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| {
                    roster
                        .voice(a.index)
                        .map(|voice| voice.name)
                        .unwrap_or_default()
                        .cmp(
                            roster
                                .voice(b.index)
                                .map(|voice| voice.name)
                                .unwrap_or_default(),
                        )
                })
                .then_with(|| a.index.cmp(&b.index))
        });
        Ok(&self.ranked)
    }
    fn by_name<R: Roster + ?Sized>(roster: &R, name: &[u8], separator: u8) -> Option<usize> {
        voice_selection::by_name(
            (0..roster.len()).filter_map(|index| roster.voice(index)),
            name,
            separator,
        )
    }
    /// Directory discovery is an owner input. `directory` is true only if the
    /// normalized one-part MBROLA selector names an existing voice directory.
    pub fn select<R: Roster + ?Sized>(
        &mut self,
        roster: &R,
        properties: Properties<'_>,
        directory: bool,
        separator: u8,
        default_voice: &[u8],
    ) -> Result<Option<Selection>, Error> {
        self.select_with_directory(roster, properties, separator, default_voice, |_| directory)
    }
    pub fn select_with_directory<R: Roster + ?Sized>(
        &mut self,
        roster: &R,
        properties: Properties<'_>,
        separator: u8,
        default_voice: &[u8],
        mut directory: impl FnMut(&Filter) -> bool,
    ) -> Result<Option<Selection>, Error> {
        self.ranked.clear();
        self.candidates.clear();
        if roster.len() > self.capacity {
            return Err(Error("voice roster exceeds workspace admission"));
        }
        let mut derived_language = [0; 80];
        if (0..roster.len()).any(|index| roster.voice(index).is_none()) {
            return Err(Error("missing voice roster entry"));
        }
        let mut requested_name = properties.name;
        let mut requested_language = properties.language;
        if properties
            .language
            .is_none_or(|language| language.is_empty())
        {
            let requested = properties
                .name
                .or(properties.identifier)
                .unwrap_or(default_voice);
            requested_name = Some(requested);
            let requested = &requested[..requested.len().min(59)];
            let variant = voice_selection::variant(Some(requested), 0, false, separator)?;
            if let Some(index) = Self::by_name(roster, &requested[..variant.base_length], separator)
            {
                let voice = roster.voice(index).ok_or(Error("missing named voice"))?;
                if properties.gender == 0 && properties.age == 0 && properties.variant == 0 {
                    return Ok(Some(Selection {
                        index,
                        suffix: *variant.terminated_suffix(),
                        found: true,
                    }));
                }
                let primary = voice.languages.get(1..).unwrap_or_default();
                let count = primary
                    .iter()
                    .position(|byte| *byte == 0)
                    .unwrap_or(primary.len());
                if count > 79 {
                    return Err(Error("voice primary language exceeds bound"));
                }
                derived_language[..count].copy_from_slice(&primary[..count]);
                requested_language = Some(&derived_language[..count]);
            }
        }
        let include_mbrola = properties
            .identifier
            .is_some_and(|id| id.starts_with(b"mb/"));
        let mut filter = Filter::new(requested_language, include_mbrola, false, separator)?;
        if include_mbrola && filter.parts == 1 && directory(&filter) {
            filter = Filter::new(requested_language, include_mbrola, true, separator)?;
        }
        self.rank(
            roster,
            Selector {
                name: requested_name,
                gender: properties.gender,
                age: properties.age,
            },
            &filter,
            include_mbrola,
        )?;
        let mut found = true;
        if self.ranked.is_empty() {
            found = false;
            if let Some(index) = Self::by_name(roster, default_voice, separator) {
                self.ranked.push(Ranked {
                    index,
                    score: 0,
                    update_score: false,
                });
            }
        }
        let gender = if properties.gender == 2 || (properties.age > 0 && properties.age < 13) {
            2
        } else if properties.gender == 1 {
            1
        } else {
            0
        };
        let aged = properties.age >= 60;
        let variants: &[u8] = match gender {
            1 => &[1, 2, 3, 4, 5, 6, 0],
            2 => &[11, 12, 13, 14, 0],
            _ => &[1, 2, 12, 3, 13, 4, 14, 5, 11, 0],
        };
        let mut cursor = usize::from(!aged);
        let mut added = 0;
        let mut last = None;
        for rank in &self.ranked {
            let voice = roster
                .voice(rank.index)
                .ok_or(Error("missing ranked voice"))?;
            last = Some(rank.index);
            let skip = (gender != 0 && voice.gender != gender)
                || (self.candidates.is_empty() && aged && voice.age < 60);
            if !skip {
                self.candidates.push(Candidate {
                    index: rank.index,
                    variant: 0,
                });
            }
            for _ in 0..voice.variants {
                if added >= MAX_VARIANTS {
                    break;
                }
                if variants[cursor] == 0 {
                    cursor = 0;
                }
                self.candidates.push(Candidate {
                    index: rank.index,
                    variant: variants[cursor],
                });
                cursor += 1;
                added += 1;
            }
        }
        if let Some(index) = last {
            while added < MAX_VARIANTS && variants.get(cursor).is_some_and(|number| *number != 0) {
                self.candidates.push(Candidate {
                    index,
                    variant: variants[cursor],
                });
                cursor += 1;
                added += 1;
            }
        }
        if self.candidates.is_empty() {
            return Ok(None);
        }
        let candidate = self.candidates[usize::from(properties.variant) % self.candidates.len()];
        let variant =
            voice_selection::variant(None, i32::from(candidate.variant), false, separator)?;
        Ok(Some(Selection {
            index: candidate.index,
            suffix: *variant.terminated_suffix(),
            found,
        }))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn public_catalogue_visibility_keeps_variants_and_mbrola_for_explicit_queries() {
        let mut metadata = crate::voice_selection::Metadata::parse(b"language en\n").unwrap();
        assert!(super::visible(metadata.view(b"en").unwrap(), b'/'));
        assert!(!super::visible(metadata.view(b"mb/en").unwrap(), b'/'));
        assert!(!super::visible(metadata.view(b"mb\\en").unwrap(), b'\\'));
        assert!(super::visible(metadata.view(b"mb").unwrap(), b'/'));
        metadata.languages[0] = 0;
        assert!(!super::visible(metadata.view(b"en").unwrap(), b'/'));
        metadata = crate::voice_selection::Metadata::parse(b"language variant\n").unwrap();
        assert!(!super::visible(metadata.view(b"!v/m1").unwrap(), b'/'));
    }
    use super::*;
    #[test]
    fn selection_reuses_admitted_storage_and_retains_variant_order() {
        let voices = [
            Voice {
                name: b"English",
                identifier: b"en",
                languages: b"\x05en\0\0",
                gender: 1,
                age: 0,
                variants: 4,
            },
            Voice {
                name: b"German",
                identifier: b"de",
                languages: b"\x05de\0\0",
                gender: 1,
                age: 0,
                variants: 4,
            },
        ];
        let mut workspace = Workspace::new(2).unwrap();
        let (ranks, candidates) = (workspace.ranked.as_ptr(), workspace.candidates.as_ptr());
        for (variant, suffix) in [b"".as_slice(), b"m2", b"f2", b"m3", b"f3"]
            .iter()
            .enumerate()
        {
            let chosen = workspace
                .select(
                    &voices[..],
                    Properties {
                        language: Some(b"en"),
                        variant: variant as u8,
                        ..Properties::default()
                    },
                    false,
                    b'/',
                    b"en",
                )
                .unwrap()
                .unwrap();
            assert_eq!(chosen.index, 0);
            assert!(chosen.found);
            assert_eq!(&chosen.suffix[..suffix.len()], *suffix);
        }
        let named = workspace
            .select(
                &voices[..],
                Properties {
                    name: Some(b"English+f3"),
                    ..Properties::default()
                },
                false,
                b'/',
                b"en",
            )
            .unwrap()
            .unwrap();
        assert_eq!(&named.suffix[..3], b"f3\0");
        let absent = workspace
            .select(
                &voices[..],
                Properties {
                    language: Some(b"missing"),
                    ..Properties::default()
                },
                false,
                b'/',
                b"en",
            )
            .unwrap()
            .unwrap();
        assert!(!absent.found);
        assert_eq!(absent.index, 0);
        assert_eq!(workspace.ranked.as_ptr(), ranks);
        assert_eq!(workspace.candidates.as_ptr(), candidates);
        assert!(Workspace::new(1)
            .unwrap()
            .select(&voices[..], Properties::default(), false, b'/', b"en")
            .is_err());
    }
    #[test]
    fn filter_preserves_all_case_and_directory_rules() {
        assert!(Filter::new(None, false, false, b'/').unwrap().all);
        assert!(
            Filter::new(Some(b"all-more"), false, false, b'/')
                .unwrap()
                .all
        );
        assert!(!Filter::new(Some(b"ALL"), false, false, b'/').unwrap().all);
        let filter = Filter::new(Some(b"MBROLA"), true, true, b'/').unwrap();
        assert_eq!(filter.bytes(), b"mb/");
        assert_eq!(filter.parts, -1);
        assert!(Filter::new(Some(&[b'a'; 80]), false, false, b'/').is_err());
    }
}
