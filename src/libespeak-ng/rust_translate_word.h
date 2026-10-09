/* Complete native word control over serialized live source/state/output owners.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_TRANSLATE_WORD_H
#define ESPEAK_NG_RUST_TRANSLATE_WORD_H
#include <stddef.h>
#include <stdint.h>
typedef struct { unsigned slot; intptr_t offset; } RustWordSource;
typedef struct {
    void *context;
    int (*byte)(void *, RustWordSource);
    int (*write)(void *, RustWordSource, unsigned char);
    int (*value)(void *, unsigned, unsigned);
    void (*store)(void *, unsigned, int);
    int (*locale)(void *, unsigned, unsigned);
    int (*list)(void *, RustWordSource *, char *, unsigned *, int, int *);
    int (*emoji)(void *, RustWordSource *, unsigned *);
    int (*text)(void *, RustWordSource);
    int (*dotted)(void *, RustWordSource, int *);
    int (*number_language)(void *);
    int (*number)(void *, unsigned, RustWordSource, char *, unsigned *, int *);
    int (*spell)(void *, RustWordSource *, char *, int, int *);
    int (*letter)(void *, RustWordSource, char *, unsigned, size_t *);
    int (*unpronounceable)(void *, RustWordSource, int, int *);
    int (*spelling_stress)(void *, char *, int);
    int (*rules)(void *, RustWordSource, char *, char *, unsigned, unsigned *, int *);
    int (*remove)(void *, RustWordSource, int, char *, int *);
    int (*prefix)(void *, const char *, RustWordSource *);
    void (*trace_suffix)(void *, const char *);
    int (*append)(void *, char *, const char *);
    int (*plural)(void *, unsigned, unsigned);
    int (*stress)(void *, char *, unsigned *, int, int);
    int (*snapshot)(void *, char *);
    int (*publish)(void *, const char *, unsigned);
    int (*change_stress)(void *, int);
    int (*special)(void *, unsigned);
} RustTranslateWord;
/* Every callback/context required. Source 0 starts at the original word;
 * byte/write admit offsets including initialized predecessors and lookahead.
 * Replacements and copied 65-byte prefix storage receive numeric live-owner
 * identities; returned pointers/cursors must be admitted before callback return.
 * Pronunciation/ending arguments are owned initialized disjoint 200-byte
 * scratch, flags are two owned u32s, suffix copy is optional initialized 160
 * bytes. Nullable ending disables ending output; nullable stress pronunciation
 * selects fresh shared word output. Primitives retain no Rust pointer loan.
 * Word output is 200 live bytes; optional replacement output holds 161 bytes.
 * Snapshot copies only initialized prefix; joined publication also clears byte
 * 199. Returned flags are disjoint from owners/table/source. -1 preserves flags
 * but retains executed source/state/output effects; never replay failure. */
int espeak_rs_translate_word(const RustTranslateWord *, unsigned *);
#endif
