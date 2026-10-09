/*
 * Copyright (C) 2005 to 2013 by Jonathan Duddington
 * email: jonsd@users.sourceforge.net
 * Copyright (C) 2013-2017 Reece H. Dunn
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program; if not, see: <http://www.gnu.org/licenses/>.
 */

#include "config.h"
#include "soundicon.h"

#include <assert.h>
#include <ctype.h>
#include <errno.h>
#include <locale.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
#include <wchar.h>

#if USE_RUST_AUDIO
#include "rust_audio.h"
#define HAVE_AUDIO_OUTPUT 1
#elif USE_LIBPCAUDIO
#include <pcaudiolib/audio.h>
#define HAVE_AUDIO_OUTPUT 1
#endif

#ifndef HAVE_AUDIO_OUTPUT
#define HAVE_AUDIO_OUTPUT 0
#endif

#if defined(_WIN32) || defined(_WIN64)
#include <fcntl.h>
#include <io.h>
#include <windows.h>
#include <winreg.h>
#endif

#include <espeak-ng/espeak_ng.h>
#include <espeak-ng/speak_lib.h>
#include <espeak-ng/encoding.h>

#include "speech.h"
#include "common.h"               // for GetFileLength
#include "dictionary.h"           // for GetTranslatedPhonemeString, strncpy0
#include "espeak_command.h"       // for delete_espeak_command, SetParameter
#include "event.h"                // for event_declare, event_clear_all, eve...
#include "fifo.h"                 // for fifo_add_command, fifo_add_commands
#include "langopts.h"             // for LoadConfig
#include "mbrola.h"               // for mbrola_delay
#include "readclause.h"           // for PARAM_STACK, param_stack
#include "synthdata.h"            // for FreePhData, LoadPhData
#include "synthesize.h"           // for SpeakNextClause, Generate, Synthesi...
#ifdef USE_RUST_CORE
#include "rust_data.h"
#endif
#include "translate.h"            // for p_decoder, InitText, translator
#include "voice.h"                // for FreeVoiceList, VoiceReset, current_...
#include "wavegen.h"              // for WavegenFill, WavegenInit, WcmdqUsed

#ifndef USE_RUST_CORE
static unsigned char *outbuf = NULL;
static int outbuf_size = 0;
static unsigned char *out_start;
#else
// The output buffer belongs to Rust.
#define outbuf (espeak_rs_output.buffer)
#define out_start (espeak_rs_output.buffer)
#endif

#ifndef USE_RUST_CORE
espeak_EVENT *event_list = NULL;
static int event_list_ix = 0;
static int n_event_list;
#else
// The event list belongs to Rust.
#define event_list (espeak_rs_events.events)
#define event_list_ix (espeak_rs_events.count)
#define n_event_list (espeak_rs_events.capacity)
#endif
static long count_samples;
#ifndef USE_RUST_CORE
#if HAVE_AUDIO_OUTPUT
static struct audio_object *my_audio = NULL;
#endif

static espeak_ng_OUTPUT_MODE my_mode = ENOUTPUT_MODE_SYNCHRONOUS;
static int out_samplerate = 0;
static int voice_samplerate = 22050;
static const int min_buffer_length = 60; // minimum buffer length in ms
static espeak_ng_STATUS err = ENS_OK;
#define ENGINE_STORE(field, variable, value) ((variable) = (value))
#else
#include "rust_engine_lifecycle.h"
#include "rust_engine_request.h"
static const RustEngineLifecycle engine_lifecycle;
static const RustEngineRequest engine_request;
#define my_audio espeak_rs_engine_audio()
#define my_mode ((espeak_ng_OUTPUT_MODE)espeak_rs_engine_value(RUST_ENGINE_MODE))
#define out_samplerate espeak_rs_engine_value(RUST_ENGINE_OUTPUT_RATE)
#define voice_samplerate espeak_rs_engine_value(RUST_ENGINE_VOICE_RATE)
#define err ((espeak_ng_STATUS)espeak_rs_engine_value(RUST_ENGINE_ERROR))
#define ENGINE_STORE(field, variable, value) espeak_rs_engine_store(field, (int32_t)(value))
#endif

static unsigned int my_unique_identifier = 0;
static void *my_user_data = NULL;

static t_espeak_callback *synth_callback = NULL;

char path_home[N_PATH_BUF]; // this is the espeak-ng-data directory
extern int saved_parameters[N_SPEECH_PARAM]; // Parameters saved on synthesis start

void cancel_audio(void)
{
#if HAVE_AUDIO_OUTPUT
	if ((my_mode & ENOUTPUT_MODE_SPEAK_AUDIO) == ENOUTPUT_MODE_SPEAK_AUDIO) {
		audio_object_flush(my_audio);
	}
#endif
}

#if USE_ASYNC
#if USE_PROACTOR
// Declares an event for delivery when its sample plays: after the audio
// still queued for the device, less what follows the event in the buffer
// just written. Without the Rust sink the delay is unknown, so it is 0.
static espeak_ng_STATUS declare_event(espeak_EVENT *event)
{
	int delay = 0;
#if USE_RUST_AUDIO
	if (my_audio != NULL && voice_samplerate > 0) {
#if USE_MBROLA
		long end = count_samples + mbrola_delay;
#else
		long end = count_samples;
#endif
		long after = (event->type == espeakEVENT_MSG_TERMINATED) ? 0 : end - event->sample;
		delay = espeak_rs_audio_latency_ms(my_audio);
		if (after > 0)
			delay -= (int)(after * 1000 / voice_samplerate);
	}
#endif
	return espeak_rs_event_declare_wait(event, delay < 0 ? 0 : delay);
}
#else
#define declare_event event_declare
#endif
#endif

