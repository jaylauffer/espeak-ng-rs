/* The asynchronous API on the proactor-driven command queue: queued texts
 * produce the synchronous API's audio (less the final sample, as with the
 * legacy queue), a cancel stops a
 * long text, and synthesis works again afterwards.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <espeak-ng/espeak_ng.h>
#include <espeak-ng/speak_lib.h>

static uint64_t hash;
static long samples;
static int events;
static int callback(short *wav, int count, espeak_EVENT *event)
{
	for (int i = 0; i < count; i++)
		hash = (hash ^ (uint16_t)wav[i]) * 0x100000001b3ull;
	samples += count;
	for (; event != NULL && event->type != espeakEVENT_LIST_TERMINATED; event++) {
		hash = (hash ^ (unsigned)(event->type * 131 + event->text_position)) * 0x100000001b3ull;
		events++;
	}
	return 0;
}

static const char *texts[] = {
	"Hello world. This is a test of the asynchronous queue.",
	"<speak>One <mark name=\"m\"/> two <break time=\"200ms\"/> three.</speak>",
	"The quick brown fox jumps over the lazy dog.",
};

static void run(espeak_AUDIO_OUTPUT output, uint64_t *out_hash, long *out_samples)
{
	hash = 0xcbf29ce484222325ull; samples = 0; events = 0;
	TEST_ASSERT(espeak_Initialize(output, 0, NULL, 0) > 0);
	espeak_SetSynthCallback(callback);
	TEST_ASSERT(espeak_SetVoiceByName("en") == EE_OK);
	for (int i = 0; i < 3; i++)
		TEST_ASSERT(espeak_Synth(texts[i], strlen(texts[i]) + 1, 0, POS_CHARACTER, 0, espeakCHARS_AUTO | espeakSSML, NULL, NULL) == EE_OK);
	TEST_ASSERT(espeak_Synchronize() == EE_OK);
	*out_hash = hash; *out_samples = samples;
}

int main(void)
{
	uint64_t sync_hash, async_hash;
	long sync_samples, async_samples;
	run(AUDIO_OUTPUT_SYNCHRONOUS, &sync_hash, &sync_samples);
	TEST_ASSERT(espeak_Terminate() == EE_OK);
	run(AUDIO_OUTPUT_RETRIEVAL, &async_hash, &async_samples);
	printf("synchronous %ld samples, asynchronous %ld samples (hash %016llx), %d events\n", sync_samples, async_samples,
	       (unsigned long long)async_hash, events);
	// the asynchronous API ends a text one sample earlier than the synchronous
	// one, with either queue; the proactor queue must match the legacy one,
	// which the hash above lets a reference build compare
	TEST_ASSERT(sync_samples > 10000 && async_samples <= sync_samples && async_samples >= sync_samples - 2);

	// a long text, cancelled
	static char text[20000];
	text[0] = 0;
	while (strlen(text) + 60 < sizeof(text))
		strcat(text, "This sentence is repeated to make a long text. ");
	samples = 0;
	TEST_ASSERT(espeak_Synth(text, strlen(text) + 1, 0, POS_CHARACTER, 0, espeakCHARS_AUTO, NULL, NULL) == EE_OK);
	TEST_ASSERT(espeak_Cancel() == EE_OK);
	TEST_ASSERT(espeak_Synchronize() == EE_OK);
	TEST_ASSERT(espeak_IsPlaying() == 0);
	long cancelled = samples;
	printf("cancelled after %ld samples\n", cancelled);
	// This text is much longer than the three short parity texts combined.
	// Waiting for its following terminated-message command in fifo_add would
	// silently make submission synchronous and leave nothing to cancel.
	TEST_ASSERT(cancelled < sync_samples);

	// and it works again
	samples = 0;
	TEST_ASSERT(espeak_Synth(texts[2], strlen(texts[2]) + 1, 0, POS_CHARACTER, 0, espeakCHARS_AUTO, NULL, NULL) == EE_OK);
	TEST_ASSERT(espeak_Synchronize() == EE_OK);
	TEST_ASSERT(samples > 1000);
	TEST_ASSERT(espeak_Terminate() == EE_OK);
	return EXIT_SUCCESS;
}
