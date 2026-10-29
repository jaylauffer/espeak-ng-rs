/* Letter/symbol and accent pronunciation via serialized dictionary primitives.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_LETTER_LOOKUP_H
#define ESPEAK_NG_RUST_LETTER_LOOKUP_H
#include <stddef.h>
typedef struct {
    void *context;
    int (*lookup)(void *, char *, size_t, unsigned, char *);
    int (*named)(void *, const char *, char *);
    int (*value)(void *, unsigned);
    int (*space)(void *, unsigned);
    void (*rules)(void *, char *, size_t, size_t, unsigned, char *);
    void (*select)(void *, unsigned);
    void (*stress)(void *, char *, unsigned *, int);
} RustLetterLookup;
/* All fields required, immutable table and live serialized context disjoint from
 * output. Source is owned initialized 10-byte storage; callbacks retain no source
 * or scratch pointer and rule translation scopes/restores its source context.
 * Lookup/named/rules/stress scratch is initialized 200 bytes; rule scratch carries
 * the dictionary pronunciation prefix and supports append/no-op; stress flags two
 * initialized unsigneds. Capacity is actual output extent 1..200. Accent mode
 * preserves output when no pronunciation is assembled. -1 preserves output,
 * with already executed dictionary/source/state effects retained, never replayed.
 * 0 leaves accent output unchanged; 1 publishes only the terminated prefix. */
int espeak_rs_lookup_letter(const RustLetterLookup *, unsigned, int, int, int, char *, size_t);
typedef struct {
    void **secondary, **voice;
} RustLetterGlobals;
typedef struct {
    int (*lookup)(void *,const char *,char *);
    int (*space)(unsigned);
    void (*rules)(void *,char *,size_t,unsigned,char *);
    void (*setup)(void);
    void (*restore)(int);
    void (*stress)(void *,char *,unsigned *,int);
} RustLetterEffects;
/* Serialized live typed translator and initialized immutable tables/global
 * pointer slots. Secondary can be null; voice retains a valid acoustic snapshot
 * on restoration. Lookup names are readonly terminated keys; other lookup/rule
 * source is mutable owned initialized 10-byte storage. Effects borrow source,
 * 200-byte pronunciation/two flags through return; retain/reenter no scratch,
 * never free owner or slots. Rule source metadata is scoped/restored by Rust,
 * including nested child changes. No foreign owner/output reference spans an
 * effect. All effects required. Same -1/0/1 atomic output semantics as generic
 * ABI; invalid/missing voice on restoration fails with executed effects retained
 * and no replay. Output's capacity is its actual writable extent 1..200. */
/* LookupLetterBounded (accent 0) and LookupAccentedLetterBounded (accent 1)
 * with the engine's primitives. */
int espeak_rs_lookup_letter_engine(void *, unsigned, int, int, int, char *, size_t);
int espeak_rs_lookup_letter_host(void *,const RustLetterGlobals *,const RustLetterEffects *,
    unsigned,int,int,int,char *,size_t);
#endif