static int dispatch_audio(short *samples, int length, espeak_EVENT *event)
{
	int a_wave_can_be_played = 1;
#if USE_ASYNC
	if ((my_mode & ENOUTPUT_MODE_SYNCHRONOUS) == 0)
		a_wave_can_be_played = fifo_is_command_enabled();
#endif

	switch ((int)my_mode)
	{
	case ENOUTPUT_MODE_SPEAK_AUDIO:
	case ENOUTPUT_MODE_SPEAK_AUDIO | ENOUTPUT_MODE_SYNCHRONOUS:
	{
		int event_type = 0;
		if (event)
			event_type = event->type;

		if (event_type == espeakEVENT_SAMPLERATE) {
			ENGINE_STORE(RUST_ENGINE_VOICE_RATE, voice_samplerate, event->id.number);

			if (out_samplerate != voice_samplerate) {
#if HAVE_AUDIO_OUTPUT
				if (out_samplerate != 0) {
					// sound was previously open with a different sample rate
					audio_object_close(my_audio);
					ENGINE_STORE(RUST_ENGINE_OUTPUT_RATE, out_samplerate, 0);
				}
#endif
#if HAVE_AUDIO_OUTPUT
				int error = audio_object_open(my_audio, AUDIO_OBJECT_FORMAT_S16LE, voice_samplerate, 1);
				if (error != 0) {
					fprintf(stderr, "audio reopen error: %s\n", audio_object_strerror(my_audio, error));
					ENGINE_STORE(RUST_ENGINE_ERROR, err, ENS_AUDIO_ERROR);
					return -1;
				}
#endif
				ENGINE_STORE(RUST_ENGINE_OUTPUT_RATE, out_samplerate, voice_samplerate);
#if USE_ASYNC
				if ((my_mode & ENOUTPUT_MODE_SYNCHRONOUS) == 0)
					event_init();
#endif
			}
		}

#if HAVE_AUDIO_OUTPUT
		if (out_samplerate == 0) {
			int error = audio_object_open(my_audio, AUDIO_OBJECT_FORMAT_S16LE, voice_samplerate, 1);
			if (error != 0) {
				fprintf(stderr, "audio open error: %s\n", audio_object_strerror(my_audio, error));
				ENGINE_STORE(RUST_ENGINE_ERROR, err, ENS_AUDIO_ERROR);
				return -1;
			}
			ENGINE_STORE(RUST_ENGINE_OUTPUT_RATE, out_samplerate, voice_samplerate);
		}
#endif

#if HAVE_AUDIO_OUTPUT
		if (samples && length && a_wave_can_be_played) {
			int error = audio_object_write(my_audio, (char *)samples, 2*length);
			if (error != 0)
				fprintf(stderr, "audio write error: %s\n", audio_object_strerror(my_audio, error));
		}
#endif

#if USE_ASYNC
		while (event && a_wave_can_be_played) {
			// TBD: some event are filtered here but some insight might be given
			// TBD: in synthesise.cpp for avoiding to create WORDs with size=0.
			// TBD: For example sentence "or ALT)." returns three words
			// "or", "ALT" and "".
			// TBD: the last one has its size=0.
			if ((event->type == espeakEVENT_WORD) && (event->length == 0))
				break;
			if ((my_mode & ENOUTPUT_MODE_SYNCHRONOUS) == 0) {
				ENGINE_STORE(RUST_ENGINE_ERROR, err, declare_event(event));
#if USE_PROACTOR
				break; // native admission already waits for capacity completions
#else
				if (err != ENS_EVENT_BUFFER_FULL)
					break;
				usleep(10000);
				a_wave_can_be_played = fifo_is_command_enabled();
#endif
			} else
				break;
		}
#endif
	}
		break;
	case 0:
		if (synth_callback)
			synth_callback(samples, length, event);
		break;
	}

	return a_wave_can_be_played == 0; // 1 = stop synthesis, -1 = error
}

static int create_events(short *samples, int length, espeak_EVENT *events)
{
	int finished;
	int i = 0;

	// The audio data are written to the output device.
	// The list of events in event_list (index: event_list_ix) is read:
	// Each event is declared to the "event" object which stores them internally.
	// The event object is responsible of calling the external callback
	// as soon as the relevant audio sample is played.

	do { // for each event
		espeak_EVENT *event;
		if (event_list_ix == 0)
			event = NULL;
		else
			event = events + i;
		finished = dispatch_audio((short *)samples, length, event);
		length = 0; // the wave data are played once.
		i++;
	} while ((i < event_list_ix) && !finished);
	return finished;
}

#if USE_ASYNC

int sync_espeak_terminated_msg(uint32_t unique_identifier, void *user_data)
{
	int finished = 0;

#ifndef USE_RUST_CORE
	memset(event_list, 0, 2*sizeof(espeak_EVENT));

	event_list[0].type = espeakEVENT_MSG_TERMINATED;
	event_list[0].unique_identifier = unique_identifier;
	event_list[0].user_data = user_data;
	event_list[1].type = espeakEVENT_LIST_TERMINATED;
	event_list[1].unique_identifier = unique_identifier;
	event_list[1].user_data = user_data;
#else
	espeak_rs_events_terminated_message(&espeak_rs_events, unique_identifier, user_data);
#endif

	if (my_mode == ENOUTPUT_MODE_SPEAK_AUDIO) {
#if USE_PROACTOR
		ENGINE_STORE(RUST_ENGINE_ERROR, err, declare_event(event_list));
#else
		while (1) {
			ENGINE_STORE(RUST_ENGINE_ERROR, err, declare_event(event_list));
			if (err != ENS_EVENT_BUFFER_FULL)
				break;
			usleep(10000);
		}
#endif
	} else if (synth_callback)
		finished = synth_callback(NULL, 0, event_list);
	return finished;
}

#endif

static int check_data_path(const char *path, int allow_directory)
{
	if (!path) return 0;

	snprintf(path_home, sizeof(path_home), "%s/espeak-ng-data", path);
	if (GetFileLength(path_home) == -EISDIR)
		return 1;

	if (!allow_directory)
		return 0;

	snprintf(path_home, sizeof(path_home), "%s", path);
	return GetFileLength(path_home) == -EISDIR;
}

#pragma GCC visibility push(default)

#ifndef USE_RUST_CORE
/* Begin retained engine output. */
ESPEAK_NG_API espeak_ng_STATUS espeak_ng_InitializeOutput(espeak_ng_OUTPUT_MODE output_mode, int buffer_length, const char *device)
{
	(void)device; // unused without audio output

	my_mode = output_mode;
	out_samplerate = 0;

#if HAVE_AUDIO_OUTPUT
	if (((my_mode & ENOUTPUT_MODE_SPEAK_AUDIO) == ENOUTPUT_MODE_SPEAK_AUDIO) && (my_audio == NULL))
		my_audio = create_audio_device_object(device, "eSpeak", "Text-to-Speech");
#endif

#if USE_ASYNC
	if ((my_mode & ENOUTPUT_MODE_SYNCHRONOUS) == 0) fifo_init();
#endif

	// Don't allow buffer be smaller than safe minimum
	if (buffer_length < min_buffer_length)
		buffer_length = min_buffer_length;

	// allocate 2 bytes per sample
	// Always round up to the nearest sample and the nearest byte.
	int millisamples = buffer_length * samplerate;
#ifndef USE_RUST_CORE
	outbuf_size = (millisamples + 1000 - millisamples % 1000) / 500;
	out_start = (unsigned char *)realloc(outbuf, outbuf_size);
	if (out_start == NULL)
		return ENOMEM;
	else
		outbuf = out_start;
#else
	if (espeak_rs_output_reserve(&espeak_rs_output, (size_t)((millisamples + 1000 - millisamples % 1000) / 500)) != 0)
		return ENOMEM;
#endif

	// allocate space for event list.  Allow 200 events per second.
	// Add a constant to allow for very small buffer_length
#ifndef USE_RUST_CORE
	n_event_list = (buffer_length*200)/1000 + 20;
	espeak_EVENT *new_event_list = (espeak_EVENT *)realloc(event_list, sizeof(espeak_EVENT) * n_event_list);
	if (new_event_list == NULL)
		return ENOMEM;
	event_list = new_event_list;
#else
	if (espeak_rs_events_reserve(&espeak_rs_events, (buffer_length*200)/1000 + 20) != 0)
		return ENOMEM;
#endif

	return ENS_OK;
}
/* End retained engine output. */
#else
ESPEAK_NG_API espeak_ng_STATUS espeak_ng_InitializeOutput(espeak_ng_OUTPUT_MODE mode, int length, const char *device)
{
	return espeak_rs_engine_output(&engine_lifecycle, mode, length, samplerate, device);
}
#endif


