//! The fork's Unicode 11.0 classification, case conversion and speech properties.
//! These tables intentionally differ from the Unicode version in Rust's `char`.
// Copyright (C) 2012-2018 Reece H. Dunn; Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later

#[path = "unicode_case.rs"]
mod case;
#[path = "unicode_data.rs"]
mod data;
#[path = "unicode_names.rs"]
mod names;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
pub enum Category {
    Cc,
    Cf,
    Cn,
    Co,
    Cs,
    Ii,
    Ll,
    Lm,
    Lo,
    Lt,
    Lu,
    Mc,
    Me,
    Mn,
    Nd,
    Nl,
    No,
    Pc,
    Pd,
    Pe,
    Pf,
    Pi,
    Po,
    Ps,
    Sc,
    Sk,
    Sm,
    So,
    Zl,
    Zp,
    Zs,
}

const CATEGORIES: [Category; 31] = [
    Category::Cc,
    Category::Cf,
    Category::Cn,
    Category::Co,
    Category::Cs,
    Category::Ii,
    Category::Ll,
    Category::Lm,
    Category::Lo,
    Category::Lt,
    Category::Lu,
    Category::Mc,
    Category::Me,
    Category::Mn,
    Category::Nd,
    Category::Nl,
    Category::No,
    Category::Pc,
    Category::Pd,
    Category::Pe,
    Category::Pf,
    Category::Pi,
    Category::Po,
    Category::Ps,
    Category::Sc,
    Category::Sk,
    Category::Sm,
    Category::So,
    Category::Zl,
    Category::Zp,
    Category::Zs,
];

impl Category {
    pub fn from_id(id: u32) -> Option<Self> {
        CATEGORIES.get(id as usize).copied()
    }
    pub fn name(self) -> &'static str {
        names::CATEGORY_NAMES[self as usize].trim_end_matches('\0')
    }
    /// Group IDs agree with `ucd_category_group` in the C ABI.
    pub fn group(self) -> u32 {
        match self {
            Self::Cc | Self::Cf | Self::Cn | Self::Co | Self::Cs => 0,
            Self::Ii => 1,
            Self::Ll | Self::Lm | Self::Lo | Self::Lt | Self::Lu => 2,
            Self::Mc | Self::Me | Self::Mn => 3,
            Self::Nd | Self::Nl | Self::No => 4,
            Self::Pc | Self::Pd | Self::Pe | Self::Pf | Self::Pi | Self::Po | Self::Ps => 5,
            Self::Sc | Self::Sk | Self::Sm | Self::So => 6,
            Self::Zl | Self::Zp | Self::Zs => 7,
        }
    }
}

fn range_value(table: &[(u32, u32, u64)], c: u32) -> u64 {
    let index = table.partition_point(|(_, end, _)| *end < c);
    table
        .get(index)
        .filter(|(start, _, _)| *start <= c)
        .map_or(0, |(_, _, value)| *value)
}

pub fn category(c: u32) -> Category {
    CATEGORIES[range_value(data::CATEGORIES, c) as usize]
}
pub fn script(c: u32) -> u32 {
    range_value(data::SCRIPTS, c) as u32
}
pub fn script_name(id: u32) -> &'static str {
    script_c_string(id).trim_end_matches('\0')
}
pub(crate) fn script_c_string(id: u32) -> &'static str {
    names::SCRIPT_NAMES
        .get(id as usize)
        .copied()
        .unwrap_or("----\0")
}
#[cfg(feature = "c-abi")]
pub(crate) fn category_c_string(id: u32) -> &'static str {
    names::CATEGORY_NAMES
        .get(id as usize)
        .copied()
        .unwrap_or("--\0")
}

pub fn properties(c: u32, cat: Category) -> u64 {
    // The C API also accepts a caller-supplied category for invalid codepoints.
    // Its Cn noncharacter rule repeats every 64K across the full u32 domain.
    if c > 0x10ffff && cat == Category::Cn {
        return if c & 0xffff >= 0xfffe { 0x10000 } else { 0 };
    }
    range_value(data::PROPERTIES[cat as usize], c)
}
fn has(c: u32, cat: Category, bit: u64) -> bool {
    properties(c, cat) & bit != 0
}

