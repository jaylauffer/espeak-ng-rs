//! Complete public voice-selection, variant loading and voice-stack ordering.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::voice_selection;
pub const NOT_FOUND: i32 = 0x100006ff;
#[derive(Clone)]
pub struct Name(pub [u8; 4096]);
impl Name {
    pub fn bytes(&self) -> &[u8] {
        &self.0[..self.0.iter().position(|b| *b == 0).unwrap_or(4096)]
    }
}
pub trait Host {
    type Loaded: Copy;
    type Selector;
    fn classes(&mut self) -> [u8; 256];
    fn load(&mut self, name: &[u8], control: i32) -> Option<Self::Loaded>;
    fn empty_catalog(&mut self) -> bool;
    fn catalog(&mut self);
    fn by_name(&mut self, name: &[u8]) -> Option<Name>;
    fn select(&mut self, selector: &Self::Selector) -> (bool, Option<Name>);
    fn change(&mut self);
    fn language(&mut self) -> Option<[u8; 20]>;
    fn stack_name(&mut self, name: &[u8], language: &[u8; 20], variant: &[u8; 40]);
    fn stack_properties(&mut self, selector: &Self::Selector);
}
fn request(name: &[u8], number: i32, separator: u8) -> ([u8; 60], [u8; 40]) {
    let mut base = [0; 60];
    let length = name
        .iter()
        .position(|b| *b == 0)
        .unwrap_or(name.len())
        .min(59);
    base[..length].copy_from_slice(&name[..length]);
    let mut suffix = [0; 40];
    // The existing native extractor leaves the original truncated base and an
    // empty suffix on variant refusal; it does not reject the whole API call.
    if let Ok(v) = voice_selection::variant(Some(&base[..length]), number, true, separator) {
        suffix = *v.terminated_suffix();
        base[v.base_length] = 0;
    }
    (base, suffix)
}
fn terminated(bytes: &[u8]) -> &[u8] {
    &bytes[..=bytes.iter().position(|b| *b == 0).unwrap()]
}
/// A variant's failed tone load returns failure after its accepted base load.
/// The caller of property selection deliberately ignores that result.
pub fn variant<H: Host>(
    host: &mut H,
    name: &[u8],
    number: i32,
    separator: u8,
) -> Option<H::Loaded> {
    let (base, suffix) = request(name, number, separator);
    let loaded = host.load(terminated(&base), 0)?;
    if suffix[0] != 0 {
        host.load(terminated(&suffix), 2)
    } else {
        Some(loaded)
    }
}
/// Exact file/name admission and catalogue fallback. Missing optional variants
/// do not undo an accepted base; change/language/stack read the fresh engine.
pub fn named<H: Host>(host: &mut H, name: &[u8], file: bool, separator: u8) -> i32 {
    let (mut base, suffix) = request(name, 0, separator);
    let lower = host.classes();
    if lower[0] != 0 {
        return NOT_FOUND;
    }
    for byte in &mut base {
        *byte = lower[usize::from(*byte)];
        if *byte == 0 {
            break;
        }
    }
    let mut loaded = host
        .load(terminated(&base), if file { 0x10 } else { 1 })
        .is_some();
    if !loaded && !file {
        if host.empty_catalog() {
            host.catalog();
        }
        if let Some(selected) = host.by_name(terminated(&base)) {
            let length = selected.bytes().len();
            if length < 4096 {
                loaded = host.load(&selected.0[..=length], 0).is_some();
            }
        }
    }
    if !loaded {
        return NOT_FOUND;
    }
    if suffix[0] != 0 {
        let _ = host.load(terminated(&suffix), 2);
    }
    host.change();
    let Some(language) = host.language() else {
        return NOT_FOUND;
    };
    host.stack_name(name, &language, &suffix);
    0
}
/// Preserve found-vs-loaded distinction: found properties change/stack even if
/// their subsequent base or tone load fails, as the original public API did.
pub fn properties<H: Host>(host: &mut H, selector: &H::Selector, separator: u8) -> i32 {
    let (found, selected) = host.select(selector);
    if !found {
        return NOT_FOUND;
    }
    let Some(selected) = selected else {
        return NOT_FOUND;
    };
    let _ = variant(host, selected.bytes(), 0, separator);
    host.change();
    host.stack_properties(selector);
    0
}
#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Fixture {
        calls: Vec<(Vec<u8>, i32)>,
        changed: usize,
        stacked: usize,
    }
    impl Host for Fixture {
        type Loaded = usize;
        type Selector = ();
        fn classes(&mut self) -> [u8; 256] {
            std::array::from_fn(|i| (i as u8).to_ascii_lowercase())
        }
        fn load(&mut self, name: &[u8], control: i32) -> Option<usize> {
            self.calls.push((name.to_vec(), control));
            (control == 1).then_some(7)
        }
        fn empty_catalog(&mut self) -> bool {
            false
        }
        fn catalog(&mut self) {
            unreachable!()
        }
        fn by_name(&mut self, _: &[u8]) -> Option<Name> {
            None
        }
        fn select(&mut self, _: &()) -> (bool, Option<Name>) {
            let mut bytes = [0; 4096];
            bytes[..5].copy_from_slice(b"en+1\0");
            (true, Some(Name(bytes)))
        }
        fn change(&mut self) {
            self.changed += 1;
        }
        fn language(&mut self) -> Option<[u8; 20]> {
            Some([0; 20])
        }
        fn stack_name(&mut self, _: &[u8], _: &[u8; 20], _: &[u8; 40]) {
            self.stacked += 1;
        }
        fn stack_properties(&mut self, _: &()) {
            self.stacked += 1;
        }
    }
    #[test]
    fn optional_named_variant_failure_keeps_base_and_updates_stack() {
        let mut h = Fixture::default();
        assert_eq!(named(&mut h, b"EN+1\0", false, b'/'), 0);
        assert_eq!(h.calls, [(b"en\0".to_vec(), 1), (b"!v/m1\0".to_vec(), 2)]);
        assert_eq!((h.changed, h.stacked), (1, 1));
    }
    #[test]
    fn found_properties_publish_despite_failed_load_and_numeric_refusal_keeps_base() {
        let mut h = Fixture::default();
        assert_eq!(properties(&mut h, &(), b'/'), 0);
        assert_eq!((h.changed, h.stacked), (1, 1));
        assert_eq!(h.calls, [(b"en\0".to_vec(), 0)]);
        let mut h = Fixture::default();
        assert_eq!(named(&mut h, b"EN+999999999999", false, b'/'), 0);
        assert_eq!(h.calls, [(b"en+999999999999\0".to_vec(), 1)]);
    }
}
