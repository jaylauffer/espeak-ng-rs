//! Ordered mnemonic lookup, with first-match and sentinel-default semantics.
// Copyright (C) 2005-2014 Jonathan Duddington, (C) 2013-2017 Reece H. Dunn.
// SPDX-License-Identifier: GPL-3.0-or-later

pub fn lookup(table: &[(&str, i32)], name: Option<&str>, default: i32) -> i32 {
    table
        .iter()
        .find(|(key, _)| Some(*key) == name)
        .map_or(default, |(_, value)| *value)
}

pub fn lookup_name<'a>(table: &'a [(&'a str, i32)], value: i32) -> &'a str {
    table
        .iter()
        .find(|(_, candidate)| *candidate == value)
        .map_or("", |(name, _)| *name)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicates_use_the_first_match_and_unknown_uses_default() {
        let table = [("a", 7), ("a", 8), ("b", 7)];
        assert_eq!(lookup(&table, Some("a"), -1), 7);
        assert_eq!(lookup(&table, None, -1), -1);
        assert_eq!(lookup_name(&table, 7), "a");
        assert_eq!(lookup_name(&table, -1), "");
    }
}
