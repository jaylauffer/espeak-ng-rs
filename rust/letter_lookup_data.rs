// Letter/diacritic resources transcribed from the retained numbers.c tables.
// Copyright (C) 2005-2015 Jonathan Duddington; 2015-2016, 2020 Reece H. Dunn.
// SPDX-License-Identifier: GPL-3.0-or-later
const M_LIGATURE: u16 = 0x8000;
const M_NAME: u16 = 0;
const M_SMALLCAP: u16 = 1;
const M_TURNED: u16 = 2;
const M_REVERSED: u16 = 3;
const M_CURL: u16 = 4;
const M_ACUTE: u16 = 5;
const M_BREVE: u16 = 6;
const M_CARON: u16 = 7;
const M_CEDILLA: u16 = 8;
const M_CIRCUMFLEX: u16 = 9;
const M_DIAERESIS: u16 = 10;
const M_DOUBLE_ACUTE: u16 = 11;
const M_DOT_ABOVE: u16 = 12;
const M_GRAVE: u16 = 13;
const M_MACRON: u16 = 14;
const M_OGONEK: u16 = 15;
const M_RING: u16 = 16;
const M_STROKE: u16 = 17;
const M_TILDE: u16 = 18;
const M_BAR: u16 = 19;
const M_RETROFLEX: u16 = 20;
const M_HOOK: u16 = 21;
const M_MIDDLE_DOT: u16 = M_DOT_ABOVE;
const M_IMPLOSIVE: u16 = M_HOOK;
const CAPITAL: u16 = 0;
const L_ALPHA: u16 = 60;
const L_SCHWA: u16 = 61;
const L_OPEN_E: u16 = 62;
const L_GAMMA: u16 = 63;
const L_IOTA: u16 = 64;
const L_PHI: u16 = 67;
const L_ESH: u16 = 68;
const L_UPSILON: u16 = 69;
const L_EZH: u16 = 70;
const L_GLOTTAL: u16 = 71;
const L_RTAP: u16 = 72;
const L_RLONG: u16 = 73;
const fn letter(ch: u16, m1: u16, m2: u16) -> u16 {
    (ch - 59) + (m1 << 6) + (m2 << 11)
}
const fn ligature(a: u16, b: u16, m: u16) -> u16 {
    (a - 59) + ((b - 59) << 6) + (m << 12) + M_LIGATURE
}
pub(super) const NON_ASCII: &[u16] = &[
    0, 0x3b1, 0x259, 0x25b, 0x3b3, 0x3b9, 0x153, 0x3c9, 0x3c6, 0x283, 0x3c5, 0x292, 0x294, 0x27e,
    0x27c,
];
pub(super) const LATIN: &[u16] = &[
    letter(b'a' as u16, M_GRAVE, 0), // U+00e0
    letter(b'a' as u16, M_ACUTE, 0),
    letter(b'a' as u16, M_CIRCUMFLEX, 0),
    letter(b'a' as u16, M_TILDE, 0),
    letter(b'a' as u16, M_DIAERESIS, 0),
    letter(b'a' as u16, M_RING, 0),
    ligature(b'a' as u16, b'e' as u16, 0),
    letter(b'c' as u16, M_CEDILLA, 0),
    letter(b'e' as u16, M_GRAVE, 0),
    letter(b'e' as u16, M_ACUTE, 0),
    letter(b'e' as u16, M_CIRCUMFLEX, 0),
    letter(b'e' as u16, M_DIAERESIS, 0),
    letter(b'i' as u16, M_GRAVE, 0),
    letter(b'i' as u16, M_ACUTE, 0),
    letter(b'i' as u16, M_CIRCUMFLEX, 0),
    letter(b'i' as u16, M_DIAERESIS, 0),
    letter(b'd' as u16, M_NAME, 0), // eth U+00f0
    letter(b'n' as u16, M_TILDE, 0),
    letter(b'o' as u16, M_GRAVE, 0),
    letter(b'o' as u16, M_ACUTE, 0),
    letter(b'o' as u16, M_CIRCUMFLEX, 0),
    letter(b'o' as u16, M_TILDE, 0),
    letter(b'o' as u16, M_DIAERESIS, 0),
    0, // division sign
    letter(b'o' as u16, M_STROKE, 0),
    letter(b'u' as u16, M_GRAVE, 0),
    letter(b'u' as u16, M_ACUTE, 0),
    letter(b'u' as u16, M_CIRCUMFLEX, 0),
    letter(b'u' as u16, M_DIAERESIS, 0),
    letter(b'y' as u16, M_ACUTE, 0),
    letter(b't' as u16, M_NAME, 0), // thorn
    letter(b'y' as u16, M_DIAERESIS, 0),
    CAPITAL, // U+0100
    letter(b'a' as u16, M_MACRON, 0),
    CAPITAL,
    letter(b'a' as u16, M_BREVE, 0),
    CAPITAL,
    letter(b'a' as u16, M_OGONEK, 0),
    CAPITAL,
    letter(b'c' as u16, M_ACUTE, 0),
    CAPITAL,
    letter(b'c' as u16, M_CIRCUMFLEX, 0),
    CAPITAL,
    letter(b'c' as u16, M_DOT_ABOVE, 0),
    CAPITAL,
    letter(b'c' as u16, M_CARON, 0),
    CAPITAL,
    letter(b'd' as u16, M_CARON, 0),
    CAPITAL, // U+0110
    letter(b'd' as u16, M_STROKE, 0),
    CAPITAL,
    letter(b'e' as u16, M_MACRON, 0),
    CAPITAL,
    letter(b'e' as u16, M_BREVE, 0),
    CAPITAL,
    letter(b'e' as u16, M_DOT_ABOVE, 0),
    CAPITAL,
    letter(b'e' as u16, M_OGONEK, 0),
    CAPITAL,
    letter(b'e' as u16, M_CARON, 0),
    CAPITAL,
    letter(b'g' as u16, M_CIRCUMFLEX, 0),
    CAPITAL,
    letter(b'g' as u16, M_BREVE, 0),
    CAPITAL, // U+0120
    letter(b'g' as u16, M_DOT_ABOVE, 0),
    CAPITAL,
    letter(b'g' as u16, M_CEDILLA, 0),
    CAPITAL,
    letter(b'h' as u16, M_CIRCUMFLEX, 0),
    CAPITAL,
    letter(b'h' as u16, M_STROKE, 0),
    CAPITAL,
    letter(b'i' as u16, M_TILDE, 0),
    CAPITAL,
    letter(b'i' as u16, M_MACRON, 0),
    CAPITAL,
    letter(b'i' as u16, M_BREVE, 0),
    CAPITAL,
    letter(b'i' as u16, M_OGONEK, 0),
    CAPITAL,                        // U+0130
    letter(b'i' as u16, M_NAME, 0), // dotless i
    CAPITAL,
    ligature(b'i' as u16, b'j' as u16, 0),
    CAPITAL,
    letter(b'j' as u16, M_CIRCUMFLEX, 0),
    CAPITAL,
    letter(b'k' as u16, M_CEDILLA, 0),
    letter(b'k' as u16, M_NAME, 0), // kra
    CAPITAL,
    letter(b'l' as u16, M_ACUTE, 0),
    CAPITAL,
    letter(b'l' as u16, M_CEDILLA, 0),
    CAPITAL,
    letter(b'l' as u16, M_CARON, 0),
    CAPITAL,
    letter(b'l' as u16, M_MIDDLE_DOT, 0), // U+0140
    CAPITAL,
    letter(b'l' as u16, M_STROKE, 0),
    CAPITAL,
    letter(b'n' as u16, M_ACUTE, 0),
    CAPITAL,
    letter(b'n' as u16, M_CEDILLA, 0),
    CAPITAL,
    letter(b'n' as u16, M_CARON, 0),
    letter(b'n' as u16, M_NAME, 0), // apostrophe n
    CAPITAL,
    letter(b'n' as u16, M_NAME, 0), // eng
    CAPITAL,
    letter(b'o' as u16, M_MACRON, 0),
    CAPITAL,
    letter(b'o' as u16, M_BREVE, 0),
    CAPITAL, // U+0150
    letter(b'o' as u16, M_DOUBLE_ACUTE, 0),
    CAPITAL,
    ligature(b'o' as u16, b'e' as u16, 0),
    CAPITAL,
    letter(b'r' as u16, M_ACUTE, 0),
    CAPITAL,
    letter(b'r' as u16, M_CEDILLA, 0),
    CAPITAL,
    letter(b'r' as u16, M_CARON, 0),
    CAPITAL,
    letter(b's' as u16, M_ACUTE, 0),
    CAPITAL,
    letter(b's' as u16, M_CIRCUMFLEX, 0),
    CAPITAL,
    letter(b's' as u16, M_CEDILLA, 0),
    CAPITAL, // U+0160
    letter(b's' as u16, M_CARON, 0),
    CAPITAL,
    letter(b't' as u16, M_CEDILLA, 0),
    CAPITAL,
    letter(b't' as u16, M_CARON, 0),
    CAPITAL,
    letter(b't' as u16, M_STROKE, 0),
    CAPITAL,
    letter(b'u' as u16, M_TILDE, 0),
    CAPITAL,
    letter(b'u' as u16, M_MACRON, 0),
    CAPITAL,
    letter(b'u' as u16, M_BREVE, 0),
    CAPITAL,
    letter(b'u' as u16, M_RING, 0),
    CAPITAL, // U+0170
    letter(b'u' as u16, M_DOUBLE_ACUTE, 0),
    CAPITAL,
    letter(b'u' as u16, M_OGONEK, 0),
    CAPITAL,
    letter(b'w' as u16, M_CIRCUMFLEX, 0),
    CAPITAL,
    letter(b'y' as u16, M_CIRCUMFLEX, 0),
    CAPITAL, // Y-DIAERESIS
    CAPITAL,
    letter(b'z' as u16, M_ACUTE, 0),
    CAPITAL,
    letter(b'z' as u16, M_DOT_ABOVE, 0),
    CAPITAL,
    letter(b'z' as u16, M_CARON, 0),
    letter(b's' as u16, M_NAME, 0), // long-s U+17f
];
pub(super) const IPA: &[u16] = &[
    letter(b'a' as u16, M_TURNED, 0), // U+250
    letter(L_ALPHA, 0, 0),
    letter(L_ALPHA, M_TURNED, 0),
    letter(b'b' as u16, M_IMPLOSIVE, 0),
    0, // open-o
    letter(b'c' as u16, M_CURL, 0),
    letter(b'd' as u16, M_RETROFLEX, 0),
    letter(b'd' as u16, M_IMPLOSIVE, 0),
    letter(b'e' as u16, M_REVERSED, 0), // U+258
    0,                                  // schwa
    letter(L_SCHWA, M_HOOK, 0),
    0, // open-e
    letter(L_OPEN_E, M_REVERSED, 0),
    letter(L_OPEN_E, M_HOOK, M_REVERSED),
    0,
    letter(b'j' as u16, M_BAR, 0),
    letter(b'g' as u16, M_IMPLOSIVE, 0), // U+260
    letter(b'g' as u16, 0, 0),
    letter(b'g' as u16, M_SMALLCAP, 0),
    letter(L_GAMMA, 0, 0),
    0, // ramshorn
    letter(b'h' as u16, M_TURNED, 0),
    letter(b'h' as u16, M_HOOK, 0),
    0,
    letter(b'i' as u16, M_BAR, 0), // U+268
    letter(L_IOTA, 0, 0),
    letter(b'i' as u16, M_SMALLCAP, 0),
    letter(b'l' as u16, M_TILDE, 0),
    letter(b'l' as u16, M_BAR, 0),
    letter(b'l' as u16, M_RETROFLEX, 0),
    ligature(b'l' as u16, b'z' as u16, 0),
    letter(b'm' as u16, M_TURNED, 0),
    0,
    letter(b'm' as u16, M_HOOK, 0),
    0,
    letter(b'n' as u16, M_RETROFLEX, 0),
    letter(b'n' as u16, M_SMALLCAP, 0),
    letter(b'o' as u16, M_BAR, 0),
    ligature(b'o' as u16, b'e' as u16, M_SMALLCAP),
    0,
    letter(L_PHI, 0, 0), // U+278
    letter(b'r' as u16, M_TURNED, 0),
    letter(L_RLONG, M_TURNED, 0),
    letter(b'r' as u16, M_RETROFLEX, M_TURNED),
    0,
    letter(b'r' as u16, M_RETROFLEX, 0),
    0, // r-tap
    letter(L_RTAP, M_REVERSED, 0),
    letter(b'r' as u16, M_SMALLCAP, 0), // U+280
    letter(b'r' as u16, M_TURNED, M_SMALLCAP),
    letter(b's' as u16, M_RETROFLEX, 0),
    0, // esh
    letter(b'j' as u16, M_HOOK, 0),
    letter(L_ESH, M_REVERSED, 0),
    letter(L_ESH, M_CURL, 0),
    letter(b't' as u16, M_TURNED, 0),
    letter(b't' as u16, M_RETROFLEX, 0), // U+288
    letter(b'u' as u16, M_BAR, 0),
    letter(L_UPSILON, 0, 0),
    letter(b'v' as u16, M_HOOK, 0),
    letter(b'v' as u16, M_TURNED, 0),
    letter(b'w' as u16, M_TURNED, 0),
    letter(b'y' as u16, M_TURNED, 0),
    letter(b'y' as u16, M_SMALLCAP, 0),
    letter(b'z' as u16, M_RETROFLEX, 0), // U+290
    letter(b'z' as u16, M_CURL, 0),
    0, // ezh
    letter(L_EZH, M_CURL, 0),
    0, // glottal stop
    letter(L_GLOTTAL, M_REVERSED, 0),
    letter(L_GLOTTAL, M_TURNED, 0),
    0,
    0, // bilabial click U+298
    letter(b'b' as u16, M_SMALLCAP, 0),
    0,
    letter(b'g' as u16, M_IMPLOSIVE, M_SMALLCAP),
    letter(b'h' as u16, M_SMALLCAP, 0),
    letter(b'j' as u16, M_CURL, 0),
    letter(b'k' as u16, M_TURNED, 0),
    letter(b'l' as u16, M_SMALLCAP, 0),
    letter(b'q' as u16, M_HOOK, 0), // U+2a0
    letter(L_GLOTTAL, M_STROKE, 0),
    letter(L_GLOTTAL, M_STROKE, M_REVERSED),
    ligature(b'd' as u16, b'z' as u16, 0),
    0, // dezh
    ligature(b'd' as u16, b'z' as u16, M_CURL),
    ligature(b't' as u16, b's' as u16, 0),
    0, // tesh
    ligature(b't' as u16, b's' as u16, M_CURL),
];
