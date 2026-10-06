//! Character/special dictionary lookup with explicit copied host effects.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn.
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Bounds,
    Backend,
}
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Data {
    pub word: [u8; 160],
    pub phonemes: [u8; 60],
    pub flags: [u32; 2],
    pub start: u32,
}
impl Data {
    pub fn empty() -> Self {
        Self {
            word: [0; 160],
            phonemes: [0; 60],
            flags: [0; 2],
            start: 0,
        }
    }
    pub fn valid(&self) -> bool {
        self.start < 160
            && self.word[self.start as usize..].contains(&0)
            && self.phonemes.contains(&0)
    }
}
pub trait Host {
    fn dictionary(&mut self, data: &mut Data, secondary: bool) -> Result<bool, Error>;
    fn rules(&mut self, data: &mut Data) -> Result<(), Error>;
    fn fallback(&mut self) -> Result<(), Error>;
    fn format(&mut self, data: &mut Data, secondary: bool) -> Result<[u8; 74], Error>;
    fn restore_table(&mut self) -> Result<(), Error>;
    fn language(&self) -> u32;
}
pub struct Text {
    pub bytes: [u8; 74],
    pub length: usize,
}
fn text(bytes: [u8; 74]) -> Result<Text, Error> {
    let length = bytes
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(Error::Bounds)?;
    Ok(Text { bytes, length })
}
fn dictionary(host: &mut impl Host, data: &mut Data, secondary: bool) -> Result<bool, Error> {
    let found = host.dictionary(data, secondary)?;
    if !data.valid() {
        return Err(Error::Bounds);
    }
    Ok(found)
}
/// Preserve prefixed/unprefixed/rules/default-voice order and table restoration.
/// Buffers belong to this call; host methods copy effects and retain separate
/// engine resources. They cannot invalidate/reenter these local buffers. Native
/// wrapper text is copied, with no borrowed dictionary/catalogue pointer result.
pub fn character(code: i32, only: bool, host: &mut impl Host) -> Result<Text, Error> {
    let mut data = Data::empty();
    data.word[1] = b'_';
    let (bytes, length) = crate::suffix::encode(code as u32);
    data.word[2..2 + length].copy_from_slice(&bytes[..length]);
    if only {
        data.start = 2;
        dictionary(host, &mut data, false)?;
    } else {
        data.start = 1;
        if !dictionary(host, &mut data, false)? {
            data.start = 2;
            if !dictionary(host, &mut data, false)? {
                data.word[1] = b' ';
                host.rules(&mut data)?;
                if !data.valid() {
                    return Err(Error::Bounds);
                }
            }
        }
        if (data.phonemes[0] == 0 || data.phonemes[0] == 21) && host.language() != 0x656e {
            let result = (|| {
                host.fallback()?;
                data.start = 1;
                data.word[1] = b'_';
                if !dictionary(host, &mut data, true)? {
                    data.start = 2;
                    dictionary(host, &mut data, true)?;
                }
                if data.phonemes[0] != 0 {
                    text(host.format(&mut data, true)?)
                } else {
                    let mut bytes = [0; 74];
                    let placeholder = b"[\x02(X1)(X1)(X1)]]";
                    bytes[..placeholder.len()].copy_from_slice(placeholder);
                    text(bytes)
                }
            })();
            host.restore_table()?;
            return result;
        }
    }
    if data.phonemes[0] != 0 {
        text(host.format(&mut data, false)?)
    } else {
        let mut bytes = [0; 74];
        if !only {
            let placeholder = b"[\x02(X1)(X1)(X1)]]";
            bytes[..placeholder.len()].copy_from_slice(placeholder);
        }
        text(bytes)
    }
}
/// Special names are initialized terminated words with up to159 nonzero bytes.
/// The legacy special lookup's55-byte phoneme limit is admitted before format.
/// Missing lookup returns None and does not publish output.
pub fn special(word: &[u8], host: &mut impl Host) -> Result<Option<Text>, Error> {
    if word.is_empty() || word.len() >= 160 || word.contains(&0) {
        return Err(Error::Bounds);
    }
    let mut data = Data::empty();
    data.word[..word.len()].copy_from_slice(word);
    if !dictionary(host, &mut data, false)? {
        return Ok(None);
    }
    if data
        .phonemes
        .iter()
        .position(|code| *code == 0)
        .ok_or(Error::Bounds)?
        >= 55
    {
        return Err(Error::Bounds);
    }
    text(host.format(&mut data, false)?).map(Some)
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Backend {
        trace: Vec<&'static str>,
        dictionary: Vec<(bool, u8)>,
        language: u32,
        rules: u8,
        fail_format: bool,
    }
    impl Host for Backend {
        fn dictionary(&mut self, data: &mut Data, secondary: bool) -> Result<bool, Error> {
            self.trace.push(match (secondary, data.start) {
                (false, 0) => "special",
                (false, 1) => "prefixed",
                (false, _) => "bare",
                (true, 1) => "fallback-prefixed",
                (true, _) => "fallback-bare",
            });
            let (found, code) = self.dictionary.remove(0);
            data.phonemes[0] = code;
            data.phonemes[1] = 0;
            Ok(found)
        }
        fn rules(&mut self, data: &mut Data) -> Result<(), Error> {
            self.trace.push("rules");
            assert_eq!(data.word[1], b' ');
            data.phonemes[0] = self.rules;
            Ok(())
        }
        fn fallback(&mut self) -> Result<(), Error> {
            self.trace.push("load");
            Ok(())
        }
        fn format(&mut self, _: &mut Data, secondary: bool) -> Result<[u8; 74], Error> {
            self.trace.push(if secondary {
                "format-fallback"
            } else {
                "format"
            });
            if self.fail_format {
                return Err(Error::Backend);
            }
            let mut bytes = [0; 74];
            bytes[..4].copy_from_slice(b"name");
            Ok(bytes)
        }
        fn restore_table(&mut self) -> Result<(), Error> {
            self.trace.push("restore");
            Ok(())
        }
        fn language(&self) -> u32 {
            self.language
        }
    }
    #[test]
    fn fallback_orders_dictionary_rules_format_and_table_restoration() {
        let mut host = Backend {
            trace: Vec::new(),
            dictionary: vec![(false, 0), (false, 0), (false, 0), (true, 12)],
            language: 0x6672,
            rules: 21,
            fail_format: false,
        };
        let result = character(0x2026, false, &mut host).unwrap();
        assert_eq!(&result.bytes[..result.length], b"name");
        assert_eq!(
            host.trace,
            [
                "prefixed",
                "bare",
                "rules",
                "load",
                "fallback-prefixed",
                "fallback-bare",
                "format-fallback",
                "restore"
            ]
        );
        host.trace.clear();
        host.dictionary = vec![(true, 12)];
        host.fail_format = true;
        assert!(matches!(
            character(65, true, &mut host),
            Err(Error::Backend)
        ));
        assert_eq!(host.trace, ["bare", "format"]);
        host.trace.clear();
        host.dictionary = vec![(false, 0), (false, 0), (true, 12)];
        assert!(matches!(
            character(65, false, &mut host),
            Err(Error::Backend)
        ));
        assert_eq!(host.trace.last(), Some(&"restore"));
    }
    #[test]
    fn missing_only_name_and_special_do_not_invent_pronunciation() {
        let mut host = Backend {
            trace: Vec::new(),
            dictionary: vec![(false, 0), (false, 0)],
            language: 0x656e,
            rules: 0,
            fail_format: false,
        };
        assert_eq!(character(65, true, &mut host).unwrap().length, 0);
        assert!(special(b"_cap", &mut host).unwrap().is_none());
        assert_eq!(host.trace, ["bare", "special"]);
        assert!(matches!(special(b"", &mut host), Err(Error::Bounds)));
    }
}
