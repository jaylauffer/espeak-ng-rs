/* The engine's data, dictionaries and voices load through loadngo's proactor
 * in a proactor build: synthesis works and the reads did not fall back to
 * blocking std::fs.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <espeak-ng/espeak_ng.h>
#include <espeak-ng/speak_lib.h>
#include <espeak-ng/encoding.h>
#include "phoneme.h"
#include "synthesize.h"
#include "voice.h"
#include "rust_data.h"

static int samples;
static int callback(short *wav, int count, espeak_EVENT *events)
{
	(void)wav;(void)events;
	samples += count;
	return 0;
}

int main(void)
{
	espeak_ng_InitializePath(getenv("ESPEAK_DATA_PATH"));
	TEST_ASSERT(espeak_ng_Initialize(NULL) == ENS_OK);
	TEST_ASSERT(espeak_ng_InitializeOutput(ENOUTPUT_MODE_SYNCHRONOUS, 0, NULL) == ENS_OK);
	espeak_SetSynthCallback(callback);
	// phoneme data at initialization; a dictionary and voice file now
	TEST_ASSERT(espeak_ng_SetVoiceByName("de") == ENS_OK);
	const char *text = "Hallo Welt.";
	TEST_ASSERT(espeak_ng_Synthesize(text, strlen(text) + 1, 0, POS_CHARACTER, 0, espeakCHARS_AUTO, NULL, NULL) == ENS_OK);
	TEST_ASSERT(espeak_ng_Synchronize() == ENS_OK);
	TEST_ASSERT(samples > 1000);
	int backend = espeak_rs_engine_io_backend();
#ifdef ESPEAK_TEST_PROACTOR
	TEST_ASSERT(backend == 0 || backend == 1); // a proactor, not std::fs
#else
	TEST_ASSERT(backend == 2);
#endif
	printf("engine I/O backend %d; %d samples\n", backend, samples);
	TEST_ASSERT(espeak_ng_Terminate() == ENS_OK);
	return EXIT_SUCCESS;
}
