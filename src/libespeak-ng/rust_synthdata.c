/* Native resource-host ABI/configuration declarations; no engine logic. */
/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "speech.h"
#include "synthdata.h"
#include "rust_phoneme_assets.h"
#include <stddef.h>
const size_t espeak_rs_synthdata_path_width = N_PATH_BUF;
const int espeak_rs_synthdata_path_separator = PATHSEP;
_Static_assert(sizeof(RustPhonemeTableView) == sizeof(PHONEME_TAB_LIST) &&
 offsetof(RustPhonemeTableView, records) == offsetof(PHONEME_TAB_LIST, phoneme_tab_ptr) &&
 offsetof(RustPhonemeTableView, count) == offsetof(PHONEME_TAB_LIST, n_phonemes) &&
 offsetof(RustPhonemeTableView, includes) == offsetof(PHONEME_TAB_LIST, includes), "native resident table layout");
_Static_assert(sizeof(WORD_PH_DATA) == sizeof(PHONEME_LIST), "native previous vowel layout");
_Static_assert(phonPAUSE == 9 && phonLENGTHEN == 12 && phVOWEL == 2 &&
 i_SET_LENGTH == 10 && i_LENGTH_MOD == 9 && SFLAG_LENGTHEN == 8, "native phoneme constants");
