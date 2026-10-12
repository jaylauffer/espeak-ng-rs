//! Complete resident phoneme reload, teardown and table-selection ownership.
// Copyright (C) 2005-2014 Jonathan Duddington, 2015-2017 Reece H. Dunn.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    core_storage::{Slot, Storage},
    phoneme_data::{self, TableIndex, MISSING},
};
use std::io;
pub const FORMAT: i32 = 0x10000bff;
pub const TUNE_BYTES: usize = 68;
pub const NAMES: [&[u8]; 4] = [b"phontab", b"phonindex", b"phondata", b"intonations"];
pub struct Paths {
    pub files: [[u8; 4096]; 4],
}
impl Paths {
    pub fn new(home: &[u8], width: usize, separator: u8) -> Result<Self, i32> {
        if !(2..=4096).contains(&width) || home.contains(&0) || separator == 0 {
            return Err(22);
        }
        let mut files = [[0; 4096]; 4];
        for (out, name) in files.iter_mut().zip(NAMES) {
            let n = home.len().min(width - 1);
            out[..n].copy_from_slice(&home[..n]);
            if n + 1 < width {
                out[n] = separator;
                let m = name.len().min(width - 2 - n);
                out[n + 1..n + 1 + m].copy_from_slice(&name[..m]);
            }
        }
        Ok(Self { files })
    }
}
pub trait Host {
    /// Serialized projection of live resident storage. The projection itself
    /// cannot be retained; compatibility byte views expire before mutation.
    fn project(&mut self, workspace: &mut Workspace);
    fn file_error(&mut self, path: &[u8], error: io::Error) -> i32;
    fn version_error(&mut self, actual: u32) -> i32;
}
pub struct Workspace {
    pub storage: Storage,
    pub index: Option<TableIndex>,
    pub selected: [usize; 256],
    pub current: i32,
    pub number: i32,
    pub active: i32,
    pub tunes: i32,
    pub wave_ready: bool,
}
impl Default for Workspace {
    fn default() -> Self {
        Self {
            storage: Storage::default(),
            index: None,
            selected: [MISSING; 256],
            current: -1,
            number: 0,
            active: 0,
            tunes: 0,
            wave_ready: false,
        }
    }
}
impl Workspace {
    /// A reload revokes selected/table/wave views before any buffer changes.
    /// Slot admission/open failure retains old bytes; failed exact I/O clears
    /// only the affected slot. No stale wave pointer survives a partial reload.
    pub fn load(&mut self, paths: &Paths, number: i32, host: &mut impl Host) -> Result<u32, i32> {
        self.index = None;
        self.current = -1;
        self.number = number;
        self.selected.fill(MISSING);
        self.wave_ready = false;
        self.tunes = 0;
        host.project(self);
        for (i, slot) in [
            Slot::Phontab,
            Slot::Phonindex,
            Slot::Phondata,
            Slot::Intonations,
        ]
        .into_iter()
        .enumerate()
        {
            let path = match crate::voice_storage::compat_path(crate::compiler_driver::string(
                &paths.files[i],
            )) {
                Ok(p) => p,
                Err(e) => {
                    host.project(self);
                    return Err(host.file_error(&paths.files[i], e));
                }
            };
            if let Err(error) = self.storage.load(slot, &path) {
                host.project(self);
                if error.kind() == io::ErrorKind::OutOfMemory {
                    return Err(12);
                }
                return Err(host.file_error(&paths.files[i], error));
            }
        }
        self.wave_ready = true;
        self.tunes = (self.storage.bytes(Slot::Intonations).len() / TUNE_BYTES) as i32;
        let data = self.storage.bytes(Slot::Phondata);
        let [version, _] = match phoneme_data::header(data) {
            Ok(h) => h,
            Err(_) => {
                host.project(self);
                return Err(FORMAT);
            }
        };
        if version != phoneme_data::PHONDATA_VERSION {
            host.project(self);
            return Err(host.version_error(version));
        }
        let rate = match phoneme_data::sample_rate(data) {
            Ok(rate) => rate,
            Err(_) => {
                host.project(self);
                return Err(FORMAT);
            }
        };
        let index = match TableIndex::parse(self.storage.bytes(Slot::Phontab)) {
            Ok(index) => index,
            Err(_) => {
                host.project(self);
                return Err(FORMAT);
            }
        };
        if self.number >= index.tables().len() as i32 {
            self.number = 0;
        }
        self.index = Some(index);
        host.project(self);
        Ok(rate)
    }
    pub fn select(&mut self, number: i32) -> bool {
        if self.current == number {
            return true;
        }
        self.active = 0;
        self.selected.fill(MISSING);
        self.current = -1;
        let selected = if number < 0 {
            None
        } else {
            self.index.as_ref().and_then(|index| {
                index
                    .select(self.storage.bytes(Slot::Phontab), number as usize)
                    .ok()
            })
        };
        let Some(selected) = selected else {
            return false;
        };
        self.active = selected
            .iter()
            .rposition(|offset| *offset != MISSING)
            .unwrap_or(0) as i32
            + 1;
        self.selected = selected;
        self.current = number;
        true
    }
    pub fn lookup(&mut self, name: &[u8]) -> i32 {
        let number = self.index.as_ref().and_then(|index| index.lookup(name));
        if let Some(number) = number {
            self.number = number as i32;
        }
        number.map_or(-1, |number| number as i32)
    }
    pub fn select_name(&mut self, name: &[u8]) -> i32 {
        let number = self.lookup(name);
        if number >= 0 {
            self.select(number);
        }
        number
    }
    /// Revoke every compatibility view before actually releasing storage.
    /// Retain the requested table number, matching engine teardown semantics.
    pub fn release(&mut self, host: &mut impl Host) {
        self.index = None;
        self.selected.fill(MISSING);
        self.current = -1;
        self.active = 0;
        self.tunes = 0;
        self.wave_ready = false;
        // Callback observes no resident views while old allocations are live.
        let number = self.number;
        let mut empty = Self {
            number,
            ..Self::default()
        };
        host.project(&mut empty);
        self.storage = Storage::default();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Effects {
        projections: Vec<(usize, i32, bool, usize)>,
        error_path: Vec<u8>,
    }
    impl Host for Effects {
        fn project(&mut self, w: &mut Workspace) {
            self.projections.push((
                w.index.as_ref().map_or(0, |i| i.tables().len()),
                w.active,
                w.wave_ready,
                w.storage.reserved_bytes(),
            ));
        }
        fn file_error(&mut self, p: &[u8], e: io::Error) -> i32 {
            self.error_path = crate::compiler_driver::string(p).to_vec();
            crate::compiler_driver::errno(&e)
        }
        fn version_error(&mut self, _: u32) -> i32 {
            0x100002ff
        }
    }
    fn effects() -> Effects {
        Effects {
            projections: Vec::new(),
            error_path: Vec::new(),
        }
    }
    fn table() -> Vec<u8> {
        let mut out = vec![3, 0, 0, 0];
        for (i, codes) in [&[1, 100][..], &[2][..], &[3][..]].into_iter().enumerate() {
            out.extend_from_slice(&[codes.len() as u8, u8::from(i != 0), 0, 0]);
            let mut name = [0; 32];
            name[0] = b'a' + i as u8;
            out.extend_from_slice(&name);
            for &code in codes {
                let mut row = [0; 16];
                row[0] = code;
                row[10] = code;
                out.extend_from_slice(&row);
            }
        }
        out
    }
    fn directory(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("espeak-assets-{tag}-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        root
    }
    fn source(root: &std::path::Path) -> Paths {
        std::fs::write(root.join("phontab"), table()).unwrap();
        std::fs::write(root.join("phonindex"), [1, 2, 3, 4]).unwrap();
        std::fs::write(
            root.join("phondata"),
            [
                crate::phoneme_data::PHONDATA_VERSION.to_le_bytes(),
                22050_u32.to_le_bytes(),
            ]
            .concat(),
        )
        .unwrap();
        std::fs::write(root.join("intonations"), [0; TUNE_BYTES * 2 + 1]).unwrap();
        Paths::new(root.to_str().unwrap().as_bytes(), 4096, b'/').unwrap()
    }
    #[test]
    fn reload_selection_and_release_reuse_resident_storage_and_revoke_views() {
        let root = directory("warm");
        let paths = source(&root);
        let mut w = Workspace::default();
        let mut h = effects();
        assert_eq!(w.load(&paths, 99, &mut h), Ok(22050));
        assert_eq!((w.number, w.tunes), (0, 2));
        let pointers = [
            Slot::Phontab,
            Slot::Phonindex,
            Slot::Phondata,
            Slot::Intonations,
        ]
        .map(|s| w.storage.bytes(s).as_ptr());
        for _ in 0..20 {
            assert_eq!(w.load(&paths, 1, &mut h), Ok(22050));
            assert_eq!(w.select_name(b"b"), 1);
            assert_ne!(w.selected[2], MISSING);
            assert_eq!(w.select_name(b"c"), 2);
            assert_eq!(w.selected[2], MISSING);
            assert_ne!(w.selected[3], MISSING);
            assert_eq!(w.active, 101);
            assert_eq!(
                pointers,
                [
                    Slot::Phontab,
                    Slot::Phonindex,
                    Slot::Phondata,
                    Slot::Intonations
                ]
                .map(|s| w.storage.bytes(s).as_ptr())
            );
        }
        assert_eq!(w.lookup(b"missing"), -1);
        assert_eq!(w.number, 2);
        w.release(&mut h);
        assert_eq!(h.projections.last(), Some(&(0, 0, false, 0)));
        assert_eq!(w.storage.reserved_bytes(), 0);
        assert_eq!(w.number, 2);
        assert!(!w.select(1));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn partial_reload_and_bad_headers_do_not_leave_selected_or_wave_views() {
        let root = directory("failure");
        let paths = source(&root);
        let mut w = Workspace::default();
        let mut h = effects();
        w.load(&paths, 0, &mut h).unwrap();
        assert!(w.select(1));
        std::fs::write(root.join("phondata"), [1; 8192]).unwrap();
        std::fs::remove_file(root.join("intonations")).unwrap();
        assert_eq!(w.load(&paths, 1, &mut h), Err(2));
        assert!(!w.wave_ready && w.index.is_none() && w.selected.iter().all(|v| *v == MISSING));
        assert_eq!(w.tunes, 0);
        assert_eq!(w.storage.bytes(Slot::Phondata).len(), 8192);
        assert_eq!(
            h.error_path,
            root.join("intonations").to_str().unwrap().as_bytes()
        );
        let paths = source(&root);
        std::fs::write(root.join("phondata"), [1; 3]).unwrap();
        assert_eq!(w.load(&paths, 0, &mut h), Err(FORMAT));
        assert!(w.index.is_none());
        assert!(!w.select(0));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn paths_preserve_platform_clipping_and_lookup_preserves_missing_selection() {
        assert_eq!(
            crate::compiler_driver::string(&Paths::new(b"/home", 9, b'/').unwrap().files[0]),
            b"/home/ph"
        );
        assert!(Paths::new(b"/home", 1, b'/').is_err());
        let mut w = Workspace {
            number: 7,
            ..Workspace::default()
        };
        assert_eq!(w.select_name(b"missing"), -1);
        assert_eq!(w.number, 7);
        assert!(!w.select(99));
        assert_eq!(w.active, 0);
        assert_eq!(w.current, -1);
    }
}
