/* Complete native rule translation over retained serialized primitives.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_TRANSLATE_RULES_H
#define ESPEAK_NG_RUST_TRANSLATE_RULES_H
#include <stddef.h>
#include <stdint.h>
#define RUST_RULES_NO_DELETE INTPTR_MIN
typedef struct {
    int32_t points, ending;
    size_t cursor;
    intptr_t delete_offset;
    char phonemes[200];
} RustRulesMatch;
typedef struct {
    void *context;
    int (*byte)(void *, intptr_t);
    int (*write)(void *, intptr_t, unsigned char);
    int (*value)(void *, unsigned, unsigned);
    void (*store)(void *, unsigned, int);
    int (*locale)(void *, unsigned, unsigned);
    size_t (*group)(void *, unsigned, unsigned);
    int (*matched)(void *, size_t, size_t, unsigned, unsigned, RustRulesMatch *);
    int (*symbol)(void *, const char *, char *);
    int (*letter)(void *, unsigned, char *);
    int (*publish)(void *, unsigned, const char *);
    int (*has_ending)(void *);
    int (*append)(void *, const char *);
    void (*trace)(void *, unsigned, const char *);
} RustTranslateRules;
/* Every callback/context required; source/translator/output owners retained and
 * serialized. Byte/write check numeric offsets, including the preceding byte.
 * Group: dictionary offset, SIZE_MAX absent, SIZE_MAX-1 invalid. Matched copies
 * pronunciation into initialized owned 200-byte scratch, advances cursor and
 * converts deletion to a checked numeric offset; never retain its loan. Symbol
 * uses an initialized 8-byte terminated key and at most 40 pronunciation bytes;
 * letter uses at most 160; scratch is always 200 bytes. Publish copies only the
 * validated prefix to actual owner capacities. Append is the native phoneme-word
 * leaf over fresh output/count/table state, including nested shared-output
 * effects. Trace borrows 120 initialized bytes. No foreign source/state loan
 * survives a nested match/translation call. Result is disjoint from all owners,
 * source and table. -1 preserves result with executed primitive/source/output
 * effects retained; 0 publishes signed ending/status bits. Never replay failure. */
int espeak_rs_translate_rules(const RustTranslateRules *, unsigned, int *);
#endif
