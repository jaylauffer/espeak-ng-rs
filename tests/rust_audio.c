/* Playback through the Rust audio sink on the default device (ALSA's null
 * device under CTest, through a test .asoundrc): the
 * pcaudio-shaped C API directly, then synchronous (and, with the queue,
 * asynchronous) speech. Skips where no device opens.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <espeak-ng/espeak_ng.h>
#include <espeak-ng/speak_lib.h>
#include "rust_audio.h"

#define SKIP 77

int main(void)
{
	struct audio_object *audio = create_audio_device_object(NULL, "eSpeak", "test");
	TEST_ASSERT(audio != NULL);
	if (audio_object_open(audio, AUDIO_OBJECT_FORMAT_S16LE, 22050, 1) != 0) {
		printf("skipped: %s\n", audio_object_strerror(audio, 1));
		audio_object_destroy(audio);
		return SKIP;
	}
	static short samples[4410];
	for (int i = 0; i < 4410; i++)
		samples[i] = (short)((i * 37) % 2000 - 1000);
	// more than the sink holds at once: the writer waits for the device
	for (int i = 0; i < 20; i++)
		TEST_ASSERT(audio_object_write(audio, samples, sizeof(samples)) == 0);
	TEST_ASSERT(audio_object_drain(audio) == 0);
	TEST_ASSERT(audio_object_flush(audio) == 0);
	TEST_ASSERT(audio_object_write(audio, samples, sizeof(samples)) == 0);
	// a rate change keeps the device open
	TEST_ASSERT(audio_object_open(audio, AUDIO_OBJECT_FORMAT_S16LE, 16000, 1) == 0);
	TEST_ASSERT(audio_object_write(audio, samples, sizeof(samples)) == 0);
	TEST_ASSERT(audio_object_drain(audio) == 0);
	audio_object_close(audio);
	TEST_ASSERT(audio_object_write(audio, samples, sizeof(samples)) == 2);
	TEST_ASSERT(strcmp(audio_object_strerror(audio, 2), "audio device not open") == 0);
	audio_object_destroy(audio);

	espeak_ng_InitializePath(NULL);
	TEST_ASSERT(espeak_ng_Initialize(NULL) == ENS_OK);
	TEST_ASSERT(espeak_ng_InitializeOutput(ENOUTPUT_MODE_SPEAK_AUDIO | ENOUTPUT_MODE_SYNCHRONOUS, 0, NULL) == ENS_OK);
	TEST_ASSERT(espeak_ng_SetVoiceByName("en") == ENS_OK);
	const char *text = "Playback through the Rust sink. A second sentence.";
	TEST_ASSERT(espeak_ng_Synthesize(text, strlen(text) + 1, 0, POS_CHARACTER, 0, espeakCHARS_AUTO, NULL, NULL) == ENS_OK);
	TEST_ASSERT(espeak_ng_Synchronize() == ENS_OK);
#if USE_ASYNC
	TEST_ASSERT(espeak_ng_InitializeOutput(ENOUTPUT_MODE_SPEAK_AUDIO, 0, NULL) == ENS_OK);
	for (int i = 0; i < 3; i++)
		TEST_ASSERT(espeak_ng_Synthesize(text, strlen(text) + 1, 0, POS_CHARACTER, 0, espeakCHARS_AUTO, NULL, NULL) == ENS_OK);
	TEST_ASSERT(espeak_ng_Cancel() == ENS_OK);
	TEST_ASSERT(espeak_ng_Synthesize(text, strlen(text) + 1, 0, POS_CHARACTER, 0, espeakCHARS_AUTO, NULL, NULL) == ENS_OK);
	TEST_ASSERT(espeak_ng_Synchronize() == ENS_OK);
	TEST_ASSERT(espeak_IsPlaying() == 0);
#endif
	TEST_ASSERT(espeak_ng_Terminate() == ENS_OK);
	return EXIT_SUCCESS;
}
