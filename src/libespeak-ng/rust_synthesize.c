/* Native synthesis-host ABI/configuration declarations; no engine logic. */
/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "synthesize.h"
#include "rust_data.h"
#include "mbrola.h"
#include "voice.h"
#include "translate.h"
#include <stddef.h>
int (*const espeak_rs_synthesis_mbrola)(PHONEME_LIST *, int *, bool) =
#if USE_MBROLA
 MbrolaGenerate;
#else
 NULL;
#endif
/* Sizes that depend on pointer width are given as (64-bit : 32-bit). */
_Static_assert(sizeof(RustGenerateEntry) == 36 && sizeof(FMT_PARAMS) == 48 &&
 sizeof(RustCommandState) == (sizeof(void *) == 8 ? 48 : 44) && sizeof(RustCommandSettings) == (sizeof(void *) == 8 ? 80 : 60) &&
 sizeof(RustCommandEffect) == (sizeof(void *) == 8 ? 104 : 64) && sizeof(RustSpectPhoneme) == 20 &&
 sizeof(frameref_t) == (sizeof(void *) == 8 ? 16 : 8), "native synthesis layout");
_Static_assert(N_PHONEME_LIST == 1000 && N_SEQ_FRAMES == 25 &&
 N_EMBEDDED_LIST == 250 && PITCHfall == 0 && LOPT_LONG_VOWEL_THRESHOLD == 14 &&
 LOPT_SONORANT_MIN == 5 && LOPT_WORD_MERGE == 6 && espeakEVENT_PHONEME == 7,
 "native synthesis constants");