ESPEAK_NG_API void espeak_ng_InitializePath(const char *path)
{
	if (check_data_path(path, 1))
		return;

#if PLATFORM_WINDOWS
	HKEY RegKey;
	unsigned long size;
	unsigned long var_type;
	unsigned char buf[sizeof(path_home)-13];

	if (check_data_path(getenv("ESPEAK_DATA_PATH"), 1))
		return;

	buf[0] = 0;
	RegOpenKeyExA(HKEY_LOCAL_MACHINE, "Software\\eSpeak NG", 0, KEY_READ, &RegKey);
	if (RegKey == NULL)
		RegOpenKeyExA(HKEY_LOCAL_MACHINE, "Software\\WOW6432Node\\eSpeak NG", 0, KEY_READ, &RegKey);
	size = sizeof(buf);
	var_type = REG_SZ;
	RegQueryValueExA(RegKey, "Path", 0, &var_type, buf, &size);

	if (check_data_path(buf, 1))
		return;
#elif !defined(PLATFORM_DOS)
	if (check_data_path(getenv("ESPEAK_DATA_PATH"), 1))
		return;

	if (check_data_path(getenv("HOME"), 0))
		return;
#endif

	strcpy(path_home, PATH_ESPEAK_DATA);
}

const int param_defaults[N_SPEECH_PARAM] = {
	0,   // silence (internal use)
	espeakRATE_NORMAL, // rate wpm
	100, // volume
	50,  // pitch
	50,  // range
	0,   // punctuation
	0,   // capital letters
	0,   // wordgap
	0,   // options
	0,   // intonation
	100, // ssml break mul
	0,
	0,   // emphasis
	0,   // line length
	0,   // voice type
};


#ifdef USE_RUST_CORE
/* Platform/engine primitives only; lifecycle decisions live in Rust. */
#if !USE_ASYNC || !USE_MBROLA
static void RustEngineNoop(void) {}
#endif
#if USE_ASYNC
static void RustEngineQueueStop(void) { (void)fifo_stop(); }
static void RustEngineEventClear(void) { (void)event_clear_all(); }
#else
#define fifo_init RustEngineNoop
#define RustEngineQueueStop RustEngineNoop
#define fifo_terminate RustEngineNoop
#define RustEngineEventClear RustEngineNoop
#define event_terminate RustEngineNoop
#endif
static void RustEngineCurrentVoiceClear(void) { memset(espeak_GetCurrentVoice(), 0, sizeof(espeak_VOICE)); }
static void RustEngineStackReset(void) { SetVoiceStack(NULL, ""); }
static void RustEngineVoiceReset(void) { VoiceReset(0); }
static void RustEngineEventsRelease(void) { espeak_rs_events_release(&espeak_rs_events); }
static void RustEngineOutputRelease(void) { espeak_rs_output_release(&espeak_rs_output); }
static int RustEngineOutputReserve(size_t size) { return espeak_rs_output_reserve(&espeak_rs_output, size); }
static int RustEngineEventsReserve(int count) { return espeak_rs_events_reserve(&espeak_rs_events, count); }
static long RustEngineClock(void) { return (long)time(NULL); }
static const RustEngineLifecycle engine_lifecycle = {
	.capabilities = (USE_ASYNC ? 1 : 0) | (HAVE_AUDIO_OUTPUT ? 2 : 0) | (USE_MBROLA ? 4 : 0) |
#if USE_PROACTOR
		8,
#else
		0,
#endif
	.actions = { LoadConfig, SynthesizeInit, InitNamedata, fifo_init, RustEngineQueueStop, fifo_terminate,
		RustEngineEventClear, event_terminate, FreePhData, FreeVoiceList, FreeCurrentVoice,
		FreeAlternateTranslators, FreeDictionaryCache, WavegenFini, FreeNamedata, FreeSoundIcons,
#if USE_MBROLA
		FreeMbrolaTable,
#else
		RustEngineNoop,
#endif
		RustEngineCurrentVoiceClear, RustEngineStackReset, RustEngineVoiceReset, RustEngineEventsRelease, RustEngineOutputRelease },
	.locale = setlocale, .ctype = LC_CTYPE, .load = LoadPhData, .wave_init = WavegenInit,
	.defaults = param_defaults, .current = param_stack[0].parameter, .saved = saved_parameters,
	.capitals = &option_capitals, .punctuation = &option_punctuation, .phonemes = &option_phonemes,
	.phoneme_events = &option_phoneme_events, .echo = &embedded_value[EMBED_T], .parameter = SetParameter,
	.clock = RustEngineClock, .seed = espeak_srand,
#if HAVE_AUDIO_OUTPUT
	.create_audio = create_audio_device_object, .close_audio = audio_object_close,
	.destroy_audio = audio_object_destroy, .flush_audio = audio_object_flush,
#endif
	.output_reserve = RustEngineOutputReserve, .events_reserve = RustEngineEventsReserve,
#if USE_ASYNC && USE_PROACTOR
	.synchronize = fifo_synchronize,
#endif
	.translator = &translator, .destroy_translator = DeleteTranslator,
	.decoder = &p_decoder, .destroy_decoder = destroy_text_decoder
};
#endif

