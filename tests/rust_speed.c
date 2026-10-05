/* Speech-rate snapshots and Sonic effects against retained C, both features.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <espeak-ng/speak_lib.h>
#include "voice.h"
#include "synthesize.h"
#include "rust_data.h"
static voice_t source;
static voice_t *reference_voice = &source;
static SPEED_FACTORS expected;
static int expected_lengths[3], rates[N_EMBEDDED_VALUES];
static RustSonicEffects expected_effects;
static void RecordSonic(int multiplier)
{
	TEST_ASSERT(expected_effects.count < 2);
	expected_effects.values[expected_effects.count++] = multiplier;
}
#undef USE_LIBSONIC
#define USE_LIBSONIC 1
#define voice reference_voice
#define speed expected
#define len_speeds expected_lengths
#define embedded_value rates
#define DoSonicSpeed RecordSonic
#define SetSpeed ReferenceSonic
#define SetSpeedFactors ReferenceFactors
#define SetSpeedMods ReferenceMods
#define SetSpeedMultiplier ReferenceMultiplier
#include "speed_reference.inc"
#undef SetSpeed
#undef SetSpeedFactors
#undef SetSpeedMods
#undef SetSpeedMultiplier
#undef USE_LIBSONIC
#define USE_LIBSONIC 0
#define SetSpeed ReferencePlain
#define SetSpeedFactors ReferencePlainFactors
#define SetSpeedMods ReferencePlainMods
#define SetSpeedMultiplier ReferencePlainMultiplier
#define speed_lookup plain_lookup
#define pause_factor_350 plain_pause
#define wav_factor_350 plain_wave
#include "speed_reference.inc"
#undef voice
#undef speed
#undef len_speeds
#undef embedded_value
#undef DoSonicSpeed
#undef SetSpeed
#undef SetSpeedFactors
#undef SetSpeedMods
#undef SetSpeedMultiplier

static unsigned sequence = 0x192fe2u;
static unsigned next(void) { sequence = sequence * 1664525u + 1013904223u; return sequence; }
static void compare(int rate, int secondary, unsigned control, unsigned sonic)
{
	SPEED_FACTORS actual;
	int32_t lengths[3];
	RustSonicEffects effects = {0};
	int32_t initial[8];
	for (int index = 0; index < 8; index++) initial[index] = next() % 1000;
	memcpy(&actual, initial, sizeof(actual));
	for (int index = 0; index < 3; index++) lengths[index] = (int)(next() % 1000) - 500;
	expected = actual;
	memcpy(expected_lengths, lengths, sizeof(lengths));
	memset(&expected_effects, 0, sizeof(expected_effects));
	rates[EMBED_S] = rate;
	rates[EMBED_S2] = secondary;
	if (sonic) ReferenceSonic(control); else ReferencePlain(control);
	TEST_ASSERT(espeak_rs_speed_configure(&source, &actual, &lengths, rate, secondary,
	                                      control, sonic, &effects) == 0);
	if (memcmp(&actual, &expected, sizeof(actual)) != 0)
		fprintf(stderr, "Rate mismatch %d/%d control %u sonic %u percent %d\n",
		        rate, secondary, control, sonic, source.speed_percent);
	TEST_ASSERT(memcmp(&actual, &expected, sizeof(actual)) == 0);
	TEST_ASSERT(memcmp(lengths, expected_lengths, sizeof(lengths)) == 0);
	TEST_ASSERT(memcmp(&effects, &expected_effects, sizeof(effects)) == 0);
}
int main(void)
{
	_Static_assert(sizeof(SPEED_FACTORS) == 32, "native speed layout");
	_Static_assert(sizeof(RustSonicEffects) == 12, "native effects layout");
	size_t count = 0;
	for (int percent = 0; percent <= 180; percent += 60) {
		source.speed_percent = percent;
		for (int rate = -128; rate <= 1500; rate++) {
			for (unsigned control = 0; control < 4; control++) {
				for (unsigned sonic = 0; sonic < 2; sonic++) {
					source.speedf1 = 256; source.speedf2 = 238; source.speedf3 = 232;
					compare(rate, 1500-rate, control, sonic); count++;
					/* Signed/custom settings cover C-defined arithmetic only. */
					source.speedf1 = (int)(next()%4096)-2048;
					source.speedf2 = (int)(next()%4096)-2048;
					source.speedf3 = (int)(next()%4096)-2048;
					compare(rate, 1500-rate, control, sonic); count++;
				}
			}
		}
	}
	SPEED_FACTORS previous = expected, actual = expected;
	int32_t lengths[3], old_lengths[3];
	memcpy(lengths, expected_lengths, sizeof(lengths));
	memcpy(old_lengths, lengths, sizeof(lengths));
	RustSonicEffects effects = {2,{123,456}}, old_effects = effects;
	TEST_ASSERT(espeak_rs_speed_configure(&source,&actual,&lengths,175,175,4,0,&effects) == 1);
	TEST_ASSERT(espeak_rs_speed_configure(&source,&actual,&lengths,175,175,3,2,&effects) == 1);
	source.speed_percent = 100;
	TEST_ASSERT(espeak_rs_speed_configure(&source,&actual,&lengths,INT32_MAX,175,3,0,&effects) == 1);
	TEST_ASSERT(memcmp(&actual,&previous,sizeof(actual)) == 0);
	TEST_ASSERT(memcmp(lengths,old_lengths,sizeof(lengths)) == 0);
	TEST_ASSERT(memcmp(&effects,&old_effects,sizeof(effects)) == 0);
	printf("Compared %zu speech-rate states and ordered Sonic effects with C\n",count);
	return 0;
}
