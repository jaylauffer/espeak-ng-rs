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
typedef struct {
    void *tr;
    const char *source;
    size_t length;
    char **wordptr;
    int end_flags;
    void *words;
    int remaining;
} RustListHost;
typedef struct {
    int *skipwords, *say_as, *trace;
    const char *word_phonemes;
} RustListGlobals;
typedef struct {
    const char *(*lookup)(void *,const char *,const char *,char *,unsigned *);
    void (*accent)(void *,unsigned,size_t,char *);
    int (*translate)(void *,char *);
    void (*trace)(void *,const char *,const char *);
} RustListEffects;
/* Live serialized typed owner, initialized metadata/globals/effects and word
 * slot; source is terminated or contained in the retained rule-source extent.
 * Word rows obey the dictionary lookup contract. Global pronunciation has 200
 * initialized readable bytes and survives child translation. Effects borrow a
 * Rust-owned readonly RustListHost prefix; never retain/reenter metadata or
 * owned scratch, nor mutate metadata. Lookup/accent fill owned pronunciation
 * and flags; translation borrows mutable owned text. Translation can nest with
 * scoped owner source, restored on
 * return. No foreign source/owner/output reference spans an effect. All effects
 * and globals are required. Native list/symbol clear only output[0] on admitted
 * failure and preserve executed effects; early null/zero-capacity leaves output
 * untouched. Prefix uses terminated source (no rule extent), clears two flags
 * on failure and returns the signed first-flag pattern. Never replay failure. */
int espeak_rs_lookup_list_host(const RustListHost *,const RustListGlobals *,
    const RustListEffects *,unsigned *,char *,size_t);
int espeak_rs_lookup_symbol_host(const RustListHost *,const RustListGlobals *,
    const RustListEffects *,char *,size_t);
int espeak_rs_lookup_flags_host(const RustListHost *,const RustListGlobals *,
    const RustListEffects *,unsigned *);
#endif