#ifndef USE_RUST_CORE
/* Begin retained engine initialization. */
ESPEAK_NG_API espeak_ng_STATUS espeak_ng_Initialize(espeak_ng_ERROR_CONTEXT *context)
{
	int param;
	int srate = 22050; // default sample rate 22050 Hz

	// It seems that the wctype functions don't work until the locale has been set
	// to something other than the default "C".  Then, not only Latin1 but also the
	// other characters give the correct results with iswalpha() etc.
	if (setlocale(LC_CTYPE, "C.UTF-8") == NULL) {
		if (setlocale(LC_CTYPE, "UTF-8") == NULL) {
			if (setlocale(LC_CTYPE, "en_US.UTF-8") == NULL)
				setlocale(LC_CTYPE, "");
		}
	}

	espeak_ng_STATUS result = LoadPhData(&srate, context);
	if (result != ENS_OK)
		return result;

	WavegenInit(srate, 0);
	LoadConfig();

	espeak_VOICE *current_voice_selected = espeak_GetCurrentVoice();
	memset(current_voice_selected, 0, sizeof(espeak_VOICE));
	SetVoiceStack(NULL, "");
	SynthesizeInit();
	InitNamedata();

	VoiceReset(0);

	for (param = 0; param < N_SPEECH_PARAM; param++)
		param_stack[0].parameter[param] = saved_parameters[param] = param_defaults[param];

	SetParameter(espeakRATE, espeakRATE_NORMAL, 0);
	SetParameter(espeakVOLUME, 100, 0);
	SetParameter(espeakCAPITALS, option_capitals, 0);
	SetParameter(espeakPUNCTUATION, option_punctuation, 0);
	SetParameter(espeakWORDGAP, 0, 0);

	option_phonemes = 0;
	option_phoneme_events = 0;

	// Seed random generator
	espeak_srand(time(NULL));

	return ENS_OK;
}
/* End retained engine initialization. */
#else
ESPEAK_NG_API espeak_ng_STATUS espeak_ng_Initialize(espeak_ng_ERROR_CONTEXT *context)
{
	return espeak_rs_engine_initialize(&engine_lifecycle, context);
}
#endif

ESPEAK_NG_API espeak_ng_STATUS espeak_ng_SetPhonemeEvents(int enable, int ipa) {
	option_phoneme_events = 0;
	if (enable) {
		option_phoneme_events |= espeakINITIALIZE_PHONEME_EVENTS;
		if (ipa) {
			option_phoneme_events |= espeakINITIALIZE_PHONEME_IPA;
		}
	}
	return ENS_OK;
}

ESPEAK_NG_API int espeak_ng_GetSampleRate(void)
{
	return samplerate;
}

#pragma GCC visibility pop

#if !defined(USE_RUST_CORE) || !USE_PROACTOR
/* Begin retained engine driver. */
// One pass of the synthesis loop: fill a buffer, deliver it with its events,
// generate more. Returns 1 when synthesis has finished, with its status.
typedef struct {
	unsigned int unique_identifier;
	espeak_ng_STATUS status;
} SynthesisState;

static int SynthesizeStep(void *context)
{
	SynthesisState *state = (SynthesisState *)context;
	int length;
	int finished = 0;

#ifndef USE_RUST_CORE
	out_ptr = outbuf;
	out_end = &outbuf[outbuf_size];
#else
	espeak_rs_output_begin(&espeak_rs_output);
#endif
	event_list_ix = 0;
	WavegenFill();

	length = (out_ptr - outbuf)/2;
	count_samples += length;
#ifndef USE_RUST_CORE
	event_list[event_list_ix].type = espeakEVENT_LIST_TERMINATED; // indicates end of event list
	event_list[event_list_ix].unique_identifier = state->unique_identifier;
	event_list[event_list_ix].user_data = my_user_data;
#else
	espeak_rs_events_terminate(&espeak_rs_events, event_list_ix, state->unique_identifier, my_user_data);
#endif

	if ((my_mode & ENOUTPUT_MODE_SPEAK_AUDIO) == ENOUTPUT_MODE_SPEAK_AUDIO) {
		finished = create_events((short *)outbuf, length, event_list);
		if (finished < 0)
			{ state->status = ENS_AUDIO_ERROR; return 1; }
	} else if (synth_callback)
		finished = synth_callback((short *)outbuf, length, event_list);
	if (finished) {
		SpeakNextClause(2); // stop
		{ state->status = ENS_SPEECH_STOPPED; return 1; }
	}

	if (Generate(phoneme_list, &n_phoneme_list, 1) == 0) {
		if (WcmdqUsed() == 0) {
			// don't process the next clause until the previous clause has finished generating speech.
			// This ensures that <audio> tag (which causes end-of-clause) is at a sound buffer boundary

#ifndef USE_RUST_CORE
			event_list[0].type = espeakEVENT_LIST_TERMINATED;
			event_list[0].unique_identifier = my_unique_identifier;
			event_list[0].user_data = my_user_data;
#else
			espeak_rs_events_terminate(&espeak_rs_events, 0, my_unique_identifier, my_user_data);
#endif

			if (SpeakNextClause(1) == 0) {
				finished = 0;
				if ((my_mode & ENOUTPUT_MODE_SPEAK_AUDIO) == ENOUTPUT_MODE_SPEAK_AUDIO) {
					if (dispatch_audio(NULL, 0, NULL) < 0)
						{ state->status = ENS_AUDIO_ERROR; return 1; }
				} else if (synth_callback)
					finished = synth_callback(NULL, 0, event_list); // NULL buffer ptr indicates end of data
				if (finished) {
					SpeakNextClause(2); // stop
					{ state->status = ENS_SPEECH_STOPPED; return 1; }
				}
				{ state->status = ENS_OK; return 1; }
			}
		}
	}
	return 0;
}

static espeak_ng_STATUS Synthesize(unsigned int unique_identifier, const void *text, int flags)
{
	// Fill the buffer with output sound
	if ((outbuf == NULL) || (event_list == NULL))
		return ENS_NOT_INITIALIZED;

	option_ssml = flags & espeakSSML;
	option_phoneme_input = flags & espeakPHONEMES;
	option_endpause = flags & espeakENDPAUSE;

	count_samples = 0;

	espeak_ng_STATUS status;
	if (translator == NULL) {
		status = espeak_ng_SetVoiceByName(ESPEAKNG_DEFAULT_VOICE);
		if (status != ENS_OK)
			return status;
	}

	if (p_decoder == NULL)
		p_decoder = create_text_decoder();

	status = text_decoder_decode_string_multibyte(p_decoder, text, translator->encoding, flags);
	if (status != ENS_OK)
		return status;

	SpeakNextClause(0);

	SynthesisState state = { unique_identifier, ENS_OK };
#ifdef USE_PROACTOR
	// each pass is a work item on this thread's proactor (synthesis_loop.rs)
	if (espeak_rs_synthesis_run(SynthesizeStep, &state) < 0) {
		SpeakNextClause(2);
		state.status = ENS_SPEECH_STOPPED;
	}
#else
	while (SynthesizeStep(&state) == 0)
		;
#endif
	return state.status;
}

