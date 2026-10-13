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
_Static_assert(sizeof(RustGenerateEntry) == 36 && sizeof(FMT_PARAMS) == 48 &&
 sizeof(RustCommandState) == 48 && sizeof(RustCommandSettings) == 80 &&
 sizeof(RustCommandEffect) == 104 && sizeof(RustSpectPhoneme) == 20 &&
 sizeof(frameref_t) == 16, "native synthesis layout");
_Static_assert(N_PHONEME_LIST == 1000 && N_SEQ_FRAMES == 25 &&
 N_EMBEDDED_LIST == 250 && PITCHfall == 0 && LOPT_LONG_VOWEL_THRESHOLD == 14 &&
 LOPT_SONORANT_MIN == 5 && LOPT_WORD_MERGE == 6 && espeakEVENT_PHONEME == 7,
 "native synthesis constants");
