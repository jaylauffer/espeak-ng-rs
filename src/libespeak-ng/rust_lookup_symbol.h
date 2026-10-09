/* Native symbol lookup, translated names and bounded pronunciation.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_LOOKUP_SYMBOL_H
#define ESPEAK_NG_RUST_LOOKUP_SYMBOL_H
#include "rust_lookup_list.h"
typedef struct {
    RustLookupList list;
    int (*byte)(void *, size_t);
    int (*say_as)(void *);
    void (*set_say_as)(void *, int);
    int (*translate)(void *, char *, char *);
} RustLookupSymbol;
/* Every callback required; uses list.context. List query supplies original
 * word, FLAG_ALLOW_TEXTMODE and no word rows. Byte projects the current original
 * or replacement string; reject outside admitted source. Translate receives
 * initialized 80-byte text (three-byte prefix, NUL) and 200-byte scratch, scopes/
 * restores rule context and retains no loans. Output/result/table/owner/source
 * disjoint, capacity actual writable extent 1..200. Status -1 preserves output/
 * result with executed primitive effects retained; 0 publishes initialized
 * terminated prefix and signed flags, which may be zero despite pronunciation. */
int espeak_rs_lookup_symbol(const RustLookupSymbol *, char *, size_t, int *);
/* Ordinary context/end flags and no word rows for rule-prefix queries. Output
 * is two exclusive unsigneds; -1 preserves it with primitive effects retained;
 * 0 publishes flags regardless of list found/textmode result. */
int espeak_rs_lookup_flags(const RustLookupList *, unsigned *);
#endif