/* End retained engine driver. */
#else
#include "rust_engine_driver.h"
static unsigned RustDriverEncoding(Translator *value) { return value->encoding; }
static espeak_ng_STATUS RustDriverDefaultVoice(void) { return espeak_ng_SetVoiceByName(ESPEAKNG_DEFAULT_VOICE); }
static int RustDriverGenerate(void) { return Generate(phoneme_list, &n_phoneme_list, 1); }
static const RustEngineDriver engine_driver = {
    .output = &espeak_rs_output, .events = &espeak_rs_events, .samples = &count_samples,
    .options = { &option_ssml, &option_phoneme_input, &option_endpause },
    .translator = &translator, .decoder = &p_decoder, .encoding = RustDriverEncoding,
    .voice = RustDriverDefaultVoice, .create_decoder = create_text_decoder,
    .decode = text_decoder_decode_string_multibyte, .begin = espeak_rs_output_begin,
    .fill = WavegenFill, .terminate_events = espeak_rs_events_terminate,
    .identifier = &my_unique_identifier, .user = &my_user_data,
    .value = espeak_rs_engine_value, .callback = &synth_callback,
    .play = create_events, .dispatch = dispatch_audio, .generate = RustDriverGenerate,
    .queued = WcmdqUsed, .clause = SpeakNextClause, .run = espeak_rs_synthesis_run
};
static espeak_ng_STATUS Synthesize(unsigned int unique_identifier, const void *text, int flags)
{
    return espeak_rs_driver_synthesize(&engine_driver, unique_identifier, text, flags);
}
#endif

#ifndef USE_RUST_CORE
void MarkerEvent(int type, unsigned int char_position, int value, int value2, unsigned char *position)
{
	// type: 1=word, 2=sentence, 3=named mark, 4=play audio, 5=end, 7=phoneme
	espeak_EVENT *ep;
	double time;

	if ((event_list == NULL) || (event_list_ix >= (n_event_list-2)))
		return;

	ep = &event_list[event_list_ix++];
	ep->type = (espeak_EVENT_TYPE)type;
	ep->unique_identifier = my_unique_identifier;
	ep->user_data = my_user_data;
	ep->text_position = char_position & 0xffffff;
	ep->length = char_position >> 24;

#if !USE_MBROLA
	static const int mbrola_delay = 0;
#endif

	time = ((double)(count_samples + mbrola_delay + (position - out_start)/2)*1000.0)/samplerate;
	ep->audio_position = (int)time;
	ep->sample = (count_samples + mbrola_delay + (position - out_start)/2);

	if ((type == espeakEVENT_MARK) || (type == espeakEVENT_PLAY))
		ep->id.name = &namedata[value];
	else if (type == espeakEVENT_PHONEME) {
		int *p;
		p = (int *)(ep->id.string);
		p[0] = value;
		p[1] = value2;
	} else
		ep->id.number = value;
}

/* End legacy marker event. */
#else
void MarkerEvent(int type, unsigned int char_position, int value, int value2, unsigned char *position)
{
	// type: 1=word, 2=sentence, 3=named mark, 4=play audio, 5=end, 7=phoneme
#if !USE_MBROLA
	static const int mbrola_delay = 0;
#endif
	RustEventSettings settings = { my_unique_identifier, my_user_data, count_samples, mbrola_delay, samplerate, namedata };
	espeak_rs_event_marker(&espeak_rs_events, &settings, type, char_position, value, value2, position - out_start);
}
#endif

#if USE_LIBSONIC
#ifndef USE_RUST_CORE
void RescaleEventSamples(int length_pre, int length_post)
{
	// MarkerEvent() records positions while the buffer is being filled, which
	// happens before libsonic compresses it, so the positions refer to audio
	// that is longer than what is handed to the caller.  Map them onto the
	// audio actually produced, keeping ep->sample and ep->audio_position
	// consistent with the buffer passed to the synth callback.
	//
	// libsonic is a stream and can carry samples over between calls, so this
	// linear mapping is an approximation; what it guarantees is that an event
	// never points past the audio its buffer produced.

#if !USE_MBROLA
	static const int mbrola_delay = 0;
#endif
	int base;

	if ((event_list == NULL) || (length_pre <= 0) || (length_pre == length_post))
		return;

	// count_samples is not advanced for this buffer until WavegenFill() has
	// returned, so it still holds the total emitted by preceding buffers --
	// the same base MarkerEvent() used.
	base = count_samples + mbrola_delay;

	for (int ix = 0; ix < event_list_ix; ix++) {
		espeak_EVENT *ep = &event_list[ix];
		int offset = ep->sample - base;

		if (offset <= 0)
			continue;
		if (offset > length_pre)
			offset = length_pre;
		// Both factors are bounded by the buffer length, so the product can
		// exceed 32 bits for buffers over about two seconds.  long is 32-bit on
		// Windows, so widen explicitly rather than relying on it.
		offset = (int)(((int64_t)offset * length_post) / length_pre);

		ep->sample = base + offset;
		ep->audio_position = (int)(((double)ep->sample * 1000.0) / samplerate);
	}
}

/* End legacy event rescaling. */
#else
void RescaleEventSamples(int length_pre, int length_post)
{
#if !USE_MBROLA
	static const int mbrola_delay = 0;
#endif
	espeak_rs_events_rescale(&espeak_rs_events, length_pre, length_post, count_samples, mbrola_delay, samplerate);
}
#endif
#endif

#ifdef USE_RUST_CORE
#if HAVE_AUDIO_OUTPUT
static void RustRequestDiagnostic(const char *operation, const char *message)
{
	fprintf(stderr, "audio %s error: %s\n", operation, message);
}
#endif
static const RustEngineRequest engine_request = {
	.capabilities = (USE_ASYNC ? 1 : 0) | (HAVE_AUDIO_OUTPUT ? 2 : 0),
	.value = espeak_rs_engine_value, .audio = espeak_rs_engine_audio,
	.init_text = InitText, .synthesize = Synthesize,
	.key = sync_espeak_Key, .character = sync_espeak_Char,
	.parameter = SetParameter, .punctuation = sync_espeak_SetPunctuationList,
#if USE_ASYNC
	.create = espeak_rs_async_command_create, .single = fifo_add_command,
	.pair = fifo_add_commands, .delete_command = delete_espeak_command,
#endif
	.identifier = &my_unique_identifier, .user = &my_user_data,
	.current = param_stack[0].parameter, .saved = saved_parameters,
	.skip = { &skip_characters, &skip_words, &skip_sentences },
	.skipping = &skipping_text, .end = &end_character_position, .marker = skip_marker,
#if HAVE_AUDIO_OUTPUT
	.flush = audio_object_flush, .drain = audio_object_drain,
	.audio_error = audio_object_strerror, .diagnose = RustRequestDiagnostic,
#endif
};
#endif

