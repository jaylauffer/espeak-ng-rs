/* Native emoji fallback over serialized source/replacement owners.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_EMOJI_FALLBACK_H
#define ESPEAK_NG_RUST_EMOJI_FALLBACK_H
#include <stddef.h>
typedef struct {
    void *context;
    int (*byte)(void *, size_t);
    int (*list)(void *, char *, char *, unsigned *, char *);
    int (*write)(void *, size_t, const char *, size_t);
    int (*tracing)(void *);
    int (*snapshot)(void *, char *);
    void (*trace)(void *, const char *, const char *);
    int (*finish)(void *);
} RustEmojiFallback;
/* All callbacks/context required, no pointer retained. Byte resolves original
 * live source at a numeric offset. List receives disjoint initialized 162-byte
 * input, 200-byte pronunciation, two flags and 162-byte copied selected text.
 * In textmode it copies a complete terminated admitted result before return.
 * Write updates only checked ranges in the initialized 160-byte live owner;
 * other nested effects and untouched tails remain. Snapshot copies that owner.
 * Trace receives terminated owned 160-byte source/replacement (text at +2).
 * Finish publishes the retained owner pointer. Flags are aligned and disjoint
 * from table/scratch; no flag loan crosses a callback. Return 1 ORs textmode
 * into fresh flags; 0 leaves flags alone. -1 preserves the flag slot while
 * retaining executed effects, including writes; failure never replays C. */
int espeak_rs_lookup_emoji(const RustEmojiFallback *, unsigned *);
#endif
