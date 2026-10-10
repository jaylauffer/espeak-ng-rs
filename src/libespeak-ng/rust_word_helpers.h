/* Native spelling/plural control over serialized owner effects.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_WORD_HELPERS_H
#define ESPEAK_NG_RUST_WORD_HELPERS_H
#include "rust_translate_word.h"
typedef struct {
    void *context;
    int (*byte)(void *, RustWordSource);
    int (*letter)(void *, RustWordSource, unsigned, size_t *);
    unsigned char (*leading)(void *);
    int (*copy_switch)(void *);
    int (*stress)(void *, int, int);
    int (*plural)(void *, char *);
} RustWordHelpers;
/* All callbacks/context required; no pointer retained. Source zero is original
 * word in admitted initialized live storage. Pronunciation/output owners each
 * admit 200 bytes (may alias); effects use current output after nested calls.
 * Plural receives owned initialized five-byte text, including predecessor and
 * NUL. The owner scopes/restores its rule context around that primitive.
 * Cursor/switched are aligned and disjoint from each other and all owners.
 * Spelling switch preserves cursor and publishes switched=1; otherwise returns
 * ending cursor and switched=0. -1 preserves result slots and executed effects;
 * failure never replays C control. */
int espeak_rs_word_spell(const RustWordHelpers *, int, RustWordSource *, int *);
int espeak_rs_word_plural(const RustWordHelpers *, unsigned, unsigned char);
#endif