#ifndef USE_RUST_CORE
/* Begin retained request text. */
espeak_ng_STATUS sync_espeak_Synth(unsigned int unique_identifier, const void *text,
                                   unsigned int position, espeak_POSITION_TYPE position_type,
                                   unsigned int end_position, unsigned int flags, void *user_data)
{
	InitText(flags);
	my_unique_identifier = unique_identifier;
	my_user_data = user_data;

	for (int i = 0; i < N_SPEECH_PARAM; i++)
		saved_parameters[i] = param_stack[0].parameter[i];

	switch (position_type)
	{
	case POS_CHARACTER:
		skip_characters = position;
		break;
	case POS_WORD:
		skip_words = position;
		break;
	case POS_SENTENCE:
		skip_sentences = position;
		break;

	}
	if (skip_characters || skip_words || skip_sentences)
		skipping_text = true;

	end_character_position = end_position;

	espeak_ng_STATUS aStatus = Synthesize(unique_identifier, text, flags);
#if HAVE_AUDIO_OUTPUT
	if ((my_mode & ENOUTPUT_MODE_SPEAK_AUDIO) == ENOUTPUT_MODE_SPEAK_AUDIO) {
		int error = (aStatus == ENS_SPEECH_STOPPED)
		          ? audio_object_flush(my_audio)
		          : audio_object_drain(my_audio);
		if (error != 0)
			fprintf(stderr, "audio %s error: %s\n",
				(aStatus == ENS_SPEECH_STOPPED) ? "flush" : "drain",
				audio_object_strerror(my_audio, error));
	}
#endif

	return aStatus;
}
/* End retained request text. */
#else
espeak_ng_STATUS sync_espeak_Synth(unsigned int unique_identifier, const void *text,
                                   unsigned int position, espeak_POSITION_TYPE position_type,
                                   unsigned int end_position, unsigned int flags, void *user_data)
{
	t_espeak_text args = { unique_identifier, (void *)text, position, position_type, end_position, flags, user_data };
	return espeak_rs_request_synthesize(&engine_request, &args);
}
#endif

#ifndef USE_RUST_CORE
/* Begin retained request mark. */
espeak_ng_STATUS sync_espeak_Synth_Mark(unsigned int unique_identifier, const void *text,
                                        const char *index_mark, unsigned int end_position,
                                        unsigned int flags, void *user_data)
{
	InitText(flags);

	my_unique_identifier = unique_identifier;
	my_user_data = user_data;

	if (index_mark != NULL) {
		strncpy0(skip_marker, index_mark, sizeof(skip_marker));
		skipping_text = true;
	}

	end_character_position = end_position;

	return Synthesize(unique_identifier, text, flags | espeakSSML);
}
/* End retained request mark. */
#else
espeak_ng_STATUS sync_espeak_Synth_Mark(unsigned int unique_identifier, const void *text,
                                        const char *index_mark, unsigned int end_position,
                                        unsigned int flags, void *user_data)
{
	t_espeak_mark args = { unique_identifier, (void *)text, index_mark, end_position, flags, user_data };
	return espeak_rs_request_mark(&engine_request, &args);
}
#endif

espeak_ng_STATUS sync_espeak_Key(const char *key)
{
	// symbolic name, symbolicname_character  - is there a system resource of symbolic names per language?
	int letter;
	int ix;

	ix = utf8_in(&letter, key);
	if (key[ix] == 0) // a single character
		return sync_espeak_Char(letter);

	my_unique_identifier = 0;
	my_user_data = NULL;
	return Synthesize(0, key, 0); // speak key as a text string
}

espeak_ng_STATUS sync_espeak_Char(wchar_t character)
{
	// is there a system resource of character names per language?
	char buf[80];
	my_unique_identifier = 0;
	my_user_data = NULL;

	sprintf(buf, "<say-as interpret-as=\"tts:char\">&#%d;</say-as>", character);
	return Synthesize(0, buf, espeakSSML);
}

void sync_espeak_SetPunctuationList(const wchar_t *punctlist)
{
	// Set the list of punctuation which are spoken for "some".
	my_unique_identifier = 0;
	my_user_data = NULL;

	option_punctlist[0] = 0;
	if (punctlist != NULL) {
		wcsncpy(option_punctlist, punctlist, N_PUNCTLIST);
		option_punctlist[N_PUNCTLIST-1] = 0;
	}
}

#pragma GCC visibility push(default)

ESPEAK_API void espeak_SetSynthCallback(t_espeak_callback *SynthCallback)
{
	synth_callback = SynthCallback;
#if USE_ASYNC
	event_set_callback(synth_callback);
#endif
}

#ifndef USE_RUST_CORE
/* Begin retained request submit_text. */
ESPEAK_NG_API espeak_ng_STATUS
espeak_ng_Synthesize(const void *text, size_t size,
                     unsigned int position,
                     espeak_POSITION_TYPE position_type,
                     unsigned int end_position, unsigned int flags,
                     unsigned int *unique_identifier, void *user_data)
{
	(void)size; // unused in non-async modes

	unsigned int temp_identifier;

	if (unique_identifier == NULL)
		unique_identifier = &temp_identifier;
	*unique_identifier = 0;

	if (my_mode & ENOUTPUT_MODE_SYNCHRONOUS)
		return sync_espeak_Synth(0, text, position, position_type, end_position, flags, user_data);

#if USE_ASYNC
	// Create the text command
	t_espeak_command *c1 = create_espeak_text(text, size, position, position_type, end_position, flags, user_data);
	if (c1) {
		// Retrieve the unique identifier
		*unique_identifier = c1->u.my_text.unique_identifier;
	}

	// Create the "terminated msg" command (same uid)
	t_espeak_command *c2 = create_espeak_terminated_msg(*unique_identifier, user_data);

	// Try to add these 2 commands (single transaction)
	if (c1 && c2) {
		espeak_ng_STATUS status = fifo_add_commands(c1, c2);
		if (status != ENS_OK) {
			delete_espeak_command(c1);
			delete_espeak_command(c2);
		}
		return status;
	}

	delete_espeak_command(c1);
	delete_espeak_command(c2);
	return ENOMEM;
#else
	return sync_espeak_Synth(0, text, position, position_type, end_position, flags, user_data);
#endif
}
/* End retained request submit_text. */
#else
ESPEAK_NG_API espeak_ng_STATUS
espeak_ng_Synthesize(const void *text, size_t size,
                     unsigned int position,
                     espeak_POSITION_TYPE position_type,
                     unsigned int end_position, unsigned int flags,
                     unsigned int *unique_identifier, void *user_data)
{
	t_espeak_command args = { .type = ET_TEXT, .u.my_text = { 0, (void *)text, position, position_type, end_position, flags, user_data } };
	return espeak_rs_request_submit(&engine_request, &args, size, unique_identifier);
}
#endif

