//! Active voice request paths, fallback controls and current identifiers.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::phoneme_data::InvalidPhonemeData as Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Request {
    pub path: [u8; 4096],
    pub name: [u8; 40],
    pub control: u32,
}
fn string<const N: usize>(output: &mut [u8; N], bytes: &[u8]) {
    output.fill(0);
    let count = bytes.len().min(N - 1);
    output[..count].copy_from_slice(&bytes[..count]);
}
impl Request {
    /// `length` is a synchronous owner input during initialization/worker work.
    /// File opening is a separate owner operation; an inaccessible first path
    /// only falls back when its length probe is nonpositive, matching C.
    pub fn prepare(
        root: &[u8],
        name: Option<&[u8]>,
        control: u32,
        separator: u8,
        path_capacity: usize,
        mut length: impl FnMut(&[u8]) -> i64,
    ) -> Result<Option<Self>, Error> {
        if !(2..=4096).contains(&path_capacity)
            || !matches!(separator, b'/' | b'\\')
            || root.contains(&0)
        {
            return Err(Error("invalid voice path capacity/root/separator"));
        }
        let name = name.unwrap_or_default();
        if name.contains(&0) {
            return Err(Error("voice request contains NUL"));
        }
        if name.is_empty() && control & 8 == 0 {
            return Ok(None);
        }
        let mut request = Self {
            path: [0; 4096],
            name: [0; 40],
            control,
        };
        string(&mut request.name, name);
        if control & 0x10 != 0 {
            if name.len() >= path_capacity {
                return Err(Error("explicit voice path exceeds capacity"));
            }
            request.path[..name.len()].copy_from_slice(name);
            if length(&request.path[..name.len()]) <= 0 {
                return Ok(None);
            }
        } else {
            let name_length = request
                .name
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(40);
            request.join(root, b"voices", separator, path_capacity, name_length);
            if length(request.path()) <= 0 {
                request.join(root, b"lang", separator, path_capacity, name_length);
            }
        }
        Ok(Some(request))
    }
    fn join(
        &mut self,
        root: &[u8],
        directory: &[u8],
        separator: u8,
        capacity: usize,
        name_length: usize,
    ) {
        self.path.fill(0);
        // C builds the directory prefix with snprintf, then appends the bounded
        // voice name with a second snprintf. Truncation of that prefix is retained.
        let mut used = 0;
        for byte in root
            .iter()
            .chain(std::iter::once(&separator))
            .chain(directory)
            .chain(std::iter::once(&separator))
        {
            if used == capacity - 1 {
                break;
            }
            self.path[used] = *byte;
            used += 1;
        }
        let append = name_length.min(capacity - 1 - used);
        self.path[used..used + append].copy_from_slice(&self.name[..append]);
    }
    pub fn path(&self) -> &[u8] {
        &self.path[..self.path.iter().position(|byte| *byte == 0).unwrap_or(4096)]
    }
    pub fn name(&self) -> &[u8] {
        &self.name[..self.name.iter().position(|byte| *byte == 0).unwrap_or(40)]
    }
    pub fn tone_only(&self) -> bool {
        self.control & 2 != 0
    }
    pub fn compiling(&self) -> bool {
        self.control & 8 != 0
    }
    /// A failed open with no-default/tone-only flags aborts before reset. Otherwise
    /// an existing phoneme table can provide the fallback language identifier.
    pub fn fallback(
        &self,
        opened: bool,
        table_found: bool,
        default: &[u8],
    ) -> Result<Option<[u8; 40]>, Error> {
        if default.len() >= 40 || default.contains(&0) {
            return Err(Error("default voice identifier exceeds bound"));
        }
        if !opened && self.control & 3 != 0 {
            return Ok(None);
        }
        let mut fallback = [0; 40];
        string(
            &mut fallback,
            if !opened && table_found {
                self.name()
            } else if self.compiling() {
                b""
            } else {
                default
            },
        );
        Ok(Some(fallback))
    }
}
/// Preserve the legacy 39-byte current identifier and replace any old variant.
/// Tone-only names use the suffix after the three-byte `!v/` or `!v\\` prefix.
pub fn identifier(
    current: &[u8; 40],
    requested: &[u8],
    tone_only: bool,
) -> Result<[u8; 40], Error> {
    if requested.contains(&0) {
        return Err(Error("voice identifier contains NUL"));
    }
    let mut result = [0; 40];
    if !tone_only {
        string(&mut result, requested);
        return Ok(result);
    }
    if requested.len() < 3 {
        return Err(Error("voice variant prefix is truncated"));
    }
    let size = current
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(Error("current voice identifier is unterminated"))?;
    let end = current[..size]
        .iter()
        .position(|byte| *byte == b'+')
        .unwrap_or(size);
    result[..end].copy_from_slice(&current[..end]);
    if end < 39 {
        result[end] = b'+';
        let count = (requested.len() - 3).min(38 - end);
        result[end + 1..end + 1 + count].copy_from_slice(&requested[3..3 + count]);
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requests_retain_probe_order_truncation_and_fallback_controls() {
        let mut probes = Vec::new();
        let request = Request::prepare(b"root", Some(b"en"), 0, b'/', 4096, |path| {
            probes.push(path.to_vec());
            0
        })
        .unwrap()
        .unwrap();
        assert_eq!(probes, [b"root/voices/en"]);
        assert_eq!(request.path(), b"root/lang/en");
        assert_eq!(
            &request.fallback(false, true, b"en").unwrap().unwrap()[..3],
            b"en\0"
        );
        assert!(Request::prepare(b"root", None, 0, b'/', 4096, |_| panic!(
            "empty request must not probe"
        ))
        .unwrap()
        .is_none());
        let compiler = Request::prepare(b"root", Some(b""), 8, b'/', 4096, |_| 0)
            .unwrap()
            .unwrap();
        assert_eq!(
            compiler.fallback(false, false, b"en").unwrap().unwrap(),
            [0; 40]
        );
        let no_default = Request::prepare(b"root", Some(b"xx"), 1, b'/', 4096, |_| 0)
            .unwrap()
            .unwrap();
        assert!(no_default.fallback(false, true, b"en").unwrap().is_none());
        assert!(Request::prepare(b"root", Some(b"filename"), 16, b'/', 8, |_| 1).is_err());
        let truncated = Request::prepare(b"longroot", Some(b"name"), 0, b'/', 6, |_| 0)
            .unwrap()
            .unwrap();
        assert_eq!(truncated.path(), b"longr");
    }
    #[test]
    fn current_identifier_replaces_the_old_variant_without_overflow() {
        let current = identifier(&[0; 40], b"en+m1", false).unwrap();
        assert_eq!(
            &identifier(&current, b"!v/m2", true).unwrap()[..6],
            b"en+m2\0"
        );
        let full = identifier(&[0; 40], &[b'a'; 50], false).unwrap();
        assert_eq!(identifier(&full, b"!v/m2", true).unwrap(), full);
        assert!(identifier(&current, b"m2", true).is_err());
        assert!(identifier(&[b'a'; 40], b"!v/m2", true).is_err());
    }
}