fn convert(c: u32, column: usize) -> u32 {
    case::CASE
        .binary_search_by_key(&c, |entry| entry[0])
        .ok()
        .map_or(c, |i| {
            let mapped = case::CASE[i][column];
            if mapped == 0 {
                c
            } else {
                mapped
            }
        })
}
pub fn to_upper(c: u32) -> u32 {
    convert(c, 1)
}
pub fn to_lower(c: u32) -> u32 {
    convert(c, 2)
}
pub fn to_title(c: u32) -> u32 {
    convert(c, 3)
}

pub fn is_alpha(c: u32) -> bool {
    let cat = category(c);
    match cat {
        Category::Lu | Category::Ll | Category::Lt | Category::Lm | Category::Lo | Category::Nl => {
            true
        }
        Category::Mn | Category::Mc | Category::So => has(c, cat, 0x400),
        _ => false,
    }
}
pub fn is_alnum(c: u32) -> bool {
    is_alpha(c) || matches!(category(c), Category::Nd | Category::No)
}
pub fn is_blank(c: u32) -> bool {
    match category(c) {
        Category::Zs => !matches!(c, 0xa0 | 0x2007 | 0x202f),
        Category::Cc => c == 9,
        _ => false,
    }
}
pub fn is_control(c: u32) -> bool {
    category(c) == Category::Cc
}
pub fn is_digit(c: u32) -> bool {
    (0x30..=0x39).contains(&c)
}
pub fn is_graph(c: u32) -> bool {
    is_print(c) && !matches!(category(c), Category::Zl | Category::Zp | Category::Zs)
}
pub fn is_lower(c: u32) -> bool {
    let cat = category(c);
    match cat {
        Category::Ll => true,
        Category::Lt => to_upper(c) != c,
        Category::Lm | Category::Lo | Category::Mn | Category::Nl | Category::So => {
            has(c, cat, 0x4000)
        }
        _ => false,
    }
}
pub fn is_print(c: u32) -> bool {
    !matches!(
        category(c),
        Category::Cc | Category::Cf | Category::Cn | Category::Co | Category::Cs | Category::Ii
    )
}
pub fn is_punct(c: u32) -> bool {
    is_graph(c) && !is_alnum(c)
}
pub fn is_space(c: u32) -> bool {
    matches!(category(c), Category::Zl | Category::Zp) || is_blank(c) || matches!(c, 10..=13 | 0x85)
}
pub fn is_upper(c: u32) -> bool {
    let cat = category(c);
    match cat {
        Category::Lu => true,
        Category::Lt => to_lower(c) != c,
        Category::Nl | Category::So => has(c, cat, 0x8000),
        _ => false,
    }
}
pub fn is_hex_digit(c: u32) -> bool {
    matches!(c, 0x30..=0x39 | 0x41..=0x46 | 0x61..=0x66)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn speech_specific_unicode_semantics() {
        assert!(!is_space(0xa0));
        assert!(is_space(0x2028));
        assert!(is_alnum(0x0661));
        assert!(!is_digit(0x0661));
        assert_eq!(category(0xd800), Category::Cs);
        assert_eq!(category(u32::MAX), Category::Ii);
        assert_eq!(to_title(0x1c6), 0x1c5);
        assert_eq!(script_name(script(0x41)), "Latn");
        assert_eq!(category(0x1fae0), Category::Cn); // assigned after Unicode 11
    }
    #[test]
    fn range_tables_are_sorted_disjoint_and_names_cover_ids() {
        for table in [data::CATEGORIES, data::SCRIPTS]
            .into_iter()
            .chain(data::PROPERTIES.iter().copied())
        {
            for row in table {
                assert!(row.0 <= row.1);
            }
            for pair in table.windows(2) {
                assert!(pair[0].1 < pair[1].0);
            }
        }
        assert_eq!(names::CATEGORY_NAMES.len(), CATEGORIES.len());
        assert!(case::CASE.windows(2).all(|p| p[0][0] < p[1][0]));
    }
}
