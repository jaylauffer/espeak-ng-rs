/* Playback through the Rust audio sink on the default device (ALSA's null
 * device under CTest, through a test .asoundrc): the
 * pcaudio-shaped C API directly, then synchronous (and, with the queue,
 * asynchronous) speech, and the events playback delivers against those
 * retrieval reports. Skips where no device opens.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <espeak-ng/espeak_ng.h>
#include <espeak-ng/speak_lib.h>
#include "rust_audio.h"

#define SKIP 77

#if USE_ASYNC
#include <pthread.h>

// the events a callback saw: type, text position, and the thread
static char seen[4096];
static size_t seen_length;
static int other_thread;
static pthread_t main_thread;

static int record(short *wav, int count, espeak_EVENT *event)
{
	(void)wav;
	(void)count;
	for (; event && event->type != espeakEVENT_LIST_TERMINATED; event++) {
		int type = event->type;
		// what playback delivers: no sample-rate or sound-icon events, and
		// no empty words
		if (type == espeakEVENT_SAMPLERATE || type == espeakEVENT_PLAY
		    || (type == espeakEVENT_WORD && event->length == 0))
			continue;
		if (!pthread_equal(pthread_self(), main_thread))
			other_thread = 1;
		seen_length += snprintf(seen + seen_length, sizeof(seen) - seen_length, "%d@%d%s%s ",
		                        type, event->text_position,
		                        type == espeakEVENT_MARK ? ":" : "",
		                        type == espeakEVENT_MARK ? event->id.name : "");
	}
	return 0;
}
#endif

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

#if USE_ASYNC
	main_thread = pthread_self();
#endif
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

	// playback delivers the events retrieval reports, from another thread
	const char *marked = "<speak>One <mark name=\"here\"/> two. Three four.</speak>";
	unsigned int flags = espeakCHARS_AUTO | espeakSSML;
	TEST_ASSERT(espeak_ng_InitializeOutput(ENOUTPUT_MODE_SYNCHRONOUS, 0, NULL) == ENS_OK);
	espeak_SetSynthCallback(record);
	TEST_ASSERT(espeak_ng_Synthesize(marked, strlen(marked) + 1, 0, POS_CHARACTER, 0, flags, NULL, NULL) == ENS_OK);
	char retrieved[sizeof(seen)];
	strcpy(retrieved, seen);
	TEST_ASSERT(espeak_ng_InitializeOutput(ENOUTPUT_MODE_SPEAK_AUDIO, 0, NULL) == ENS_OK);
	// drop events of earlier messages still waiting for their audio
	TEST_ASSERT(espeak_ng_Cancel() == ENS_OK);
	seen_length = 0;
	seen[0] = 0;
	TEST_ASSERT(espeak_ng_Synthesize(marked, strlen(marked) + 1, 0, POS_CHARACTER, 0, flags, NULL, NULL) == ENS_OK);
	TEST_ASSERT(espeak_ng_Synchronize() == ENS_OK);
	for (int i = 0; i < 500 && strstr(seen, "6@") == NULL; i++)
		usleep(10000); // the delivery thread may still be calling back
	printf("retrieved: %s\nplayed:    %s\n", retrieved, seen);
	TEST_ASSERT(strstr(retrieved, "3@") != NULL && strstr(retrieved, ":here") != NULL);
	TEST_ASSERT(strncmp(seen, retrieved, strlen(retrieved)) == 0);
	TEST_ASSERT(strcmp(seen + strlen(retrieved), "6@0 ") == 0); // then the message ends
	TEST_ASSERT(other_thread);
#endif
	TEST_ASSERT(espeak_ng_Terminate() == ENS_OK);
	return EXIT_SUCCESS;
}
