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
static int reference_modulation, reference_pause;
#define modn_flags reference_modulation
#define RMS_GLOTTAL1 35
#define RMS_START 28
#define VOWEL_FRONT_LENGTH 50
static void ReferencePause(int milliseconds, int control) { TEST_ASSERT(control == 0); reference_pause += milliseconds; }
#define DoPause ReferencePause
#define FormantTransition2 ReferenceFormantTransition
#include "formant_reference.inc"
#undef DoPause
#undef modn_flags
#define LookupSpect ReferenceLookupSpect
#define GetEnvelope ReferenceGetEnvelope
#include "spectrum_reference.inc"
#undef LookupSpect
#undef GetEnvelope
#undef FormantTransition2
static intptr_t reference_queue[N_WCMDQ][4];
static int reference_start, reference_end, reference_centre;
#define wcmdq reference_queue
#define syllable_start reference_start
#define syllable_end reference_end
#define syllable_centre reference_centre
#define SmoothSpect ReferenceSmoothSpect
#include "smoothing_reference.inc"
#undef wcmdq
#undef syllable_start
#undef syllable_end
#undef syllable_centre
#undef SmoothSpect

static uint32_t seed = 0xc24ad572;
static uint32_t random32(void) { seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5; return seed; }
static unsigned long comparisons;
typedef struct { frame_t frames[N_WCMDQ]; size_t cursor; } NativePool;
static frame_t *native_pool(void *opaque, uint32_t kind, frame_t *frame)
{
	NativePool *pool = opaque;
	if (kind == 0) { pool->cursor = (pool->cursor+1)%N_WCMDQ; return &pool->frames[pool->cursor]; }
	uintptr_t address = (uintptr_t)frame, base = (uintptr_t)pool->frames;
	if (kind == 1 && address >= base && address-base < sizeof(pool->frames) && (address-base)%sizeof(frame_t) == 0) return frame;
	return NULL;
}
static void direct_smoothing(void)
{
	NativePool pool = {0};
	int saved_rates[6]; memcpy(saved_rates,formant_rate,sizeof(saved_rates));
	unsigned long trials = 0;
	for (int trial = 0; trial < 30000; trial++) {
		frame_t a[N_WCMDQ], b[N_WCMDQ];
		for (size_t byte = 0; byte < sizeof(a); byte++) ((unsigned char *)a)[byte] = random32();
		for (int ix = 0; ix < N_WCMDQ; ix++) {
			a[ix].frflags = (trial & 1 ? FRFLAG_KLATT : 0) |
			    (trial%13 != 0 && random32()%9 == 0 ? FRFLAG_BREAK : 0) |
			    (random32()%7 == 0 ? FRFLAG_BREAK_LF : 0) |
			    (random32()%3 == 0 ? FRFLAG_FORMANT_RATE : 0);
			for (int peak = 0; peak < 7; peak++) a[ix].ffreq[peak] = (int)(random32()%7001)-1000;
		}
		memcpy(b,a,sizeof(b));
		intptr_t actual[N_WCMDQ][4];
		for (int ix = 0; ix < N_WCMDQ; ix++) {
			reference_queue[ix][0] = actual[ix][0] = WCMD_PITCH;
			reference_queue[ix][1] = actual[ix][1] = 456;
			reference_queue[ix][2] = actual[ix][2] = 123;
			reference_queue[ix][3] = actual[ix][3] = 789;
		}
		int start = random32()%N_WCMDQ, span = 1+random32()%(trial%17 == 0 ? N_WCMDQ-1 : 60);
		int end = (start+span)%N_WCMDQ, centre = trial%11 == 0 ? -1 : (start+random32()%span)%N_WCMDQ;
		for (int step = 0; step < span; step++) {
			int ix = (start+step)%N_WCMDQ;
			int kind = trial%3 == 0 && random32()%8 == 0 ? WCMD_PAUSE+random32()%4 : WCMD_KLATT+random32()%4;
			reference_queue[ix][0] = actual[ix][0] = kind;
			reference_queue[ix][1] = actual[ix][1] = (random32()&0xffff0000u) | (random32()%2049);
			if (kind <= WCMD_SPECT2) {
				int first = step, last = step+1;
				if (trial%5 == 0 && random32()%8 == 0) last = random32()%N_WCMDQ;
				reference_queue[ix][2] = (intptr_t)&a[first]; reference_queue[ix][3] = (intptr_t)&a[last];
				actual[ix][2] = (intptr_t)&b[first]; actual[ix][3] = (intptr_t)&b[last];
			}
		}
		for (int peak = 0; peak < 6; peak++) formant_rate[peak] = random32()%501;
		for (int pass = 0; pass < 2; pass++) {
			reference_start = start; reference_end = end; reference_centre = centre;
			ReferenceSmoothSpect();
			int actual_start = start;
			TEST_ASSERT(espeak_rs_smooth_spectrum(actual,N_WCMDQ,&actual_start,end,centre,formant_rate,&pool,native_pool) == 0);
			TEST_ASSERT(actual_start == reference_start);
			for (int ix = 0; ix < N_WCMDQ; ix++) {
				TEST_ASSERT(actual[ix][0] == reference_queue[ix][0] && actual[ix][1] == reference_queue[ix][1]);
				int next = (ix+1)%N_WCMDQ;
				if (actual[ix][0] <= WCMD_SPECT2 && actual[next][0] <= WCMD_SPECT2)
					TEST_ASSERT((actual[ix][3] == actual[next][2]) == (reference_queue[ix][3] == reference_queue[next][2]));
				for (int word = 2; word < 4; word++) {
					if (actual[ix][0] > WCMD_SPECT2) { TEST_ASSERT(actual[ix][word] == reference_queue[ix][word]); continue; }
					frame_t *expected = (frame_t *)reference_queue[ix][word], *frame = (frame_t *)actual[ix][word];
					size_t bytes = expected->frflags & FRFLAG_KLATT ? 64 : 44;
					if (memcmp(expected,frame,bytes)) {
						fprintf(stderr,"smoothing mismatch trial=%d pass=%d index=%d word=%d start=%d end=%d centre=%d\n",trial,pass,ix,word,start,end,centre);
						TEST_ASSERT(false);
					}
				}
			}
			trials++;
		}
	}
	memcpy(formant_rate,saved_rates,sizeof(saved_rates));
	int start = 0; size_t cursor = pool.cursor;
	TEST_ASSERT(espeak_rs_smooth_spectrum(reference_queue,N_WCMDQ+1,&start,1,0,formant_rate,&pool,native_pool) == 2);
	TEST_ASSERT(espeak_rs_smooth_spectrum(reference_queue,N_WCMDQ,&start,2,3,formant_rate,&pool,native_pool) == 2);
	TEST_ASSERT(start == 0 && pool.cursor == cursor);
	printf("Compared %lu native smoothing passes over wrapped rings, reuse, breaks and discontinuities\n",trials);
}
static void direct_formant_transitions(void)
{
	NativePool pool = {0};
	int old_klatt = voice->klattv[0], old_factor = voice->formant_factor;
	unsigned long trials = 0;
	for (int trial = 0; trial < 100000; trial++) {
		frame_t original[6], a[6], b[6];
		for (size_t ix = 0; ix < sizeof(original); ix++) ((unsigned char *)original)[ix] = random32();
		for (int ix = 0; ix < 6; ix++) {
			original[ix].frflags = trial & 1 ? FRFLAG_KLATT : 0;
			for (int f = 0; f < 7; f++) original[ix].ffreq[f] = trial & 8 ? (int16_t)random32() : (int)(random32()%6001)-1000;
		}
		memcpy(a,original,sizeof(a)); memcpy(b,original,sizeof(b));
		frameref_t expected[N_SEQ_FRAMES] = {0}, actual[N_SEQ_FRAMES] = {0};
		for (int ix = 0; ix < N_SEQ_FRAMES; ix++) {
			expected[ix].frflags = actual[ix].frflags = random32();
			expected[ix].length = actual[ix].length = random32();
			expected[ix].frame = &a[ix%6]; actual[ix].frame = &b[ix%6];
		}
		int expected_count = trial%6, actual_count = expected_count;
		voice->klattv[0] = trial & 2 ? 1 : 0;
		voice->formant_factor = 128+random32()%385;
		for (int pass = 0; pass < 2; pass++) {
			unsigned int data1 = random32(), data2 = random32();
			int which = trial%3, adjustment = (int)(random32()%1001)-500;
			PHONEME_TAB other = {0}; other.mnemonic = trial & 4 ? '?' : 'x';
			reference_modulation = 123; reference_pause = 0; seq_len_adjust = adjustment;
			int expected_return = ReferenceFormantTransition(expected,&expected_count,data1,data2,&other,which);
			int expected_adjust = seq_len_adjust;
			RustFormantSettings settings = { .which = which, .klatt = voice->klattv[0] != 0,
			    .formant_factor = voice->formant_factor, .other_glottal = other.mnemonic == '?', .length_adjust = adjustment };
			RustFormantEffects effects;
			TEST_ASSERT(espeak_rs_formant_transition(actual,N_SEQ_FRAMES,&actual_count,data1,data2,&settings,&pool,native_pool,&effects) == 0);
			TEST_ASSERT(expected_return == effects.return_length && expected_adjust == effects.length_adjust);
			TEST_ASSERT(reference_pause == (int)effects.pause);
			TEST_ASSERT(reference_modulation == (effects.has_modulation ? effects.modulation : 123));
			TEST_ASSERT(expected_count == actual_count);
			for (int ix = 0; ix < actual_count; ix++) {
				TEST_ASSERT(expected[ix].length == actual[ix].length && expected[ix].frflags == actual[ix].frflags);
				// Ordinary records have no meaningful Klatt extension. The native
				// pool clears it; legacy C copied the following allocation bytes.
				size_t length = expected[ix].frame->frflags & FRFLAG_KLATT ? 64 : 44;
				if ((actual[ix].frame->frflags & (FRFLAG_COPIED|FRFLAG_KLATT)) == FRFLAG_COPIED)
					for (size_t byte = 44; byte < 64; byte++) TEST_ASSERT(((unsigned char *)actual[ix].frame)[byte] == 0);
				if (memcmp(expected[ix].frame,actual[ix].frame,length)) {
					fprintf(stderr,"formant mismatch trial=%d pass=%d index=%d which=%d d1=%x d2=%x\n",trial,pass,ix,which,data1,data2);
					for (size_t byte = 0; byte < length; byte++) if (((unsigned char *)expected[ix].frame)[byte] != ((unsigned char *)actual[ix].frame)[byte])
						fprintf(stderr,"byte %zu: %u/%u\n",byte,((unsigned char *)expected[ix].frame)[byte],((unsigned char *)actual[ix].frame)[byte]);
					TEST_ASSERT(false);
				}
			}
			trials++;
		}
	}
	voice->klattv[0] = old_klatt; voice->formant_factor = old_factor;
	frame_t forged = { .frflags = FRFLAG_COPIED };
	size_t cursor = pool.cursor;
	TEST_ASSERT(espeak_rs_frame_copy(&forged,0,&pool,native_pool) == NULL);
	TEST_ASSERT(pool.cursor == cursor);
	frameref_t invalid[2] = {{.frame = &forged},{.frame = &forged}};
	int invalid_count = 2;
	RustFormantSettings settings = { .which = 1, .formant_factor = 256 };
	RustFormantEffects effects;
	TEST_ASSERT(espeak_rs_formant_transition(invalid,1,&invalid_count,0,0,&settings,&pool,native_pool,&effects) == 2);
	TEST_ASSERT(invalid_count == 2 && pool.cursor == cursor);
	TEST_ASSERT(espeak_rs_formant_transition(invalid,2,&invalid_count,0,0,&settings,&pool,native_pool,&effects) == 2);
	TEST_ASSERT(pool.cursor == cursor);
	printf("Compared %lu native formant transitions, reused copies, modulation and pause effects\n",trials);
}
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
	TEST_ASSERT(sizeof(RustFormantSettings) == 20 && sizeof(RustFormantEffects) == 20);
	TEST_ASSERT(espeak_Initialize(AUDIO_OUTPUT_RETRIEVAL,0,NULL,0) == 22050);
	TEST_ASSERT(espeak_SetVoiceByName("en") == EE_OK);
	real_spectra();
	direct_formant_transitions();
	direct_smoothing();
	exact_short_frame_and_readonly_guard();
	TEST_ASSERT(espeak_Terminate() == EE_OK);
	return 0;
}
