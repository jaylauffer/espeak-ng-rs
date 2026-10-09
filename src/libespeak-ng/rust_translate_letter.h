/* Complete isolated-letter control over serialized engine primitives.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_TRANSLATE_LETTER_H
#define ESPEAK_NG_RUST_TRANSLATE_LETTER_H
#include <stddef.h>
#include <stdint.h>
typedef struct {
    void *context;
    int (*value)(void *, unsigned);
    int (*classify)(void *, unsigned, unsigned);
    int (*named)(void *, unsigned, const char *, size_t, char *, int *);
    int (*letter)(void *, unsigned, unsigned, int, unsigned, size_t, char *);
    int (*secondary)(void *, const char *);
    void (*restore)(void *);
    int (*hangul)(void *, char *, char *);
    int (*encode)(void *, const char *, char *);
    int (*publish)(void *, unsigned, const char *);
} RustTranslateLetter;
/* Required callbacks and serialized live owners. Native UTF-8 decoding supplies
 * code, following signed byte, consumed bytes (1..4) and current alphabet range
 * start (UINT32_MAX absent). Named/letter borrow initialized owned 200-byte
 * scratch but write only the actual supplied extent. Named flags are disjoint.
 * Keys and secondary names are terminated and retained only through return.
 * Hangul owns 12 padded bytes: scope/restore source context and write at most 77
 * pronunciation bytes, then stress. Publish uses fresh output after nested
 * effects: replace switch prefix or append only when the whole prefix fits.
 * Result is disjoint from all callback owners/table/publications. -1 preserves
 * result but retains executed effects; never replay rejected control. */
int espeak_rs_translate_letter(const RustTranslateLetter *, unsigned, int,
    unsigned, unsigned, int, int *);
#endif
