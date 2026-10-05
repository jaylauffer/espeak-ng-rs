/* Spectrum selection against retained C, including live formant blending.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include "test_assert.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <espeak-ng/speak_lib.h>
#include "common.h"
#include "speech.h"
#include "phoneme.h"
#include "synthesize.h"
#include "synthdata.h"
#include "translate.h"
#include "voice.h"
#include "rust_data.h"
static char *phondata_ptr;
extern int seq_len_adjust;
#define LookupSpect ReferenceLookupSpect
#define GetEnvelope ReferenceGetEnvelope
#include "spectrum_reference.inc"
#undef LookupSpect
#undef GetEnvelope

static uint32_t seed = 0xc24ad572;
static uint32_t random32(void) { seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5; return seed; }
static unsigned long comparisons;
static int transitions;
static int transition(void *opaque, frameref_t *frames, int *count, const FMT_PARAMS *parameters, int which, int *adjust, size_t capacity)
{
	(void)opaque;
	TEST_ASSERT(*count >= 2 && (size_t)*count < capacity);
	transitions++;
	seq_len_adjust = *adjust;
	int result = FormantTransition2(frames,count,parameters->transition0,parameters->transition1,NULL,which);
	*adjust = seq_len_adjust;
	return result;
}
static void exact_short_frame_and_readonly_guard(void)
{
	// Last ordinary frame ends exactly at EOF; its Klatt extension is absent.
	_Alignas(4) unsigned char data[100] = {0};
	data[10] = 2;
	data[28] = 20;
	data[29] = data[73] = 100;
	FMT_PARAMS parameters = { .fmt_addr = 8, .use_vowelin = 1,
	    .std_length = 95, .transition0 = 25 | (16 << 6) | (2 << 12), .transition1 = 30 };
	RustSpectrumSettings settings = { .which = 2, .is_vowel = 1 };
	RustSpectrumSelection selected;
	frameref_t frames[N_SEQ_FRAMES] = {0};
	TEST_ASSERT(espeak_rs_spectrum_lookup(data,sizeof(data),&parameters,&settings,NULL,transition,frames,&selected) == 0);
	TEST_ASSERT(selected.count == 3 && transitions == 1);
	TEST_ASSERT(frames[2].frame->frflags & FRFLAG_COPIED);
	for (size_t ix = sizeof(frame_t2); ix < sizeof(frame_t); ix++)
		TEST_ASSERT(((unsigned char *)frames[2].frame)[ix] == 0);
	data[13] = 0x80; // A resident frame must never masquerade as a writable pool copy.
	TEST_ASSERT(espeak_rs_spectrum_lookup(data,sizeof(data),&parameters,&settings,NULL,transition,frames,&selected) == 2);
	TEST_ASSERT(transitions == 1);
}
static void compare(PHONEME_TAB *ph, PHONEME_LIST *entry, int which, FMT_PARAMS *parameters, size_t bytes)
{
	int expected_count, actual_count;
	frameref_t *reference = ReferenceLookupSpect(ph,which,parameters,&expected_count,entry);
	int expected_adjust = seq_len_adjust;
	frameref_t expected[N_SEQ_FRAMES];
	unsigned char records[N_SEQ_FRAMES][64];
	memcpy(expected,reference,expected_count*sizeof(*expected));
	for (int ix = 0; ix < expected_count; ix++)
		memcpy(records[ix],expected[ix].frame,expected[ix].frame->frflags & FRFLAG_KLATT ? 64 : 44);
	frameref_t *actual = LookupSpect(ph,which,parameters,&actual_count,entry);
	if (actual == NULL || actual_count != expected_count || seq_len_adjust != expected_adjust) {
		fprintf(stderr,"spectrum mismatch addr=%x suffix=%x which=%d count=%d/%d adjust=%d/%d\n",parameters->fmt_addr,parameters->fmt2_addr,which,actual_count,expected_count,seq_len_adjust,expected_adjust);
		TEST_ASSERT(false);
	}
	for (int ix = 0; ix < actual_count; ix++) {
		TEST_ASSERT(actual[ix].length == expected[ix].length);
		TEST_ASSERT(actual[ix].frflags == expected[ix].frflags);
		TEST_ASSERT(memcmp(records[ix],actual[ix].frame,actual[ix].frame->frflags & FRFLAG_KLATT ? 64 : 44) == 0);
		uintptr_t pointer = (uintptr_t)expected[ix].frame, base = (uintptr_t)phondata_ptr;
		if (pointer >= base && pointer - base < bytes)
			TEST_ASSERT((uintptr_t)actual[ix].frame - (uintptr_t)wavefile_data == pointer - base);
	}
	comparisons++;
}
static void real_spectra(void)
{
	char path[N_PATH_BUF];
	snprintf(path,sizeof(path),"%s/phondata",path_home);
	FILE *data = fopen(path,"rb"); TEST_ASSERT(data != NULL);
	TEST_ASSERT(fseek(data,0,SEEK_END) == 0);
	long bytes = ftell(data); TEST_ASSERT(bytes > 8);
	rewind(data);
	// Retain padding for the C oracle's old short-frame overreads only.
	phondata_ptr = calloc(1,bytes+64); TEST_ASSERT(phondata_ptr != NULL);
	TEST_ASSERT(fread(phondata_ptr,1,bytes,data) == (size_t)bytes); fclose(data);
	snprintf(path,sizeof(path),"%s/phondata-manifest",path_home);
	FILE *manifest = fopen(path,"r"); TEST_ASSERT(manifest != NULL);
	unsigned int sequences[4096], envelopes[1024]; int count = 0, envelope_count = 0;
	char line[512], kind; unsigned int address;
	while (fgets(line,sizeof(line),manifest)) {
		if (sscanf(line,"%c 0x%x",&kind,&address) != 2) continue;
		if (kind == 'S') { TEST_ASSERT(count < 4096); sequences[count++] = address; }
		if (kind == 'E') { TEST_ASSERT(envelope_count < 1024); envelopes[envelope_count++] = address; }
	}
	fclose(manifest); TEST_ASSERT(count > 100 && envelope_count > 0);
	int klatt = voice->klattv[0];
	for (int seq = 0; seq < count; seq++) for (int trial = 0; trial < 60; trial++) {
		unsigned int suffix = sequences[random32()%count];
		SPECT_SEQ *main = (SPECT_SEQ *)(phondata_ptr+sequences[seq]);
		SPECT_SEQ *secondary = (SPECT_SEQ *)(phondata_ptr+suffix);
		TEST_ASSERT(main->n_frames > 0 && main->n_frames < N_SEQ_FRAMES);
		FMT_PARAMS parameters = {0};
		parameters.fmt_addr = sequences[seq];
		parameters.fmt_length = (int)(random32()%61)-30;
		parameters.fmt2_lenadj = (int)(random32()%41)-20;
		parameters.std_length = trial%5 == 0 ? 0 : 50+random32()%251;
		parameters.fmt_control = trial & 1;
		if (trial%3 == 0 && main->n_frames+secondary->n_frames-1 <= N_SEQ_FRAMES)
			parameters.fmt2_addr = suffix;
		PHONEME_TAB ph = {0}; ph.type = trial & 2 ? phVOWEL : phLIQUID;
		PHONEME_LIST entry = {0}; entry.synthflags = trial & 4 ? SFLAG_LENGTHEN : 0;
		parameters.use_vowelin = trial & 8;
		static const int flags[] = {0,2,4,8,16,32};
		parameters.transition0 = 25 | (16 << 6) | (flags[trial%6] << 12);
		parameters.transition1 = 30 | (13 << 6) | (18 << 11) | (16 << 16) | (12 << 21) | ((trial%4) << 26) | ((trial%3) << 29);
		voice->klattv[0] = trial & 16 ? 1 : 0;
		compare(&ph,&entry,trial%3,&parameters,bytes);
	}
	voice->klattv[0] = klatt;
	for (int ix = 0; ix < envelope_count; ix++) {
		const unsigned char *actual = GetEnvelope(envelopes[ix]);
		TEST_ASSERT(actual == wavefile_data+envelopes[ix]);
		TEST_ASSERT(memcmp(actual,ReferenceGetEnvelope(envelopes[ix]),ENV_LEN) == 0);
	}
	TEST_ASSERT(GetEnvelope(0) == envelope_data[0]);
	TEST_ASSERT(GetEnvelope(-1) == envelope_data[0]);
	TEST_ASSERT(GetEnvelope(bytes-127) == envelope_data[0]);
	FMT_PARAMS invalid = {0}; invalid.fmt_addr = bytes+4;
	PHONEME_TAB ph = {0}; PHONEME_LIST entry = {0}; int frames = 99;
	TEST_ASSERT(LookupSpect(&ph,0,&invalid,&frames,&entry) == NULL && frames == 0);
	invalid.fmt_addr = sequences[0]; invalid.fmt2_addr = bytes;
	TEST_ASSERT(LookupSpect(&ph,0,&invalid,&frames,&entry) == NULL && frames == 0);
	printf("Compared %lu spectrum selections/blends across %d records and %d envelopes\n",comparisons,count,envelope_count);
	free(phondata_ptr);
}
int main(void)
{
	TEST_ASSERT(sizeof(frame_t) == 64 && sizeof(frame_t2) == 44);
	TEST_ASSERT(sizeof(FMT_PARAMS) == 48);
	TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_RETRIEVAL,0,NULL,0) == 22050);
	TEST_ASSERT(espeak_SetVoiceByName("en") == EE_OK);
	real_spectra();
	exact_short_frame_and_readonly_guard();
	TEST_ASSERT(espeak_Terminate() == EE_OK);
	return 0;
}
