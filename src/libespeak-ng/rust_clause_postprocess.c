/* Compile-time C ABI/configuration declarations; no engine logic. */
/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "synthesize.h"
#include <stddef.h>
const int espeak_rs_postprocess_features[3] = { USE_KLATT, USE_MBROLA, USE_LIBSONIC };
#if USE_MBROLA
extern char mbrola_name[20];
const unsigned char * const espeak_rs_postprocess_mbrola = (const unsigned char *)mbrola_name;
#else
const unsigned char * const espeak_rs_postprocess_mbrola = NULL;
#endif
_Static_assert(sizeof(PHONEME_LIST) == (sizeof(void *) == 8 ? 48 : 36) &&
  offsetof(PHONEME_LIST, ph) == 8 &&
  offsetof(PHONEME_LIST, tone_ph_data) == (sizeof(void *) == 8 ? 16 : 12) &&
  offsetof(PHONEME_LIST, length) == (sizeof(void *) == 8 ? 24 : 16) &&
  offsetof(PHONEME_LIST, phontab_addr) == (sizeof(void *) == 8 ? 36 : 28), "native clause row layout");
_Static_assert(sizeof(PHONEME_LIST2) == 8 && sizeof(REPLACE_PHONEMES) == 3,
  "native source/replacement layout");
