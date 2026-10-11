//! Owned phoneme compiler tables: reserve, inherit, lookup/import and packing.
// Copyright (C) 2005-2014 Jonathan Duddington, 2013-2017 Reece H. Dunn.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::phoneme::{mnemonic, Phoneme};
pub const TABLES: usize = 149;
pub const ROWS: usize = 256;
pub const LOCAL: u32 = 1 << 31;
pub const INVALID: u8 = 15;
pub const MAX_PAYLOAD: usize = 4 + TABLES * (36 + ROWS * 16);
const RESERVED: [(&[u8], usize); 32] = [
    (b"_\x01", 1),
    (b"%", 2),
    (b"%%", 3),
    (b",", 4),
    (b",,", 5),
    (b"'", 6),
    (b"''", 7),
    (b"=", 8),
    (b"_:", 9),
    (b"_", 10),
    (b"_!", 11),
    (b":", 12),
    (b"@", 13),
    (b"@-", 14),
    (b"||", 15),
    (b"1", 17),
    (b"#X1", 18),
    (b"?", 19),
    (b"-", 20),
    (b"_^_", 21),
    (b"_X1", 22),
    (b"_|", 23),
    (b"_::", 24),
    (b"t#", 25),
    (b"'!", 26),
    (b"_;_", 27),
    (b"#@", 28),
    (b"#a", 29),
    (b"#e", 30),
    (b"#i", 31),
    (b"#o", 32),
    (b"#u", 33),
];
pub trait Host {
    fn base(&mut self) -> Result<[u8; 256], i32>;
    /// 1 name, 2 missing table, 3 missing reference, 4 undeclared (weight 2),
    /// 5 table cap, 6 allocation, 7 missing base, 8 import missing, 9 override.
    fn error(&mut self, code: i32, a: i32, b: i32, name: &[u8]);
    fn write(&mut self, bytes: &[u8]) -> Result<(), i32>;
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Table {
    pub name: [u8; 32],
    pub count: usize,
    pub includes: i32,
    pub emitted: usize,
    pub closed: bool,
}
#[derive(Default)]
pub struct Workspace {
    tables: Vec<Table>,
    rows: Vec<Phoneme>,
    count: usize,
    payload: Vec<u8>,
    packed: bool,
}
pub fn text(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len())]
}
fn record(out: &mut Vec<u8>, row: Phoneme) {
    out.extend_from_slice(&row.mnemonic.to_le_bytes());
    out.extend_from_slice(&row.flags.to_le_bytes());
    out.extend_from_slice(&row.program.to_le_bytes());
    out.extend_from_slice(&[
        row.code,
        row.kind,
        row.start_type,
        row.end_type,
        row.standard_length,
        row.length_modifier,
    ]);
}
impl Workspace {
    pub fn tables(&self) -> &[Table] {
        &self.tables
    }
    pub fn count(&self) -> usize {
        self.count
    }
    pub fn clear(&mut self) {
        self.tables.clear();
        self.rows.clear();
        self.payload.clear();
        self.count = 0;
        self.packed = false;
    }
    pub fn rows(&self, table: usize) -> Option<&[Phoneme]> {
        self.rows
            .get(table.checked_mul(ROWS)?..table.checked_add(1)?.checked_mul(ROWS)?)
    }
    pub fn rows_mut(&mut self, table: usize) -> Option<&mut [Phoneme]> {
        self.rows
            .get_mut(table.checked_mul(ROWS)?..table.checked_add(1)?.checked_mul(ROWS)?)
    }
    fn active(&self) -> Result<usize, i32> {
        if self.packed {
            return Err(22);
        }
        self.tables.len().checked_sub(1).ok_or(22)
    }
    pub fn start(&mut self, host: &mut impl Host, name: &[u8]) -> Result<(), i32> {
        if self.packed || name.len() > 255 {
            return Err(22);
        }
        if self.tables.len() >= TABLES {
            host.error(5, 0, 0, b"");
            return Ok(());
        }
        if self
            .rows
            .try_reserve_exact(TABLES * ROWS - self.rows.len())
            .is_err()
            || self
                .tables
                .try_reserve_exact(TABLES - self.tables.len())
                .is_err()
        {
            host.error(6, 0, 0, b"");
            return Err(12);
        }
        let mut table = Table::default();
        let n = name.len().min(31);
        table.name[..n].copy_from_slice(&name[..n]);
        let index = self.tables.len();
        let old_count = self.count;
        self.rows.resize((index + 1) * ROWS, Phoneme::default());
        self.count = 1;
        if index == 0 {
            for (name, code) in RESERVED {
                let row = &mut self.rows[code];
                row.mnemonic = mnemonic(name);
                row.code = code as u8;
                self.count = self.count.max(code + 1);
            }
        } else {
            let token = match host.base() {
                Ok(token) => token,
                Err(code) => {
                    self.rows.truncate(index * ROWS);
                    self.count = old_count;
                    return Err(code);
                }
            };
            let base = text(&token);
            if let Some(parent) = self.tables.iter().position(|t| text(&t.name) == base) {
                if !self.tables[parent].closed {
                    self.rows.truncate(index * ROWS);
                    self.count = old_count;
                    return Err(22);
                }
                self.rows
                    .copy_within(parent * ROWS..(parent + 1) * ROWS, index * ROWS);
                self.count = self.tables[parent].count;
                table.includes = (parent + 1) as i32;
                for row in &mut self.rows[index * ROWS..index * ROWS + self.count] {
                    row.flags &= !LOCAL;
                }
            } else if base != b"_" {
                host.error(7, 0, 0, base);
            }
        }
        self.tables.push(table);
        Ok(())
    }
    pub fn lookup(
        &mut self,
        host: &mut impl Host,
        name: &[u8],
        control: i32,
        line: i32,
    ) -> Result<i32, i32> {
        let index = self.active()?;
        if name.len() > 255 || self.count > 255 || self.count == 0 {
            return Err(22);
        }
        if name == b"NULL" {
            return Ok(1);
        }
        if name.is_empty() || name.len() > 4 {
            host.error(1, 0, 0, name);
        }
        let word = mnemonic(name);
        let start = if control == 2 { 8 } else { 1 };
        let mut free = None;
        for i in start..self.count {
            let row = self.rows[index * ROWS + i];
            if row.mnemonic == word {
                return Ok(i as i32);
            }
            if free.is_none() && row.mnemonic == 0 {
                free = Some(i);
            }
        }
        let code = if let Some(free) = free {
            free
        } else {
            if control == 0 || self.count >= 255 {
                return Ok(-1);
            }
            let n = self.count;
            self.count += 1;
            n
        };
        let row = &mut self.rows[index * ROWS + code];
        row.mnemonic = word;
        row.kind = INVALID;
        row.program = line as u16;
        Ok(code as i32)
    }
    pub fn find(
        &mut self,
        host: &mut impl Host,
        name: &[u8],
        line: i32,
    ) -> Result<Option<(usize, usize)>, i32> {
        let active = self.active()?;
        if name.len() > 255 {
            return Err(22);
        }
        if name.len() <= 4 {
            let code = self.lookup(host, name, 0, line)?;
            if code >= 0 {
                return Ok(Some((active, code as usize)));
            }
        }
        let slash = name.iter().position(|b| *b == b'/');
        let table_name = slash.map_or(name, |i| &name[..i]);
        let Some(table) = self.tables.iter().position(|t| text(&t.name) == table_name) else {
            host.error(2, 0, 0, table_name);
            return Ok(None);
        };
        let word = mnemonic(slash.map_or(b"".as_slice(), |i| &name[i + 1..]));
        if let Some(code) = (1..ROWS).find(|i| self.rows[table * ROWS + *i].mnemonic == word) {
            return Ok(Some((table, code)));
        }
        host.error(3, 0, 0, name);
        Ok(None)
    }
    pub fn import(
        &mut self,
        host: &mut impl Host,
        name: &[u8],
        target: Phoneme,
        line: i32,
    ) -> Result<Phoneme, i32> {
        let Some((table, code)) = self.find(host, name, line)? else {
            host.error(8, 0, 0, name);
            return Ok(target);
        };
        if target.flags != 0
            || target.kind != INVALID
            || target.start_type != 0
            || target.end_type != 0
            || target.standard_length != 0
            || target.length_modifier != 0
        {
            host.error(9, 0, 0, b"");
        }
        let mut row = self.rows[table * ROWS + code];
        row.mnemonic = target.mnemonic;
        row.code = target.code;
        if row.kind != 2 {
            row.end_type = 0;
        }
        Ok(row)
    }
    pub fn finish_table(&mut self, host: &mut impl Host) -> Result<(), i32> {
        if self.tables.is_empty() {
            return Ok(());
        }
        let index = self.active()?;
        if self.count == 0 || self.count > 255 {
            return Err(22);
        }
        for i in 0..self.count {
            let row = self.rows[index * ROWS + i];
            if row.kind == INVALID {
                let bytes = row.mnemonic.to_le_bytes();
                host.error(4, row.program as i32, 0, text(&bytes));
                self.rows[index * ROWS + i].kind = 0;
            }
        }
        self.tables[index].count = self.count;
        self.tables[index].closed = true;
        Ok(())
    }
    /// Old unused n+1 terminator is retained where it fits; n==255 used to
    /// write past the C allocation. Packing itself uses the checked n slot.
    pub fn terminate_unused(&mut self) -> Result<(), i32> {
        let i = self.active()?;
        if self.count > 255 {
            return Err(22);
        }
        if self.count + 1 < ROWS {
            self.rows[i * ROWS + self.count + 1] = Phoneme::default();
        }
        Ok(())
    }
    pub fn pack(&mut self, host: &mut impl Host) -> Result<(), i32> {
        if self.packed || self.tables.iter().any(|t| !t.closed || t.count > 255) {
            return Err(22);
        }
        self.payload.clear();
        self.payload
            .try_reserve_exact(MAX_PAYLOAD)
            .map_err(|_| 12)?;
        self.payload
            .extend_from_slice(&[self.tables.len() as u8, 0, 0, 0]);
        for (i, table) in self.tables.iter_mut().enumerate() {
            self.rows[i * ROWS + table.count] = Phoneme::default();
            if i == 0 {
                for row in &mut self.rows[..table.count] {
                    row.flags |= LOCAL;
                }
            }
            let count = self.rows[i * ROWS..i * ROWS + table.count]
                .iter()
                .filter(|r| r.flags & LOCAL != 0)
                .count();
            table.emitted = count + 1;
            self.payload
                .extend_from_slice(&[(count + 1) as u8, table.includes as u8, 0, 0]);
            self.payload.extend_from_slice(&table.name);
            for row in &mut self.rows[i * ROWS..i * ROWS + table.count] {
                if row.flags & LOCAL != 0 {
                    row.flags &= !LOCAL;
                    record(&mut self.payload, *row);
                }
            }
            record(&mut self.payload, Phoneme::default());
        }
        self.packed = true;
        host.write(&self.payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Capture {
        base: Vec<u8>,
        diagnostics: Vec<(i32, i32, Vec<u8>)>,
        bytes: Vec<u8>,
        calls: usize,
        fail_base: i32,
        fail_write: i32,
    }
    impl Host for Capture {
        fn base(&mut self) -> Result<[u8; 256], i32> {
            self.calls += 1;
            if self.fail_base != 0 {
                return Err(self.fail_base);
            }
            let mut out = [0; 256];
            out[..self.base.len()].copy_from_slice(&self.base);
            Ok(out)
        }
        fn error(&mut self, code: i32, a: i32, _b: i32, name: &[u8]) {
            self.diagnostics.push((code, a, name.to_vec()));
        }
        fn write(&mut self, bytes: &[u8]) -> Result<(), i32> {
            self.bytes.extend_from_slice(bytes);
            if self.fail_write == 0 {
                Ok(())
            } else {
                Err(self.fail_write)
            }
        }
    }
    #[test]
    fn reserved_codes_sparse_lookup_null_and_line_narrowing_match_policy() {
        let mut w = Workspace::default();
        let mut h = Capture::default();
        w.start(&mut h, b"base").unwrap();
        assert_eq!(w.count(), 34);
        for (name, code) in RESERVED {
            assert_eq!(w.lookup(&mut h, name, 1, 3).unwrap(), code as i32);
            assert_eq!(w.rows(0).unwrap()[code].code, code as u8);
        }
        assert_eq!(w.lookup(&mut h, b"NULL", 2, 5), Ok(1));
        assert_eq!(w.lookup(&mut h, b"new", 0, -1), Ok(16));
        assert_eq!(w.rows(0).unwrap()[16].program, 65535);
        assert_eq!(w.lookup(&mut h, b"new", 1, 2), Ok(16));
        assert_eq!(w.lookup(&mut h, b"abcde", 1, 65536), Ok(34));
        assert_eq!(w.rows(0).unwrap()[34].mnemonic, mnemonic(b"abcd"));
        assert_eq!(w.rows(0).unwrap()[34].program, 0);
        w.finish_table(&mut h).unwrap();
        let count = h.diagnostics.len();
        w.finish_table(&mut h).unwrap();
        assert_eq!(h.diagnostics.len(), count);
        assert_eq!(h.diagnostics.iter().filter(|(c, _, _)| *c == 4).count(), 2);
    }
    #[test]
    fn inheritance_import_override_and_qualified_empty_reference_are_owned() {
        let mut w = Workspace::default();
        let mut h = Capture::default();
        w.start(&mut h, b"base-table").unwrap();
        let n = w.lookup(&mut h, b"xx", 1, 7).unwrap() as usize;
        w.rows_mut(0).unwrap()[n] = Phoneme {
            mnemonic: mnemonic(b"xx"),
            flags: LOCAL | 17,
            kind: 4,
            code: n as u8,
            end_type: 8,
            program: 123,
            ..Phoneme::default()
        };
        w.finish_table(&mut h).unwrap();
        h.base = b"base-table".to_vec();
        w.start(&mut h, b"child").unwrap();
        assert_eq!(w.tables()[1].includes, 1);
        assert_eq!(w.rows(1).unwrap()[n].flags, 17);
        let target = Phoneme {
            mnemonic: mnemonic(b"yy"),
            kind: INVALID,
            code: 66,
            ..Phoneme::default()
        };
        let imported = w.import(&mut h, b"base-table/xx", target, 9).unwrap();
        assert_eq!(imported.mnemonic, target.mnemonic);
        assert_eq!(imported.code, 66);
        assert_eq!(imported.end_type, 0);
        assert_eq!(imported.program, 123);
        let changed = Phoneme {
            kind: 2,
            flags: 1,
            ..target
        };
        w.import(&mut h, b"base-table/xx", changed, 9).unwrap();
        assert_eq!(h.diagnostics.last().unwrap().0, 9);
        assert!(w.find(&mut h, b"base-table", 2).unwrap().is_some()); /* no slash => zero mnemonic, C NULL word */
        assert_eq!(w.import(&mut h, b"absent/xx", target, 1).unwrap(), target);
        assert_eq!(h.diagnostics.last().unwrap().0, 8);
    }
    #[test]
    fn packing_is_explicit_and_filters_child_local_rows_with_no_replay_on_failure() {
        let mut w = Workspace::default();
        let mut h = Capture::default();
        w.start(&mut h, b"base").unwrap();
        let n = w.lookup(&mut h, b"x", 1, 2).unwrap() as usize;
        w.rows_mut(0).unwrap()[n] = Phoneme {
            mnemonic: mnemonic(b"x"),
            kind: 2,
            flags: LOCAL | 0x123456,
            program: 0xabcd,
            code: n as u8,
            ..Phoneme::default()
        };
        w.finish_table(&mut h).unwrap();
        h.base = b"base".to_vec();
        w.start(&mut h, b"child").unwrap();
        w.rows_mut(1).unwrap()[n].flags |= LOCAL;
        w.finish_table(&mut h).unwrap();
        h.fail_write = 5;
        assert_eq!(w.pack(&mut h), Err(5));
        assert_eq!(w.pack(&mut h), Err(22));
        assert_eq!(&h.bytes[..4], &[2, 0, 0, 0]);
        assert_eq!(&h.bytes[4..8], &[35, 0, 0, 0]);
        let second = 4 + 36 + 35 * 16;
        assert_eq!(&h.bytes[second..second + 4], &[2, 1, 0, 0]);
        let row: &[u8; 16] = h.bytes[second + 36..second + 52].try_into().unwrap();
        let row = Phoneme::from_record(row);
        assert_eq!(row.program, 0xabcd);
        assert_eq!(row.flags, 0x123456);
        assert_eq!(row.kind, 2);
        assert!(w
            .rows(0)
            .unwrap()
            .iter()
            .chain(w.rows(1).unwrap())
            .all(|r| r.flags & LOCAL == 0));
    }
    #[test]
    fn limits_callback_failure_and_full_table_terminator_reuse_retained_arenas() {
        let mut w = Workspace::default();
        let mut h = Capture::default();
        let mut old = None;
        let mut old_payload = None;
        for _ in 0..64 {
            w.clear();
            h.bytes.clear();
            w.start(&mut h, b"base").unwrap();
            let address = w.rows.as_ptr();
            if let Some(old) = old {
                assert_eq!(address, old);
            } else {
                old = Some(address);
            }
            h.fail_base = 5;
            let count = w.count();
            assert_eq!(w.start(&mut h, b"child"), Err(5));
            assert_eq!(w.count(), count);
            assert_eq!(w.tables().len(), 1);
            h.fail_base = 0;
            for i in 0..256 {
                let name = format!("{i:04}");
                if w.lookup(&mut h, name.as_bytes(), 1, i).unwrap() == -1 {
                    break;
                }
            }
            assert_eq!(w.count(), 255);
            w.terminate_unused().unwrap();
            assert_eq!(w.rows(0).unwrap().len(), 256);
            w.finish_table(&mut h).unwrap();
            w.pack(&mut h).unwrap();
            if let Some(pointer) = old_payload {
                assert_eq!(w.payload.as_ptr(), pointer);
            } else {
                old_payload = Some(w.payload.as_ptr());
            }
            assert_eq!(h.bytes[4], 0); /* 256 emitted records wrap the legacy byte count */
            assert!(h.bytes.len() <= MAX_PAYLOAD);
        }
        w.clear();
        w.start(&mut h, b"base").unwrap();
        w.finish_table(&mut h).unwrap();
        h.base = b"_".to_vec();
        for _ in 1..TABLES {
            w.start(&mut h, b"child").unwrap();
            w.finish_table(&mut h).unwrap();
        }
        let calls = h.calls;
        w.start(&mut h, b"extra").unwrap();
        assert_eq!(w.tables().len(), TABLES);
        assert_eq!(h.calls, calls);
        assert_eq!(h.diagnostics.last().unwrap().0, 5);
        assert_eq!(w.start(&mut h, &[b'x'; 256]), Err(22));
        w.pack(&mut h).unwrap();
        assert!(h.bytes.len() <= MAX_PAYLOAD);
    }
}
