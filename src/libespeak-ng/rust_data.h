/* Native Rust migration boundary. Offsets refer to validated resident bytes.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_DATA_H
#define ESPEAK_RUST_DATA_H
#include <stddef.h>
#include <stdint.h>

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
#endif