#ifndef USE_RUST_CORE
/* Begin retained request submit_mark. */
ESPEAK_NG_API espeak_ng_STATUS
espeak_ng_SynthesizeMark(const void *text,
                         size_t size,
                         const char *index_mark,
                         unsigned int end_position,
                         unsigned int flags,
                         unsigned int *unique_identifier,
                         void *user_data)
{
	(void)size; // unused in non-async modes

	unsigned int temp_identifier;

	if (unique_identifier == NULL)
		unique_identifier = &temp_identifier;
	*unique_identifier = 0;

	if (my_mode & ENOUTPUT_MODE_SYNCHRONOUS)
		return sync_espeak_Synth_Mark(0, text, index_mark, end_position, flags, user_data);

#if USE_ASYNC
	// Create the mark command
	t_espeak_command *c1 = create_espeak_mark(text, size, index_mark, end_position,
	                                          flags, user_data);
	if (c1) {
		// Retrieve the unique identifier
		*unique_identifier = c1->u.my_mark.unique_identifier;
	}

	// Create the "terminated msg" command (same uid)
	t_espeak_command *c2 = create_espeak_terminated_msg(*unique_identifier, user_data);

	// Try to add these 2 commands (single transaction)
	if (c1 && c2) {
		espeak_ng_STATUS status = fifo_add_commands(c1, c2);
		if (status != ENS_OK) {
			delete_espeak_command(c1);
			delete_espeak_command(c2);
		}
		return status;
	}

	delete_espeak_command(c1);
	delete_espeak_command(c2);
	return ENOMEM;
#else
	return sync_espeak_Synth_Mark(0, text, index_mark, end_position, flags, user_data);
#endif
}
/* End retained request submit_mark. */
#else
ESPEAK_NG_API espeak_ng_STATUS
espeak_ng_SynthesizeMark(const void *text,
                         size_t size,
                         const char *index_mark,
                         unsigned int end_position,
                         unsigned int flags,
                         unsigned int *unique_identifier,
                         void *user_data)
{
	t_espeak_command args = { .type = ET_MARK, .u.my_mark = { 0, (void *)text, index_mark, end_position, flags, user_data } };
	return espeak_rs_request_submit(&engine_request, &args, size, unique_identifier);
}
#endif

#ifndef USE_RUST_CORE
/* Begin retained request submit_key. */
ESPEAK_NG_API espeak_ng_STATUS espeak_ng_SpeakKeyName(const char *key_name)
{
	// symbolic name, symbolicname_character  - is there a system resource of symbolicnames per language

	if (my_mode & ENOUTPUT_MODE_SYNCHRONOUS)
		return sync_espeak_Key(key_name);

#if USE_ASYNC
	t_espeak_command *c = create_espeak_key(key_name, NULL);
	espeak_ng_STATUS status = fifo_add_command(c);
	if (status != ENS_OK)
		delete_espeak_command(c);
	return status;
#else
	return sync_espeak_Key(key_name);
#endif
}
/* End retained request submit_key. */
#else
ESPEAK_NG_API espeak_ng_STATUS espeak_ng_SpeakKeyName(const char *key_name)
{
	t_espeak_command args = { .type = ET_KEY, .u.my_key = { 0, NULL, key_name } };
	return espeak_rs_request_submit(&engine_request, &args, 0, NULL);
}
#endif

#ifndef USE_RUST_CORE
/* Begin retained request submit_character. */
ESPEAK_NG_API espeak_ng_STATUS espeak_ng_SpeakCharacter(wchar_t character)
{
	// is there a system resource of character names per language?

#if USE_ASYNC
	if (my_mode & ENOUTPUT_MODE_SYNCHRONOUS)
		return sync_espeak_Char(character);

	t_espeak_command *c = create_espeak_char(character, NULL);
	espeak_ng_STATUS status = fifo_add_command(c);
	if (status != ENS_OK)
		delete_espeak_command(c);
	return status;
#else
	return sync_espeak_Char(character);
#endif
}
/* End retained request submit_character. */
#else
ESPEAK_NG_API espeak_ng_STATUS espeak_ng_SpeakCharacter(wchar_t character)
{
	t_espeak_command args = { .type = ET_CHAR, .u.my_char = { 0, NULL, character } };
	return espeak_rs_request_submit(&engine_request, &args, 0, NULL);
}
#endif

ESPEAK_API int espeak_GetParameter(espeak_PARAMETER parameter, int current)
{
	// current: 0=default value, 1=current value
	if (current)
		return param_stack[0].parameter[parameter];
	return param_defaults[parameter];
}

#ifndef USE_RUST_CORE
/* Begin retained request submit_parameter. */
ESPEAK_NG_API espeak_ng_STATUS espeak_ng_SetParameter(espeak_PARAMETER parameter, int value, int relative)
{
#if USE_ASYNC
	if (my_mode & ENOUTPUT_MODE_SYNCHRONOUS)
		return SetParameter(parameter, value, relative);

	t_espeak_command *c = create_espeak_parameter(parameter, value, relative);

	espeak_ng_STATUS status = fifo_add_command(c);
	if (status != ENS_OK)
		delete_espeak_command(c);
	return status;
#else
	return SetParameter(parameter, value, relative);
#endif
}
/* End retained request submit_parameter. */
#else
ESPEAK_NG_API espeak_ng_STATUS espeak_ng_SetParameter(espeak_PARAMETER parameter, int value, int relative)
{
	t_espeak_command args = { .type = ET_PARAMETER, .u.my_param = { parameter, value, relative } };
	return espeak_rs_request_submit(&engine_request, &args, 0, NULL);
}
#endif

