/* Compare the complete initialized WGEN_DATA contract. ABI padding is not
 * initialized by Rust struct moves and is not synthesizer state.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#pragma once
#include "synthesize.h"
static inline int test_wgen_equal(const WGEN_DATA *a,const WGEN_DATA *b)
{
#define EQUAL(field) (a->field == b->field)
	return EQUAL(pitch_env) && EQUAL(pitch) && EQUAL(pitch_ix) &&
		EQUAL(pitch_inc) && EQUAL(pitch_base) && EQUAL(pitch_range) &&
		EQUAL(mix_wavefile) && EQUAL(n_mix_wavefile) && EQUAL(mix_wave_scale) &&
		EQUAL(mix_wave_amp) && EQUAL(mix_wavefile_ix) && EQUAL(mix_wavefile_max) &&
		EQUAL(mix_wavefile_offset) && EQUAL(amplitude) && EQUAL(amplitude_v) &&
		EQUAL(amplitude_fmt);
#undef EQUAL
}
