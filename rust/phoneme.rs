//! Phoneme records and the fork's articulatory-feature mutations.
// Copyright (C) 2017 Reece H. Dunn; Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later

/// Layout matches the legacy PHONEME_TAB and compiled 16-byte records.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct Phoneme {
    pub mnemonic: u32,
    pub flags: u32,
    pub program: u16,
    pub code: u8,
    pub kind: u8,
    pub start_type: u8,
    pub end_type: u8,
    pub standard_length: u8,
    pub length_modifier: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnknownFeature;
impl std::fmt::Display for UnknownFeature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("unknown phoneme feature")
    }
}
impl std::error::Error for UnknownFeature {}

pub fn feature_from_name(name: &[u8]) -> u32 {
    if name.len() != 3 {
        return 0;
    }
    (u32::from(name[0]) << 16) | (u32::from(name[1]) << 8) | u32::from(name[2])
}

impl Phoneme {
    pub fn from_record(bytes: &[u8; 16]) -> Self {
        Self {
            mnemonic: u32::from_le_bytes(bytes[..4].try_into().expect("four bytes")),
            flags: u32::from_le_bytes(bytes[4..8].try_into().expect("four bytes")),
            program: u16::from_le_bytes([bytes[8], bytes[9]]),
            code: bytes[10],
            kind: bytes[11],
            start_type: bytes[12],
            end_type: bytes[13],
            standard_length: bytes[14],
            length_modifier: bytes[15],
        }
    }

    pub fn add_feature(&mut self, feature: u32) -> Result<(), UnknownFeature> {
        if feature >> 24 != 0 {
            return Err(UnknownFeature);
        }
        let name = feature.to_be_bytes();
        let place = match &name[1..] {
            b"blb" | b"bld" => Some(1),
            b"lbd" => Some(2),
            b"dnt" => Some(3),
            b"alv" => Some(4),
            b"rfx" => Some(5),
            b"pla" | b"alp" => Some(6),
            b"pal" => Some(7),
            b"vel" => Some(8),
            b"lbv" => Some(9),
            b"uvl" => Some(10),
            b"phr" => Some(11),
            b"glt" => Some(12),
            _ => None,
        };
        if let Some(place) = place {
            self.flags = (self.flags & !0x000f_0000) | (place << 16);
            if matches!(&name[1..], b"pal" | b"alp") {
                self.flags |= 1 << 9;
            }
            return Ok(());
        }
        match &name[1..] {
            b"nas" => self.kind = 8,
            b"stp" | b"afr" => self.kind = 4,
            b"frc" | b"apr" => self.kind = 6,
            b"flp" => self.kind = 5,
            b"vwl" => self.kind = 2,
            b"trl" => self.flags |= 1 << 7,
            b"sib" => self.flags |= 1 << 5,
            b"vcd" => self.flags |= 1 << 4,
            b"vls" => self.flags |= 1 << 3,
            b"nsy" => self.flags |= 1 << 20,
            b"pzd" => self.flags |= 1 << 9,
            b"lng" | b"elg" => self.flags |= 1 << 21,
            // Recognized but unsupported features retain the fork's no-op semantics.
            b"clk" | b"ejc" | b"imp" | b"lat" | b"hgh" | b"smh" | b"umd" | b"mid" | b"lmd"
            | b"sml" | b"low" | b"fnt" | b"cnt" | b"bck" | b"unr" | b"rnd" | b"lgl" | b"idt"
            | b"apc" | b"lmn" | b"egs" | b"igs" | b"brv" | b"slv" | b"stv" | b"crv" | b"glc"
            | b"ptr" | b"cmp" | b"mrd" | b"lrd" | b"syl" | b"asp" | b"nrs" | b"lrs" | b"unx"
            | b"vzd" | b"fzd" | b"nzd" | b"rzd" | b"atr" | b"rtr" | b"fts" | b"lns" | b"est"
            | b"hlg" => {}
            _ => return Err(UnknownFeature),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn articulation_preserves_other_bits_and_unsupported_features_are_noops() {
        assert_eq!(std::mem::size_of::<Phoneme>(), 16);
        let mut ph = Phoneme {
            flags: 0x800f_0000,
            ..Phoneme::default()
        };
        ph.add_feature(feature_from_name(b"pal")).unwrap();
        assert_eq!(ph.flags, 0x8007_0200);
        let prior = ph;
        ph.add_feature(feature_from_name(b"clk")).unwrap();
        assert_eq!(ph, prior);
        assert!(ph.add_feature(feature_from_name(b"xyz")).is_err());
        assert_eq!(ph, prior);
    }
}