#ifndef USE_RUST_CORE
/* Begin retained request submit_punctuation. */
ESPEAK_NG_API espeak_ng_STATUS espeak_ng_SetPunctuationList(const wchar_t *punctlist)
{
	// Set the list of punctuation which are spoken for "some".

#if USE_ASYNC
	if (my_mode & ENOUTPUT_MODE_SYNCHRONOUS) {
		sync_espeak_SetPunctuationList(punctlist);
		return ENS_OK;
	}

	t_espeak_command *c = create_espeak_punctuation_list(punctlist);
	espeak_ng_STATUS status = fifo_add_command(c);
	if (status != ENS_OK)
		delete_espeak_command(c);
	return status;
#else
	sync_espeak_SetPunctuationList(punctlist);
	return ENS_OK;
#endif
}
/* End retained request submit_punctuation. */
#else
ESPEAK_NG_API espeak_ng_STATUS espeak_ng_SetPunctuationList(const wchar_t *punctlist)
{
	t_espeak_command args = { .type = ET_PUNCTUATION_LIST, .u.my_punctuation_list = punctlist };
	return espeak_rs_request_submit(&engine_request, &args, 0, NULL);
}
#endif

ESPEAK_API void espeak_SetPhonemeTrace(int phonememode, FILE *stream)
{
	/* phonememode:  Controls the output of phoneme symbols for the text
	      bits 0-2:
	         value=0  No phoneme output (default)
	         value=1  Output the translated phoneme symbols for the text
	         value=2  as (1), but produces IPA phoneme names rather than ascii
	      bit 3:   output a trace of how the translation was done (showing the matching rules and list entries)
	      bit 4:   produce pho data for mbrola
	      bit 7:   use (bits 8-23) as a tie within multi-letter phonemes names
	      bits 8-23:  separator character, between phoneme names

	   stream   output stream for the phoneme symbols (and trace).  If stream=NULL then it uses stdout.
	*/

	option_phonemes = phonememode;
	f_trans = stream;
	if (stream == NULL)
		f_trans = stderr;
}

ESPEAK_API const char* espeak_TextToPhonemesWithTerminator(const void** textptr, int textmode, int phonememode, int* terminator)
{
	/* phoneme_mode
	    bit 1:   0=eSpeak's ascii phoneme names, 1= International Phonetic Alphabet (as UTF-8 characters).
	    bit 7:   use (bits 8-23) as a tie within multi-letter phonemes names
	    bits 8-23:  separator character, between phoneme names
	 */

	if (p_decoder == NULL)
		p_decoder = create_text_decoder();

	if (text_decoder_decode_string_multibyte(p_decoder, *textptr, translator->encoding, textmode) != ENS_OK)
		return NULL;

	TranslateClauseWithTerminator(translator, NULL, NULL, terminator);
	*textptr = text_decoder_get_buffer(p_decoder);

	return GetTranslatedPhonemeString(phonememode);
}

ESPEAK_API const char *espeak_TextToPhonemes(const void **textptr, int textmode, int phonememode)
{
	return espeak_TextToPhonemesWithTerminator(textptr, textmode, phonememode, NULL);
}

#ifndef USE_RUST_CORE
/* Begin retained engine cancellation. */
ESPEAK_NG_API espeak_ng_STATUS espeak_ng_Cancel(void)
{
#if USE_ASYNC
	fifo_stop();
	event_clear_all();
#endif

#if HAVE_AUDIO_OUTPUT
	if ((my_mode & ENOUTPUT_MODE_SPEAK_AUDIO) == ENOUTPUT_MODE_SPEAK_AUDIO)
		audio_object_flush(my_audio);
#endif
	embedded_value[EMBED_T] = 0; // reset echo for pronunciation announcements

	for (int i = 0; i < N_SPEECH_PARAM; i++)
		SetParameter(i, saved_parameters[i], 0);

	return ENS_OK;
}
/* End retained engine cancellation. */
#else
ESPEAK_NG_API espeak_ng_STATUS espeak_ng_Cancel(void)
{
	return espeak_rs_engine_cancel(&engine_lifecycle);
}
#endif

ESPEAK_API int espeak_IsPlaying(void)
{
#if USE_ASYNC
	return fifo_is_busy();
#else
	return 0;
#endif
}

#if !defined(USE_RUST_CORE) || (USE_ASYNC && !USE_PROACTOR)
/* Begin retained engine synchronization. */
ESPEAK_NG_API espeak_ng_STATUS espeak_ng_Synchronize(void)
{
	espeak_ng_STATUS berr = err;
#if USE_ASYNC
#if USE_PROACTOR
	if (fifo_synchronize() != ENS_OK)
		return EINVAL;
#else
	while (espeak_IsPlaying())
		usleep(20000);
#endif
#endif
	ENGINE_STORE(RUST_ENGINE_ERROR, err, ENS_OK);
	return berr;
}
/* End retained engine synchronization. */
#else
ESPEAK_NG_API espeak_ng_STATUS espeak_ng_Synchronize(void)
{
	return espeak_rs_engine_synchronize(&engine_lifecycle);
}
#endif

#ifndef USE_RUST_CORE
/* Begin retained engine termination. */
ESPEAK_NG_API espeak_ng_STATUS espeak_ng_Terminate(void)
{
#if USE_ASYNC
	fifo_stop();
	fifo_terminate();
	event_terminate();
#endif

	if ((my_mode & ENOUTPUT_MODE_SPEAK_AUDIO) == ENOUTPUT_MODE_SPEAK_AUDIO) {
#if HAVE_AUDIO_OUTPUT
		audio_object_close(my_audio);
		audio_object_destroy(my_audio);
		my_audio = NULL;
#endif
		out_samplerate = 0;
	}

#ifndef USE_RUST_CORE
	free(event_list);
	event_list = NULL;
#else
	espeak_rs_events_release(&espeak_rs_events);
#endif

#ifndef USE_RUST_CORE
	free(outbuf);
	outbuf = NULL;
#else
	espeak_rs_output_release(&espeak_rs_output);
#endif

	FreePhData();
	FreeVoiceList();
	FreeCurrentVoice();

	DeleteTranslator(translator);
	translator = NULL;
#ifdef USE_RUST_CORE
	FreeAlternateTranslators();
	FreeDictionaryCache();
#endif

	if (p_decoder != NULL) {
		destroy_text_decoder(p_decoder);
		p_decoder = NULL;
	}

	WavegenFini();
#ifdef USE_RUST_CORE
	FreeNamedata();
#endif
	FreeSoundIcons();
#if USE_MBROLA
	FreeMbrolaTable();
#endif

	return ENS_OK;
}
/* End retained engine termination. */
#else
ESPEAK_NG_API espeak_ng_STATUS espeak_ng_Terminate(void)
{
	return espeak_rs_engine_terminate(&engine_lifecycle);
}
#endif

static const char version_string[] = PACKAGE_VERSION;
ESPEAK_API const char *espeak_Info(const char **ptr)
{
	if (ptr != NULL)
		*ptr = path_home;
	return version_string;
}

#pragma GCC visibility pop
