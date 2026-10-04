/* Native Rust migration boundary. Offsets refer to validated resident bytes.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_DATA_H
#define ESPEAK_RUST_DATA_H
#include <stddef.h>
#include <stdint.h>
#include "phoneme.h"
#include "synthesize.h"

/* Borrow little-endian phonindex and an immutable phoneme for this call;
 * output is exclusive and disjoint from all callback state. Callback kinds:
 * 0 condition at word offset, 1 stress, 2 next vowel, 3 next start type,
 * 4 previous end type, 5 invalid instruction. Negative type means missing. */
int espeak_rs_phoneme_program(const unsigned char *, size_t, const PHONEME_TAB *, uint32_t, uint32_t,
    void *, int (*)(void *, uint32_t, size_t), PHONEME_DATA *);

typedef struct {
	size_t singles[256], offsets[128], pairs[120];
	uint32_t pair_names[120];
	size_t pair_count;
	unsigned char pair_starts[256], pair_counts[256];
	size_t letters[95], replacements;
} RustRuleIndex;

typedef struct {
	char name[32];
	size_t records_offset;
	uint32_t count, includes;
} RustTableMeta;

/* Inputs must remain readable for the duration of the call. Output arrays are
 * exclusive and sized exactly as declared. Indices own metadata, not bytes. */
int espeak_rs_dictionary_index(const unsigned char *, size_t, RustRuleIndex *, size_t [1024], size_t *);
void *espeak_rs_phontab_create(const unsigned char *, size_t, RustTableMeta [150], int *);
void espeak_rs_phontab_destroy(void *);
int espeak_rs_phontab_select(const void *, const unsigned char *, size_t, int, size_t [256]);
int espeak_rs_phontab_lookup(const void *, const char *);
int espeak_rs_sample_rate(const unsigned char *, size_t);
int espeak_rs_phondata_header(const unsigned char *, size_t, uint32_t [2]);
typedef struct {
	uint32_t conditions, end_flags, word_flags, lookup_symbol, language, previous_flags;
	int32_t expect_verb, expect_verb_s, expect_past, expect_noun;
	uint32_t native_translator, sentence, single_symbol;
	size_t clause_remaining;
} RustLookupContext;
typedef struct { uint32_t flags, length; } RustWordInfo;
typedef struct {
	size_t phonemes_offset, phonemes_length, word_end;
	uint32_t flags[2], trace_flags[2], copied, has_flags, found;
	int32_t skipwords;
} RustLookupOutcome;
int espeak_rs_transpose(unsigned char *, size_t, uint32_t, uint32_t, const unsigned char *, size_t, const int16_t *, size_t);
int espeak_rs_lookup_bucket(const unsigned char *, size_t, const char *, size_t, const char *, size_t, const RustLookupContext *, const RustWordInfo *, size_t, RustLookupOutcome *);
typedef struct {
	uint32_t conditions, word_flags, dictionary_flags;
	int32_t vowel_count, stressed_count, expect_verb, tone_numbers, suffix_options;
	uint32_t trace, word_start, signed_bytes;
} RustMatchContext;
typedef struct {
	size_t phonemes, delete_offset, advance;
	int32_t points, ending;
} RustRuleMatch;
typedef struct {
	const unsigned char *bits;
	const void *const *groups;
	const size_t *lengths;
	int32_t offset;
	uint32_t wide_bytes;
} RustLetters;
/* Borrow 256 bitfield bytes, eight group pointers and eight cached lengths.
 * Non-null lists contain length wchar_t units (2 or 4 bytes), excluding NUL.
 * All configuration stays immutable during the call and its callbacks. */
int espeak_rs_is_letter(const RustLetters *, int32_t, uint32_t);
int espeak_rs_letter_group(const unsigned char *, size_t, const unsigned char *, size_t, size_t, int);
int espeak_rs_match_group(const unsigned char *, size_t, const unsigned char *, size_t, size_t, size_t,
    const RustMatchContext *, const RustLetters *, void *,
    int (*)(void *, uint32_t, uint32_t, size_t, uint32_t),
    void (*)(void *, const unsigned char *, size_t, uint32_t [2]),
    void (*)(void *, size_t, size_t, int32_t), RustRuleMatch *);
#endif
