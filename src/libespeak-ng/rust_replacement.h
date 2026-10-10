/* Complete replacement expansion over serialized live owners.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_REPLACEMENT_H
#define ESPEAK_NG_RUST_REPLACEMENT_H
#include <stddef.h>
typedef struct {
    void *context;
    int (*value)(void *, unsigned);
    int (*store)(void *, unsigned, int);
    int (*classify)(void *, unsigned, unsigned);
    unsigned (*lower)(void *, unsigned);
    int (*main)(void *, char *, size_t, size_t, char *, unsigned *);
    int (*snapshot)(void *, char *);
    int (*publish)(void *, const char *);
} RustReplacement;
/* Callbacks/context required. Main's NULL source selects the original C word;
 * otherwise source is initialized owned 323-byte phrase scratch, with checked
 * initialized prefix and offset. Optional output is owned initialized 161-byte
 * replacement scratch. Main scopes/restores rule context and retains no source
 * or output pointer. Snapshot/publish use owned 200-byte pronunciation; publish
 * copies only terminated prefix and preserves untouched live output tails.
 * Initial main publishes into the retained caller text output and copies only
 * its initialized terminated prefix into owned scratch when textmode is set.
 * Caller output tails need not be initialized and are never read or rewritten.
 * Result slots are disjoint and aligned. Failure retains executed effects
 * and preserves result slot; skip scope is restored and C is never replayed. */
int espeak_rs_replacement(const RustReplacement *, unsigned, int, unsigned *);
int espeak_rs_split_ideographs(const RustReplacement *, unsigned, unsigned);
/* Segment copies terminated input (at most 161 bytes) into owned scratch and
 * publishes only a completed terminated result; untouched output tails remain.
 * Input/output are disjoint from callbacks/state and not changed by callbacks.
 * Capacity 1..320; -1 preserves output. No foreign buffer loan crosses calls. */
int espeak_rs_segment_replacement(const RustReplacement *, const char *, size_t, char *, size_t);
#endif
