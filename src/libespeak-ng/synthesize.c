/*
 * Copyright (C) 2005 to 2014 by Jonathan Duddington
 * email: jonsd@users.sourceforge.net
 * Copyright (C) 2015-2017 Reece H. Dunn
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

#include <ctype.h>
#include <errno.h>
#include <math.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <espeak-ng/espeak_ng.h>
#include <espeak-ng/speak_lib.h>
#include <espeak-ng/encoding.h>

#include "synthesize.h"
#include "dictionary.h"           // for WritePhMnemonic, GetTranslatedPhone...
#include "intonation.h"           // for CalcPitches
#include "mbrola.h"               // for MbrolaGenerate, mbrola_name
#include "phoneme.h"              // for PHONEME_TAB, phVOWEL, phLIQUID, phN...
#include "setlengths.h"           // for CalcLengths
#include "soundicon.h"               // for soundicon_tab, n_soundicon
#include "synthdata.h"            // for InterpretPhoneme, GetEnvelope, Inte...
#include "translate.h"            // for translator, LANGUAGE_OPTIONS, Trans...
#include "voice.h"                // for voice_t, voice, LoadVoiceVariant
#include "wavegen.h"              // for WcmdqInc, WcmdqFree, WcmdqStop
#ifdef USE_RUST_CORE
#include "rust_data.h"
#endif
#include "speech.h"               // for MAKE_MEM_UNDEFINED

#ifndef USE_RUST_CORE
static void SmoothSpect(void);
#endif

// list of phonemes in a clause
int n_phoneme_list = 0;
PHONEME_LIST phoneme_list[N_PHONEME_LIST+1];

SPEED_FACTORS speed;

#ifndef USE_RUST_CORE
static int last_pitch_cmd;
static int last_amp_cmd;
static frame_t  *last_frame;
static int last_wcmdq;
static int pitch_length;
static int amp_length;
static int fmt_amplitude = 0;

static int syllable_start;
static int syllable_end;
static int syllable_centre;
#else
// Shared by the command writers, which run in Rust.
static RustCommandState command_state;
#define last_pitch_cmd (command_state.last_pitch_cmd)
#define last_amp_cmd (command_state.last_amp_cmd)
#define last_frame (command_state.last_frame)
#define last_wcmdq (command_state.last_wcmdq)
#define pitch_length (command_state.pitch_length)
#define amp_length (command_state.amp_length)
#define fmt_amplitude (command_state.fmt_amplitude)
#define syllable_start (command_state.syllable_start)
#define syllable_end (command_state.syllable_end)
#define syllable_centre (command_state.syllable_centre)
#endif
static int modn_flags;

static voice_t *new_voice = NULL;

static int (*phoneme_callback)(const char *) = NULL;

#define RMS_GLOTTAL1 35   // vowel before glottal stop
#define RMS_START 28  // 28
#define VOWEL_FRONT_LENGTH  50

const char *WordToString(char buf[5], unsigned int word)
{
	// Convert a phoneme mnemonic word into a string
	int ix;

	for (ix = 0; ix < 4; ix++)
		buf[ix] = word >> (ix*8);
	buf[4] = 0;
	return buf;
}

void SynthesizeInit(void)
{
	last_pitch_cmd = 0;
	last_amp_cmd = 0;
	last_frame = NULL;
	syllable_centre = -1;
}

extern int seq_len_adjust; // temporary fix to advance the start point for playing the wav sample

#ifndef USE_RUST_CORE
static void DoPhonemeAlignment(char* pho, int type)
{
	wcmdq[wcmdq_tail][0] = WCMD_PHONEME_ALIGNMENT;
	wcmdq[wcmdq_tail][1] = (intptr_t)pho;
	wcmdq[wcmdq_tail][2] = type;
	WcmdqInc();
}

/* End legacy phoneme alignment. */
#else
static void DoPhonemeAlignment(char* pho, int type)
{
	espeak_rs_queue_phoneme_alignment(&espeak_rs_wave_memory, pho, type);
}
#endif

#ifndef USE_RUST_CORE
static void EndAmplitude(void)
{
	if (amp_length > 0) {
		if (wcmdq[last_amp_cmd][1] == 0)
			wcmdq[last_amp_cmd][1] = amp_length;
		amp_length = 0;
	}
}

static void EndPitch(int voice_break)
{
	// possible end of pitch envelope, fill in the length
	if ((pitch_length > 0) && (last_pitch_cmd >= 0)) {
		if (wcmdq[last_pitch_cmd][1] == 0)
			wcmdq[last_pitch_cmd][1] = pitch_length;
		pitch_length = 0;
	}

	if (voice_break) {
		last_wcmdq = -1;
		last_frame = NULL;
		syllable_end = wcmdq_tail;
		SmoothSpect();
		syllable_centre = -1;
	}
}

static void DoAmplitude(int amp, const unsigned char *amp_env)
{
	intptr_t *q;

	last_amp_cmd = wcmdq_tail;
	amp_length = 0; // total length of vowel with this amplitude envelope

	q = wcmdq[wcmdq_tail];
	q[0] = WCMD_AMPLITUDE;
	q[1] = 0; // fill in later from amp_length
	q[2] = (intptr_t)amp_env;
	q[3] = amp;
	WcmdqInc();
}

static void DoPitch(const unsigned char *env, int pitch1, int pitch2)
{
	intptr_t *q;

	EndPitch(0);

	if (pitch1 == 255) {
		// pitch was not set
		pitch1 = 55;
		pitch2 = 76;
		env = envelope_data[PITCHfall];
	}
	last_pitch_cmd = wcmdq_tail;
	pitch_length = 0; // total length of spect with this pitch envelope

	if (pitch2 < 0)
		pitch2 = 0;

	q = wcmdq[wcmdq_tail];
	q[0] = WCMD_PITCH;
	q[1] = 0; // length, fill in later from pitch_length
	q[2] = (intptr_t)env;
	q[3] = (pitch1 << 16) + pitch2;
	WcmdqInc();
}

int PauseLength(int pause, int control)
{
	unsigned int len;

	if (control == 0) {
		if (pause >= 200)
			len = (pause * speed.clause_pause_factor)/256;
		else
			len = (pause * speed.pause_factor)/256;
	} else
		len = (pause * speed.wav_factor)/256;

	if (len < speed.min_pause)
		len = speed.min_pause; // mS, limit the amount to which pauses can be shortened
	return len;
}

static void DoPause(int length, int control)
{
	// length in nominal mS
	// control = 1, less shortening at fast speeds

	unsigned int len;

	if (length == 0)
		len = 0;
	else {
		len = PauseLength(length, control);

		if (len < 90000)
			len = (len * samplerate) / 1000; // convert from mS to number of samples
		else {
			int srate2 = samplerate / 25; // avoid overflow
			len = (len * srate2) / 40;
		}
	}

	EndPitch(1);
	wcmdq[wcmdq_tail][0] = WCMD_PAUSE;
	wcmdq[wcmdq_tail][1] = len;
	WcmdqInc();
	last_frame = NULL;

	if (fmt_amplitude != 0) {
		wcmdq[wcmdq_tail][0] = WCMD_FMT_AMPLITUDE;
		wcmdq[wcmdq_tail][1] = fmt_amplitude = 0;
		WcmdqInc();
	}
}

static int DoSample2(int index, int which, int std_length, int control, int length_mod, int amp)
{
	int length;
	int wav_length;
	int wav_scale;
	int min_length;
	int x;
	int len4;
	intptr_t *q;
	unsigned char *p;

	index = index & 0x7fffff;
	p = &wavefile_data[index];
	wav_scale = p[2];
	wav_length = (p[1] * 256);
	wav_length += p[0]; // length in bytes

	if (wav_length == 0)
		return 0;

	min_length = speed.min_sample_len;

	if (wav_scale == 0)
		min_length *= 2; // 16 bit samples

	if (std_length > 0) {
		std_length = (std_length * samplerate)/1000;
		if (wav_scale == 0)
			std_length *= 2;

		x = (min_length * std_length)/wav_length;
		if (x > min_length)
			min_length = x;
	} else {
		// no length specified, use the length of the stored sound
		std_length = wav_length;
	}

	if (length_mod > 0)
		std_length = (std_length * length_mod)/256;

	length = (std_length * speed.wav_factor)/256;

	if (control & pd_DONTLENGTHEN) {
		// this option is used for Stops, with short noise bursts.
		// Don't change their length much.
		if (length > std_length) {
			// don't let length exceed std_length
			length = std_length;
		}
	}

	if (length < min_length)
		length = min_length;


	if (wav_scale == 0) {
		// 16 bit samples
		length /= 2;
		wav_length /= 2;
	}

	if (amp < 0)
		return length;

	len4 = wav_length / 4;

	index += 4;

	if (which & 0x100) {
		// mix this with synthesised wave
		last_wcmdq = wcmdq_tail;
		q = wcmdq[wcmdq_tail];
		q[0] = WCMD_WAVE2;
		q[1] = length | (wav_length << 16); // length in samples
		q[2] = (intptr_t)(&wavefile_data[index]);
		q[3] = wav_scale + (amp << 8);
		WcmdqInc();
		return length;
	}

	if (length > wav_length) {
		x = len4*3;
		length -= x;
	} else {
		x = length;
		length = 0;
	}

	last_wcmdq = wcmdq_tail;
	q = wcmdq[wcmdq_tail];
	q[0] = WCMD_WAVE;
	q[1] = x; // length in samples
	q[2] = (intptr_t)(&wavefile_data[index]);
	q[3] = wav_scale + (amp << 8);
	WcmdqInc();

	while (length > len4*3) {
		x = len4;
		if (wav_scale == 0)
			x *= 2;

		last_wcmdq = wcmdq_tail;
		q = wcmdq[wcmdq_tail];
		q[0] = WCMD_WAVE;
		q[1] = len4*2; // length in samples
		q[2] = (intptr_t)(&wavefile_data[index+x]);
		q[3] = wav_scale + (amp << 8);
		WcmdqInc();

		length -= len4*2;
	}

	if (length > 0) {
		x = wav_length - length;
		if (wav_scale == 0)
			x *= 2;
		last_wcmdq = wcmdq_tail;
		q = wcmdq[wcmdq_tail];
		q[0] = WCMD_WAVE;
		q[1] = length; // length in samples
		q[2] = (intptr_t)(&wavefile_data[index+x]);
		q[3] = wav_scale + (amp << 8);
		WcmdqInc();
	}

	return length;
}

int DoSample3(PHONEME_DATA *phdata, int length_mod, int amp)
{
	int amp2;
	int len;
	EndPitch(1);

	if (amp == -1) {
		// just get the length, don't produce sound
		amp2 = amp;
	} else {
		amp2 = phdata->sound_param[pd_WAV];
		if (amp2 == 0)
			amp2 = 100;
		amp2 = (amp2 * 32)/100;
	}

	seq_len_adjust = 0;

	if (phdata->sound_addr[pd_WAV] == 0)
		len = 0;
	else
		len = DoSample2(phdata->sound_addr[pd_WAV], 2, phdata->pd_param[pd_LENGTHMOD]*2, phdata->pd_control, length_mod, amp2);
	last_frame = NULL;
	return len;
}
/* End legacy command writers. */
#else
static frame_t *CopyFrame(frame_t *frame, int force);
static frame_t *RustFrameStorage(void *opaque, uint32_t kind, frame_t *frame);
// Pauses requested by formant transitions during a spectrum lookup.
static RustCommandLookup *command_lookup;

_Static_assert(sizeof(RustCommandState) == 48 && sizeof(RustCommandSettings) == 80 && sizeof(RustCommandEffect) == 104 &&
               sizeof(RustSpectPhoneme) == 20 && sizeof(frameref_t) == 16, "command layer layout");

typedef struct {
	PHONEME_TAB *ph;
	PHONEME_LIST *plist;
} CommandSpect;

static RustCommandSettings CommandSettings(void)
{
	RustCommandSettings s;
	memset(&s, 0, sizeof(s));
	s.settings.samplerate = samplerate;
	s.settings.pause_factor = speed.pause_factor;
	s.settings.clause_pause_factor = speed.clause_pause_factor;
	s.settings.min_pause = speed.min_pause;
	s.settings.wav_factor = speed.wav_factor;
	s.settings.lenmod_factor = speed.lenmod_factor;
	s.settings.lenmod2_factor = speed.lenmod2_factor;
	s.settings.min_sample_len = speed.min_sample_len;
	s.settings.klatt = voice != NULL && voice->klattv[0] != 0;
	if (translator != NULL) {
		s.settings.long_vowel_threshold = translator->langopts.param[LOPT_LONG_VOWEL_THRESHOLD];
		s.settings.sonorant_min = translator->langopts.param[LOPT_SONORANT_MIN];
	}
	s.settings.fall_envelope = (uintptr_t)envelope_data[PITCHfall];
	s.wave = wavefile_data;
	s.wave_length = wavefile_data != NULL ? RustPhonemeDataLength() : 0;
	s.queue = &espeak_rs_wave_memory;
	return s;
}

static int CommandEffect(void *context, RustCommandEffect *e)
{
	const CommandSpect *spect = context;
	switch (e->op)
	{
	case 4: {
		int start = e->a;
		espeak_rs_smooth_spectrum(wcmdq, N_WCMDQ, &start, e->b, e->c, formant_rate, NULL, RustFrameStorage);
		return start;
	}
	case 5: {
		int n_frames = 0;
		frameref_t *frames;
		memset(e->lookup, 0, sizeof(*e->lookup));
		command_lookup = e->lookup;
		modn_flags = 0;
		frames = LookupSpect(spect->ph, e->a, e->fmt, &n_frames, spect->plist);
		command_lookup = NULL;
		e->lookup->modulation = modn_flags;
		if (frames != NULL && n_frames >= 0 && n_frames <= N_SEQ_FRAMES) {
			e->lookup->found = 1;
			e->lookup->count = n_frames;
			memcpy(e->frames, frames, (size_t)n_frames * sizeof(*frames));
		} else if (frames != NULL) {
			e->lookup->found = 1;
			e->lookup->count = N_SEQ_FRAMES + 1;
		}
		break;
	}
	case 6: {
		const frame_t *frame = (const frame_t *)e->value;
		e->a = frame != NULL ? frame->length : 0;
		e->b = frame != NULL ? frame->frflags : 0;
		break;
	}
	case 7: {
		const frame_t *high = (const frame_t *)e->words[0];
		frame_t *fr = CopyFrame((frame_t *)e->value, 1);
		if (fr != NULL && high != NULL) {
			for (int ix = 3; ix < 8; ix++) {
				if (ix < 7)
					fr->ffreq[ix] = high->ffreq[ix];
				fr->fheight[ix] = high->fheight[ix];
			}
		}
		e->value = (intptr_t)fr;
		break;
	}
	case 8:
		seq_len_adjust = 0;
		break;
	case 9:
		SetEmbedded(e->a, (int)e->value); // adjusts embedded_value[EMBED_S2]
		SetSpeed(2);
		*e->settings = CommandSettings().settings; // for the pauses after it
		return 1;
	case 10: {
		unsigned int value = (unsigned int)e->value;
		if ((int)value >= n_soundicon_tab)
			return 0;
		e->a = soundicon_tab[value].length;
		if (e->a != 0)
			e->words[0] = (intptr_t)soundicon_tab[value].data + 44; // skip WAV header
		return 1;
	}
	}
	return 0;
}

static void EndAmplitude(void)
{
	RustCommandSettings s = CommandSettings();
	espeak_rs_command_end_amplitude(&command_state, &s, NULL, CommandEffect);
}

static void EndPitch(int voice_break)
{
	RustCommandSettings s = CommandSettings();
	espeak_rs_command_end_pitch(&command_state, &s, NULL, CommandEffect, voice_break);
}

static void DoAmplitude(int amp, const unsigned char *amp_env)
{
	RustCommandSettings s = CommandSettings();
	espeak_rs_command_amplitude(&command_state, &s, NULL, CommandEffect, amp, amp_env);
}

static void DoPitch(const unsigned char *env, int pitch1, int pitch2)
{
	RustCommandSettings s = CommandSettings();
	espeak_rs_command_pitch(&command_state, &s, NULL, CommandEffect, env, pitch1, pitch2);
}

int PauseLength(int pause, int control)
{
	RustCommandSettings s = CommandSettings();
	return espeak_rs_pause_length(&s, pause, control);
}

static void DoPause(int length, int control)
{
	RustCommandSettings s = CommandSettings();
	espeak_rs_command_pause(&command_state, &s, NULL, CommandEffect, length, control);
}

int DoSample3(PHONEME_DATA *phdata, int length_mod, int amp)
{
	RustCommandSettings s = CommandSettings();
	int len = espeak_rs_command_sample(&command_state, &s, NULL, CommandEffect, phdata, length_mod, amp);
	return len < 0 ? 0 : len;
}
#endif

#ifndef USE_RUST_CORE
static frame_t *AllocFrame(void)
{
	// Allocate a temporary spectrum frame for the wavegen queue. Use a pool which is big
	// enough to use a round-robin without checks.
	// Only needed for modifying spectra for blending to consonants

	#define N_FRAME_POOL N_WCMDQ
	static int ix = 0;
	static frame_t frame_pool[N_FRAME_POOL];

	ix++;
	if (ix >= N_FRAME_POOL)
		ix = 0;

	MAKE_MEM_UNDEFINED(&frame_pool[ix], sizeof(frame_pool[ix]));
	return &frame_pool[ix];
}

static void set_frame_rms(frame_t *fr, int new_rms)
{
	// Each frame includes its RMS amplitude value, so to set a new
	// RMS just adjust the formant amplitudes by the appropriate ratio

	int x;
	int ix;

	static const short sqrt_tab[200] = {
		  0,  64,  90, 110, 128, 143, 156, 169, 181, 192, 202, 212, 221, 230, 239, 247,
		256, 263, 271, 278, 286, 293, 300, 306, 313, 320, 326, 332, 338, 344, 350, 356,
		362, 367, 373, 378, 384, 389, 394, 399, 404, 409, 414, 419, 424, 429, 434, 438,
		443, 448, 452, 457, 461, 465, 470, 474, 478, 483, 487, 491, 495, 499, 503, 507,
		512, 515, 519, 523, 527, 531, 535, 539, 543, 546, 550, 554, 557, 561, 565, 568,
		572, 576, 579, 583, 586, 590, 593, 596, 600, 603, 607, 610, 613, 617, 620, 623,
		627, 630, 633, 636, 640, 643, 646, 649, 652, 655, 658, 662, 665, 668, 671, 674,
		677, 680, 683, 686, 689, 692, 695, 698, 701, 704, 706, 709, 712, 715, 718, 721,
		724, 726, 729, 732, 735, 738, 740, 743, 746, 749, 751, 754, 757, 759, 762, 765,
		768, 770, 773, 775, 778, 781, 783, 786, 789, 791, 794, 796, 799, 801, 804, 807,
		809, 812, 814, 817, 819, 822, 824, 827, 829, 832, 834, 836, 839, 841, 844, 846,
		849, 851, 853, 856, 858, 861, 863, 865, 868, 870, 872, 875, 877, 879, 882, 884,
		886, 889, 891, 893, 896, 898, 900, 902
	};

	if (voice->klattv[0]) {
		if (new_rms == -1)
			fr->klattp[KLATT_AV] = 50;
		return;
	}

	if (fr->rms == 0) return; // check for divide by zero
	x = (new_rms * 64)/fr->rms;
	if (x >= 200) x = 199;

	x = sqrt_tab[x]; // sqrt(new_rms/fr->rms)*0x200;

	for (ix = 0; ix < 8; ix++) {
		int h;
		h = fr->fheight[ix] * x;
		fr->fheight[ix] = h/0x200;
	}
}

static void formants_reduce_hf(frame_t *fr, int level)
{
	// change height of peaks 2 to 8, percentage
	if (voice->klattv[0])
		return;

	for (int ix = 2; ix < 8; ix++) {
		int x;
		x = fr->fheight[ix] * level;
		fr->fheight[ix] = x/100;
	}
}

static frame_t *CopyFrame(frame_t *frame1, int copy)
{
	// create a copy of the specified frame in temporary buffer

	frame_t *frame2;

	if ((copy == 0) && (frame1->frflags & FRFLAG_COPIED)) {
		// this frame has already been copied in temporary rw memory
		return frame1;
	}

	frame2 = AllocFrame();
	if (frame2 != NULL) {
#ifdef USE_RUST_CORE
		// Ordinary resident frames contain 44 bytes, not the 64-byte Klatt
		// extension. Keep pooled padding initialized without overreading them.
		memset(frame2, 0, sizeof(*frame2));
		memcpy(frame2, frame1, frame1->frflags & FRFLAG_KLATT ? sizeof(frame_t) : sizeof(frame_t2));
#else
		memcpy(frame2, frame1, sizeof(frame_t));
#endif
		frame2->length = 0;
		frame2->frflags |= FRFLAG_COPIED;
	}
	return frame2;
}

static frame_t *DuplicateLastFrame(frameref_t *seq, int n_frames, int length)
{
	frame_t *fr;

	seq[n_frames-1].length = length;
	fr = CopyFrame(seq[n_frames-1].frame, 1);
	seq[n_frames].frame = fr;
	seq[n_frames].length = 0;
	return fr;
}

static void AdjustFormants(frame_t *fr, int target, int min, int max, int f1_adj, int f3_adj, int hf_reduce, int flags)
{
	int x;

	target = (target * voice->formant_factor)/256;

	x = (target - fr->ffreq[2]) / 2;
	if (x > max) x = max;
	if (x < min) x = min;
	fr->ffreq[2] += x;
	fr->ffreq[3] += f3_adj;

	if (flags & 0x20)
		f3_adj = -f3_adj; // reverse direction for f4,f5 change
	fr->ffreq[4] += f3_adj;
	fr->ffreq[5] += f3_adj;

	if (f1_adj == 1) {
		x = (235 - fr->ffreq[1]);
		if (x < -100) x = -100;
		if (x > -60) x = -60;
		fr->ffreq[1] += x;
	}
	if (f1_adj == 2) {
		x = (235 - fr->ffreq[1]);
		if (x < -300) x = -300;
		if (x > -150) x = -150;
		fr->ffreq[1] += x;
		fr->ffreq[0] += x;
	}
	if (f1_adj == 3) {
		x = (100 - fr->ffreq[1]);
		if (x < -400) x = -400;
		if (x > -300) x = -400;
		fr->ffreq[1] += x;
		fr->ffreq[0] += x;
	}
	formants_reduce_hf(fr, hf_reduce);
}

static int VowelCloseness(frame_t *fr)
{
	// return a value 0-3 depending on the vowel's f1
	int f1;

	if ((f1 = fr->ffreq[1]) < 300)
		return 3;
	if (f1 < 400)
		return 2;
	if (f1 < 500)
		return 1;
	return 0;
}

int FormantTransition2(frameref_t *seq, int *n_frames, unsigned int data1, unsigned int data2, PHONEME_TAB *other_ph, int which)
{
	int len;
	int rms;
	int f1;
	int f2;
	int f2_min;
	int f2_max;
	int f3_adj;
	int f3_amp;
	int flags;
	int vcolour;

	#define N_VCOLOUR 2
	// percentage change for each formant in 256ths
	static const short vcolouring[N_VCOLOUR][5] = {
		{ 243, 272, 256, 256, 256 }, // palatal consonant follows
		{ 256, 256, 240, 240, 240 }, // retroflex
	};

	frame_t *fr = NULL;

	if (*n_frames < 2)
		return 0;

	len = (data1 & 0x3f) * 2;
	rms = (data1 >> 6) & 0x3f;
	flags = (data1 >> 12);

	f2 = (data2 & 0x3f) * 50;
	f2_min = (((data2 >> 6) & 0x1f) - 15) * 50;
	f2_max = (((data2 >> 11) & 0x1f) - 15) * 50;
	f3_adj = (((data2 >> 16) & 0x1f) - 15) * 50;
	f3_amp = ((data2 >> 21) & 0x1f) * 8;
	f1 = ((data2 >> 26) & 0x7);
	vcolour = (data2 >> 29);

	if ((other_ph != NULL) && (other_ph->mnemonic == '?'))
		flags |= 8;

	if (which == 1) {
		// entry to vowel
		fr = CopyFrame(seq[0].frame, 0);
		seq[0].frame = fr;
		seq[0].length = VOWEL_FRONT_LENGTH;
		if (len > 0)
			seq[0].length = len;
		seq[0].frflags |= FRFLAG_LEN_MOD2; // reduce length modification
		fr->frflags |= FRFLAG_LEN_MOD2;

		int next_rms = seq[1].frame->rms;

		if (voice->klattv[0])
			fr->klattp[KLATT_AV] = seq[1].frame->klattp[KLATT_AV] - 4;
		if (f2 != 0) {
			if (rms & 0x20)
				set_frame_rms(fr, (next_rms * (rms & 0x1f))/30);
			AdjustFormants(fr, f2, f2_min, f2_max, f1, f3_adj, f3_amp, flags);

			if ((rms & 0x20) == 0)
				set_frame_rms(fr, rms*2);
		} else {
			if (flags & 8)
				set_frame_rms(fr, (next_rms*24)/32);
			else
				set_frame_rms(fr, RMS_START);
		}

		if (flags & 8)
			modn_flags = 0x800 + (VowelCloseness(fr) << 8);
	} else {
		// exit from vowel
		rms = rms*2;
		if ((f2 != 0) || (flags != 0)) {

			if (flags & 8) {
				fr = CopyFrame(seq[*n_frames-1].frame, 0);
				seq[*n_frames-1].frame = fr;
				rms = RMS_GLOTTAL1;

				// degree of glottal-stop effect depends on closeness of vowel (indicated by f1 freq)
				modn_flags = 0x400 + (VowelCloseness(fr) << 8);
			} else {
				fr = DuplicateLastFrame(seq, (*n_frames)++, len);
				if (len > 36)
					seq_len_adjust += (len - 36);

				if (f2 != 0)
					AdjustFormants(fr, f2, f2_min, f2_max, f1, f3_adj, f3_amp, flags);
			}

			set_frame_rms(fr, rms);

			if ((vcolour > 0) && (vcolour <= N_VCOLOUR)) {
				for (int ix = 0; ix < *n_frames; ix++) {
					fr = CopyFrame(seq[ix].frame, 0);
					seq[ix].frame = fr;

					for (int formant = 1; formant <= 5; formant++) {
						int x;
						x = fr->ffreq[formant] * vcolouring[vcolour-1][formant-1];
						fr->ffreq[formant] = x / 256;
					}
				}
			}
		}
	}

	if (fr != NULL) {
		if (flags & 4)
			fr->frflags |= FRFLAG_FORMANT_RATE;
		if (flags & 2)
			fr->frflags |= FRFLAG_BREAK; // don't merge with next frame
	}

	if (flags & 0x40)
		DoPause(20, 0); // add a short pause after the consonant

	if (flags & 16)
		return len;
	return 0;
}
/* End legacy formant blending. Kept as a differential oracle. */
#else
static frame_t rust_frame_pool[N_WCMDQ];
static int rust_frame_cursor;
static frame_t *RustFrameStorage(void *opaque, uint32_t kind, frame_t *frame)
{
	(void)opaque;
	if (kind == 0) {
		if (++rust_frame_cursor >= N_WCMDQ) rust_frame_cursor = 0;
		return &rust_frame_pool[rust_frame_cursor];
	}
	if (kind == 1) {
		uintptr_t pointer = (uintptr_t)frame, base = (uintptr_t)rust_frame_pool;
		if (pointer >= base && pointer - base < sizeof(rust_frame_pool) && (pointer-base) % sizeof(frame_t) == 0) return frame;
	}
	return NULL;
}
static frame_t *CopyFrame(frame_t *frame, int force)
{
	return espeak_rs_frame_copy(frame, force != 0, NULL, RustFrameStorage);
}
int FormantTransitionWithCapacity(frameref_t *seq, int *count, unsigned int data1, unsigned int data2, PHONEME_TAB *other, int which, size_t capacity)
{
	RustFormantSettings settings = { .which = which, .klatt = voice != NULL && voice->klattv[0] != 0,
	    .formant_factor = voice == NULL ? 256 : voice->formant_factor,
	    .other_glottal = other != NULL && other->mnemonic == '?', .length_adjust = seq_len_adjust };
	RustFormantEffects effects;
	if (espeak_rs_formant_transition(seq,capacity,count,data1,data2,&settings,NULL,RustFrameStorage,&effects) != 0) return 0;
	seq_len_adjust = effects.length_adjust;
	if (effects.has_modulation) modn_flags = effects.modulation;
	if (effects.pause) {
		if (command_lookup == NULL)
			DoPause(effects.pause,0);
		else if (command_lookup->n_pauses < 4)
			command_lookup->pauses[command_lookup->n_pauses++] = effects.pause;
		else
			command_lookup->n_pauses = 5; // more than the writers accept
	}
	return effects.return_length;
}
int FormantTransition2(frameref_t *seq, int *count, unsigned int data1, unsigned int data2, PHONEME_TAB *other, int which)
{
	return FormantTransitionWithCapacity(seq,count,data1,data2,other,which,N_SEQ_FRAMES);
}
#endif
#ifndef USE_RUST_CORE
int FormantTransitionWithCapacity(frameref_t *seq, int *count, unsigned int data1, unsigned int data2, PHONEME_TAB *other, int which, size_t capacity)
{
	(void)capacity;
	return FormantTransition2(seq,count,data1,data2,other,which);
}
#endif

#ifndef USE_RUST_CORE
static void SmoothSpect(void)
{
	// Limit the rate of frequence change of formants, to reduce chirping

	intptr_t *q;
	frame_t *frame;
	frame_t *frame2;
	frame_t *frame1;
	frame_t *frame_centre;
	int ix;
	int len;
	int pk;
	bool modified;
	int allowed;
	int diff;

	if (syllable_start == syllable_end)
		return;

	if ((syllable_centre < 0) || (syllable_centre == syllable_start)) {
		syllable_start = syllable_end;
		return;
	}

	q = wcmdq[syllable_centre];
	frame_centre = (frame_t *)q[2];

	// backwards
	ix = syllable_centre -1;
	frame = frame2 = frame_centre;
	for (;;) {
		if (ix < 0) ix = N_WCMDQ-1;
		q = wcmdq[ix];

		if (q[0] == WCMD_PAUSE || q[0] == WCMD_WAVE)
			break;

		if (q[0] <= WCMD_SPECT2) {
			len = q[1] & 0xffff;

			frame1 = (frame_t *)q[3];
			if (frame1 == frame) {
				q[3] = (intptr_t)frame2;
				frame1 = frame2;
			} else
				break; // doesn't follow on from previous frame

			frame = frame2 = (frame_t *)q[2];
			modified = false;

			if (frame->frflags & FRFLAG_BREAK)
				break;

			if (frame->frflags & FRFLAG_FORMANT_RATE)
				len = (len * 12)/10; // allow slightly greater rate of change for this frame (was 12/10)

			for (pk = 0; pk < 6; pk++) {
				int f1, f2;

				if ((frame->frflags & FRFLAG_BREAK_LF) && (pk < 3))
					continue;

				f1 = frame1->ffreq[pk];
				f2 = frame->ffreq[pk];

				// backwards
				if ((diff = f2 - f1) > 0)
					allowed = f1*2 + f2;
				else
					allowed = f1 + f2*2;

				// the allowed change is specified as percentage (%*10) of the frequency
				// take "frequency" as 1/3 from the lower freq
				allowed = (allowed * formant_rate[pk])/3000;
				allowed = (allowed * len)/256;

				if (diff > allowed) {
					if (modified == false) {
						frame2 = CopyFrame(frame, 0);
						modified = true;
					}
					frame2->ffreq[pk] = frame1->ffreq[pk] + allowed;
					q[2] = (intptr_t)frame2;
				} else if (diff < -allowed) {
					if (modified == false) {
						frame2 = CopyFrame(frame, 0);
						modified = true;
					}
					frame2->ffreq[pk] = frame1->ffreq[pk] - allowed;
					q[2] = (intptr_t)frame2;
				}
			}
		}

		if (ix == syllable_start)
			break;
		ix--;
	}

	// forwards
	ix = syllable_centre;

	frame = NULL;
	for (;;) {
		q = wcmdq[ix];

		if (q[0] == WCMD_PAUSE || q[0] == WCMD_WAVE)
			break;

		if (q[0] <= WCMD_SPECT2) {
			len = q[1] & 0xffff;

			frame1 = (frame_t *)q[2];
			if (frame != NULL) {
				if (frame1 == frame) {
					q[2] = (intptr_t)frame2;
					frame1 = frame2;
				} else
					break; // doesn't follow on from previous frame
			}

			frame = frame2 = (frame_t *)q[3];
			modified = false;

			if (frame1->frflags & FRFLAG_BREAK)
				break;

			if (frame1->frflags & FRFLAG_FORMANT_RATE)
				len = (len *6)/5; // allow slightly greater rate of change for this frame

			for (pk = 0; pk < 6; pk++) {
				int f1, f2;
				f1 = frame1->ffreq[pk];
				f2 = frame->ffreq[pk];

				// forwards
				if ((diff = f2 - f1) > 0)
					allowed = f1*2 + f2;
				else
					allowed = f1 + f2*2;
				allowed = (allowed * formant_rate[pk])/3000;
				allowed = (allowed * len)/256;

				if (diff > allowed) {
					if (modified == false) {
						frame2 = CopyFrame(frame, 0);
						modified = true;
					}
					frame2->ffreq[pk] = frame1->ffreq[pk] + allowed;
					q[3] = (intptr_t)frame2;
				} else if (diff < -allowed) {
					if (modified == false) {
						frame2 = CopyFrame(frame, 0);
						modified = true;
					}
					frame2->ffreq[pk] = frame1->ffreq[pk] - allowed;
					q[3] = (intptr_t)frame2;
				}
			}
		}

		ix++;
		if (ix >= N_WCMDQ) ix = 0;
		if (ix == syllable_end)
			break;
	}

	syllable_start = syllable_end;
}
/* End legacy spectrum smoothing. Kept as a differential oracle. */
#endif

#ifndef USE_RUST_CORE
static void StartSyllable(void)
{
	// start of syllable, if not already started
	if (syllable_end == syllable_start)
		syllable_end = wcmdq_tail;
}

int DoSpect2(PHONEME_TAB *this_ph, int which, FMT_PARAMS *fmt_params,  PHONEME_LIST *plist, int modulation)
{
	// which:  0 not a vowel, 1  start of vowel,   2 body and end of vowel
	// length_mod: 256 = 100%
	// modulation: -1 = don't write to wcmdq

	int n_frames;
	frameref_t *frames;
	int frameix;
	frame_t *frame1;
	frame_t *frame2;
	intptr_t *q;
	int len;
	int length_mod;
	int length_sum;
	int length_min;
	int total_len = 0;
	static int wave_flag = 0;
	int wcmd_spect = WCMD_SPECT;
	int frame_lengths[N_SEQ_FRAMES];

	if (fmt_params->fmt_addr == 0)
		return 0;

	length_mod = plist->length;
	if (length_mod == 0) length_mod = 256;

	length_min = (samplerate/70); // greater than one cycle at low pitch (Hz)
	if (which == 2) {
		if ((translator->langopts.param[LOPT_LONG_VOWEL_THRESHOLD] > 0) && ((this_ph->std_length >= translator->langopts.param[LOPT_LONG_VOWEL_THRESHOLD]) || (plist->synthflags & SFLAG_LENGTHEN) || (this_ph->phflags & phLONG)))
			length_min *= 2; // ensure long vowels are longer
	}

	if (which == 1) {
		// limit the shortening of sonorants before shortened (eg. unstressed vowels)
		if ((this_ph->type == phLIQUID) || (plist[-1].type == phLIQUID) || (plist[-1].type == phNASAL)) {
			if (length_mod < (len = translator->langopts.param[LOPT_SONORANT_MIN]))
				length_mod = len;
		}
	}

	modn_flags = 0;
	frames = LookupSpect(this_ph, which, fmt_params, &n_frames, plist);
	if (frames == NULL)
		return 0; // not found

	if (fmt_params->fmt_amp != fmt_amplitude) {
		// an amplitude adjustment is specified for this sequence
		q = wcmdq[wcmdq_tail];
		q[0] = WCMD_FMT_AMPLITUDE;
		q[1] = fmt_amplitude = fmt_params->fmt_amp;
		WcmdqInc();
	}

	frame1 = frames[0].frame;
	if (voice->klattv[0])
		wcmd_spect = WCMD_KLATT;

	if (fmt_params->wav_addr == 0) {
		if (wave_flag) {
			// cancel any wavefile that was playing previously
			wcmd_spect = WCMD_SPECT2;
			if (voice->klattv[0])
				wcmd_spect = WCMD_KLATT2;
			wave_flag = 0;
		} else {
			wcmd_spect = WCMD_SPECT;
			if (voice->klattv[0])
				wcmd_spect = WCMD_KLATT;
		}
	}

	if (last_frame != NULL) {
		if (((last_frame->length < 2) || (last_frame->frflags & FRFLAG_VOWEL_CENTRE))
		    && !(last_frame->frflags & FRFLAG_BREAK)) {
			// last frame of previous sequence was zero-length, replace with first of this sequence
			wcmdq[last_wcmdq][3] = (intptr_t)frame1;

			if (last_frame->frflags & FRFLAG_BREAK_LF) {
				// but flag indicates keep HF peaks in last segment
				frame_t *fr;
				fr = CopyFrame(frame1, 1);
				for (int ix = 3; ix < 8; ix++) {
					if (ix < 7)
						fr->ffreq[ix] = last_frame->ffreq[ix];
					fr->fheight[ix] = last_frame->fheight[ix];
				}
				wcmdq[last_wcmdq][3] = (intptr_t)fr;
			}
		}
	}

	if ((this_ph->type == phVOWEL) && (which == 2)) {
		SmoothSpect(); // process previous syllable

		// remember the point in the output queue of the centre of the vowel
		syllable_centre = wcmdq_tail;
	}

	length_sum = 0;
	for (frameix = 1; frameix < n_frames; frameix++) {
		int length_factor = length_mod;
		if (frames[frameix-1].frflags & FRFLAG_LEN_MOD) // reduce effect of length mod
			length_factor = (length_mod*(256-speed.lenmod_factor) + 256*speed.lenmod_factor)/256;
		else if (frames[frameix-1].frflags & FRFLAG_LEN_MOD2) // reduce effect of length mod, used for the start of a vowel
			length_factor = (length_mod*(256-speed.lenmod2_factor) + 256*speed.lenmod2_factor)/256;

		int frame_length = frames[frameix-1].length;
		len = (frame_length * samplerate)/1000;
		len = (len * length_factor)/256;
		length_sum += len;
		frame_lengths[frameix] = len;
	}

	if ((length_sum > 0) && (length_sum < length_min)) {
		// lengthen, so that the sequence is greater than one cycle at low pitch
		for (frameix = 1; frameix < n_frames; frameix++)
			frame_lengths[frameix] = (frame_lengths[frameix] * length_min) / length_sum;
	}

	for (frameix = 1; frameix < n_frames; frameix++) {
		frame2 = frames[frameix].frame;

		if ((fmt_params->wav_addr != 0) && ((frame1->frflags & FRFLAG_DEFER_WAV) == 0)) {
			// there is a wave file to play along with this synthesis
			seq_len_adjust = 0;

			int wavefile_amp;
			if (fmt_params->wav_amp == 0)
				wavefile_amp = 32;
			else
				wavefile_amp = (fmt_params->wav_amp * 32)/100;

			DoSample2(fmt_params->wav_addr, which+0x100, 0, fmt_params->fmt_control, 0, wavefile_amp);
			wave_flag = 1;
			fmt_params->wav_addr = 0;
		}

		if (modulation >= 0) {
			if (frame1->frflags & FRFLAG_MODULATE)
				modulation = 6;
			if ((frameix == n_frames-1) && (modn_flags & 0xf00))
				modulation |= modn_flags; // before or after a glottal stop
		}

		len = frame_lengths[frameix];
		pitch_length += len;
		amp_length += len;

		if (len == 0) {
			last_frame = NULL;
			frame1 = frame2;
		} else {
			last_wcmdq = wcmdq_tail;

			if (modulation >= 0) {
				q = wcmdq[wcmdq_tail];
				q[0] = wcmd_spect;
				q[1] = len + (modulation << 16);
				q[2] = (intptr_t)frame1;
				q[3] = (intptr_t)frame2;

				WcmdqInc();
			}
			last_frame = frame1 = frame2;
			total_len += len;
		}
	}

	if ((which != 1) && (fmt_amplitude != 0)) {
		q = wcmdq[wcmdq_tail];
		q[0] = WCMD_FMT_AMPLITUDE;
		q[1] = fmt_amplitude = 0;
		WcmdqInc();
	}

	return total_len;
}
/* End legacy spectrum command. */
#else
static void StartSyllable(void)
{
	RustCommandSettings s = CommandSettings();
	espeak_rs_command_start_syllable(&command_state, &s, NULL, CommandEffect);
}

int DoSpect2(PHONEME_TAB *this_ph, int which, FMT_PARAMS *fmt_params,  PHONEME_LIST *plist, int modulation)
{
	// which:  0 not a vowel, 1  start of vowel,   2 body and end of vowel
	// modulation: -1 = don't write to wcmdq
	RustCommandSettings s = CommandSettings();
	CommandSpect spect = { this_ph, plist };
	RustSpectPhoneme ph;
	int len;
	memset(&ph, 0, sizeof(ph));
	ph.type = this_ph->type;
	ph.std_length = this_ph->std_length;
	ph.phflags = this_ph->phflags;
	ph.synthflags = plist->synthflags;
	ph.length = plist->length;
	ph.prev_type = which == 1 ? plist[-1].type : 0;
	len = espeak_rs_command_spect(&command_state, &s, &spect, CommandEffect, &ph, which, fmt_params, modulation);
	return len < 0 ? 0 : len;
}
#endif

#ifndef USE_RUST_CORE
void DoMarker(int type, int char_posn, int length, int value)
{
	// This could be used to return an index to the word currently being spoken
	// Type 1=word, 2=sentence, 3=named marker, 4=play audio, 5=end

	if (WcmdqFree() > 5) {
		wcmdq[wcmdq_tail][0] = WCMD_MARKER + (type << 8);
		wcmdq[wcmdq_tail][1] = (char_posn & 0xffffff) | (length << 24);
		wcmdq[wcmdq_tail][2] = value;
		WcmdqInc();
	}
}

void DoPhonemeMarker(int type, int char_posn, int length, char *name)
{
	// This could be used to return an index to the word currently being spoken
	// Type 7=phoneme

	if (WcmdqFree() > 5) {
		wcmdq[wcmdq_tail][0] = WCMD_MARKER + (type << 8);
		wcmdq[wcmdq_tail][1] = (char_posn & 0xffffff) | (length << 24);
		memcpy(&wcmdq[wcmdq_tail][2], name, 8); // up to 8 bytes of UTF8 characters
		WcmdqInc();
	}
}

#if USE_LIBSONIC
void DoSonicSpeed(int value)
{
	// value, multiplier * 1024
	wcmdq[wcmdq_tail][0] = WCMD_SONIC_SPEED;
	wcmdq[wcmdq_tail][1] = value;
	WcmdqInc();
}
#endif

espeak_ng_STATUS DoVoiceChange(voice_t *v)
{
	// allocate memory for a copy of the voice data, and free it in wavegenfill()
	voice_t *v2;
	if ((v2 = (voice_t *)malloc(sizeof(voice_t))) == NULL)
		return ENOMEM;
	memcpy(v2, v, sizeof(voice_t));
	wcmdq[wcmdq_tail][0] = WCMD_VOICE;
	wcmdq[wcmdq_tail][2] = (intptr_t)v2;
	WcmdqInc();
	return ENS_OK;
}

void DoEmbedded(int *embix, int sourceix)
{
	// There were embedded commands in the text at this point
	unsigned int word; // bit 7=last command for this word, bits 5,6 sign, bits 0-4 command

	do {
		unsigned int value;
		int command;

		word = embedded_list[*embix];
		value = word >> 8;
		command = word & 0x7f;

		if (command == 0)
			return; // error

		(*embix)++;

		switch (command & 0x1f)
		{
		case EMBED_S: // speed
			SetEmbedded((command & 0x60) + EMBED_S2, value); // adjusts embedded_value[EMBED_S2]
			SetSpeed(2);
			break;
		case EMBED_I: // play dynamically loaded wav data (sound icon)
			if ((int)value < n_soundicon_tab) {
				if (soundicon_tab[value].length != 0) {
					DoPause(10, 0); // ensure a break in the speech
					wcmdq[wcmdq_tail][0] = WCMD_WAVE;
					wcmdq[wcmdq_tail][1] = soundicon_tab[value].length;
					wcmdq[wcmdq_tail][2] = (intptr_t)soundicon_tab[value].data + 44; // skip WAV header
					wcmdq[wcmdq_tail][3] = 0x1500; // 16 bit data, amp=21
					WcmdqInc();
				}
			}
			break;
		case EMBED_M: // named marker
			DoMarker(espeakEVENT_MARK, (sourceix & 0x7ff) + clause_start_char, 0, value);
			break;
		case EMBED_U: // play sound
			DoMarker(espeakEVENT_PLAY, count_characters+1, 0, value); // always occurs at end of clause
			break;
		default:
			DoPause(10, 0); // ensure a break in the speech
			wcmdq[wcmdq_tail][0] = WCMD_EMBEDDED;
			wcmdq[wcmdq_tail][1] = command;
			wcmdq[wcmdq_tail][2] = value;
			WcmdqInc();
			break;
		}
	} while ((word & 0x80) == 0);
}

/* End legacy queue writers. */
#else
void DoMarker(int type, int char_posn, int length, int value)
{
	espeak_rs_queue_marker(&espeak_rs_wave_memory, type, char_posn, length, value);
}

void DoPhonemeMarker(int type, int char_posn, int length, char *name)
{
	espeak_rs_queue_phoneme_marker(&espeak_rs_wave_memory, type, char_posn, length, name);
}

#if USE_LIBSONIC
void DoSonicSpeed(int value)
{
	espeak_rs_queue_sonic_speed(&espeak_rs_wave_memory, value);
}
#endif

espeak_ng_STATUS DoVoiceChange(voice_t *v)
{
	// allocate memory for a copy of the voice data, and free it in wavegenfill()
	voice_t *v2;
	if ((v2 = (voice_t *)malloc(sizeof(voice_t))) == NULL)
		return ENOMEM;
	memcpy(v2, v, sizeof(voice_t));
	espeak_rs_queue_voice(&espeak_rs_wave_memory, (unsigned char *)v2);
	return ENS_OK;
}

void DoEmbedded(int *embix, int sourceix)
{
	RustCommandSettings s = CommandSettings();
	espeak_rs_command_embedded(&command_state, &s, NULL, CommandEffect, embedded_list, N_EMBEDDED_LIST, embix, sourceix,
	                           clause_start_char, count_characters);
}
#endif

extern espeak_ng_OUTPUT_HOOKS* output_hooks;

#ifndef USE_RUST_CORE
int Generate(PHONEME_LIST *phoneme_list, int *n_ph, bool resume)
{
	static int ix;
	static int embedded_ix;
	static int word_count;
	PHONEME_LIST *p;
	bool released;
	int stress;
	int modulation;
	bool  pre_voiced;
	int free_min;
	int value;
	const unsigned char *pitch_env = NULL;
	const unsigned char *amp_env;
	PHONEME_TAB *ph;
	int use_ipa = 0;
	int vowelstart_prev;
	char phoneme_name[16];
	static int sourceix = 0;

	PHONEME_DATA phdata;
	PHONEME_DATA phdata_prev;
	PHONEME_DATA phdata_next;
	PHONEME_DATA phdata_tone;
	FMT_PARAMS fmtp;
	static WORD_PH_DATA worddata;

	if (option_phoneme_events & espeakINITIALIZE_PHONEME_IPA)
		use_ipa = 1;

#if USE_MBROLA
	if (mbrola_name[0] != 0)
		return MbrolaGenerate(phoneme_list, n_ph, resume);
#endif

	if (resume == false) {
		ix = 1;
		embedded_ix = 0;
		word_count = 0;
		pitch_length = 0;
		amp_length = 0;
		last_frame = NULL;
		last_wcmdq = -1;
		syllable_start = wcmdq_tail;
		syllable_end = wcmdq_tail;
		syllable_centre = -1;
		last_pitch_cmd = -1;
		memset(&worddata, 0, sizeof(worddata));
		DoPause(0, 0); // isolate from the previous clause
	}

	while ((ix < (*n_ph)) && (ix < N_PHONEME_LIST-2)) {
		p = &phoneme_list[ix];

		if(output_hooks && output_hooks->outputPhoSymbol)
		{
			char buf[30];
			int dummy=0;
			//WritePhMnemonic(buf, p->ph, p, 0, &dummy);
			WritePhMnemonicWithStress(buf, p->ph, p, 0, &dummy);

			DoPhonemeAlignment(strdup(buf),p->type);
		}

		if (p->type == phPAUSE)
			free_min = 10;
		else if (p->type != phVOWEL)
			free_min = 15; // we need less Q space for non-vowels, and we need to generate phonemes after a vowel so that the pitch_length is filled in
		else
			free_min = MIN_WCMDQ;

		if (WcmdqFree() <= free_min)
			return 1; // wait

		PHONEME_LIST *prev;
		PHONEME_LIST *next;
		PHONEME_LIST *next2;

		prev = &phoneme_list[ix-1];
		next = &phoneme_list[ix+1];
		next2 = &phoneme_list[ix+2];

		if (p->synthflags & SFLAG_EMBEDDED)
			DoEmbedded(&embedded_ix, p->sourceix);

		if (p->newword) {
			if (((p->type == phVOWEL) && (translator->langopts.param[LOPT_WORD_MERGE] & 1)) ||
			    (p->ph->phflags & phNOPAUSE)) {
			} else
				last_frame = NULL;

			sourceix = (p->sourceix & 0x7ff) + clause_start_char;

			if (p->newword & PHLIST_START_OF_SENTENCE)
				DoMarker(espeakEVENT_SENTENCE, sourceix, 0, count_sentences); // start of sentence

			if (p->newword & PHLIST_START_OF_WORD)
				DoMarker(espeakEVENT_WORD, sourceix, p->sourceix >> 11, clause_start_word + word_count++); // NOTE, this count doesn't include multiple-word pronunciations in *_list. eg (of a)
		}

		EndAmplitude();

		if ((p->prepause > 0) && !(p->ph->phflags & phPREVOICE))
			DoPause(p->prepause, 1);

		bool done_phoneme_marker = false;
		if (option_phoneme_events && (p->ph->code != phonEND_WORD)) {
			if ((p->type == phVOWEL) && (prev->type == phLIQUID || prev->type == phNASAL)) {
				// For vowels following a liquid or nasal, do the phoneme event after the vowel-start
			} else {
				//WritePhMnemonic(phoneme_name, p->ph, p, use_ipa, NULL);
				WritePhMnemonicWithStress(phoneme_name, p->ph, p, use_ipa, NULL);

				DoPhonemeMarker(espeakEVENT_PHONEME, sourceix, 0, phoneme_name);
				done_phoneme_marker = true;
			}
		}

		switch (p->type)
		{
		case phPAUSE:
			DoPause(p->length, 0);
			p->std_length = p->ph->std_length;
			break;
		case phSTOP:
			released = false;
			ph = p->ph;
			if (next->type == phVOWEL)
				released = true;
			else if (!next->newword) {
				if (next->type == phLIQUID) released = true;
			}
			if (released == false)
				p->synthflags |= SFLAG_NEXT_PAUSE;

			if (ph->phflags & phPREVOICE) {
				// a period of voicing before the release
				memset(&fmtp, 0, sizeof(fmtp));
				InterpretPhonemeWithLength(NULL, 0x01, p, phoneme_list, &phdata, &worddata, *n_ph);
				fmtp.fmt_addr = phdata.sound_addr[pd_FMT];
				fmtp.fmt_amp = phdata.sound_param[pd_FMT];

				if (last_pitch_cmd < 0) {
					DoAmplitude(next->amp, NULL);
					DoPitch(envelope_data[p->env], next->pitch1, next->pitch2);
				}

				DoSpect2(ph, 0, &fmtp, p, 0);
			}

			InterpretPhonemeWithLength(NULL, 0, p, phoneme_list, &phdata, &worddata, *n_ph);
			phdata.pd_control |= pd_DONTLENGTHEN;
			DoSample3(&phdata, 0, 0);
			break;
		case phFRICATIVE:
			InterpretPhonemeWithLength(NULL, 0, p, phoneme_list, &phdata, &worddata, *n_ph);

			if (p->synthflags & SFLAG_LENGTHEN)
				DoSample3(&phdata, p->length, 0); // play it twice for [s:] etc.
			DoSample3(&phdata, p->length, 0);
			break;
		case phVSTOP:
			ph = p->ph;
			memset(&fmtp, 0, sizeof(fmtp));
			fmtp.fmt_control = pd_DONTLENGTHEN;

			pre_voiced = false;
			if (next->type == phVOWEL) {
				DoAmplitude(p->amp, NULL);
				DoPitch(envelope_data[p->env], p->pitch1, p->pitch2);
				pre_voiced = true;
			} else if ((next->type == phLIQUID) && !next->newword) {
				DoAmplitude(next->amp, NULL);
				DoPitch(envelope_data[next->env], next->pitch1, next->pitch2);
				pre_voiced = true;
			} else {
				if (last_pitch_cmd < 0) {
					DoAmplitude(next->amp, NULL);
					DoPitch(envelope_data[p->env], p->pitch1, p->pitch2);
				}
			}

			if ((prev->type == phVOWEL) || (ph->phflags & phPREVOICE)) {
				// a period of voicing before the release
				InterpretPhonemeWithLength(NULL, 0x01, p, phoneme_list, &phdata, &worddata, *n_ph);
				fmtp.fmt_addr = phdata.sound_addr[pd_FMT];
				fmtp.fmt_amp = phdata.sound_param[pd_FMT];

				DoSpect2(ph, 0, &fmtp, p, 0);
				if (p->synthflags & SFLAG_LENGTHEN) {
					DoPause(25, 1);
					DoSpect2(ph, 0, &fmtp, p, 0);
				}
			} else {
				if (p->synthflags & SFLAG_LENGTHEN)
					DoPause(50, 0);
			}

			if (pre_voiced) {
				// followed by a vowel, or liquid + vowel
				StartSyllable();
			} else
				p->synthflags |= SFLAG_NEXT_PAUSE;
			InterpretPhonemeWithLength(NULL, 0, p, phoneme_list, &phdata, &worddata, *n_ph);
			fmtp.fmt_addr = phdata.sound_addr[pd_FMT];
			fmtp.fmt_amp = phdata.sound_param[pd_FMT];
			fmtp.wav_addr = phdata.sound_addr[pd_ADDWAV];
			fmtp.wav_amp = phdata.sound_param[pd_ADDWAV];
			DoSpect2(ph, 0, &fmtp, p, 0);

			if ((p->newword == 0) && (next2->newword == 0)) {
				if (next->type == phVFRICATIVE)
					DoPause(20, 0);
				if (next->type == phFRICATIVE)
					DoPause(12, 0);
			}
			break;
		case phVFRICATIVE:
			if (next->type == phVOWEL) {
				DoAmplitude(p->amp, NULL);
				DoPitch(envelope_data[p->env], p->pitch1, p->pitch2);
			} else if (next->type == phLIQUID) {
				DoAmplitude(next->amp, NULL);
				DoPitch(envelope_data[next->env], next->pitch1, next->pitch2);
			} else {
				if (last_pitch_cmd < 0) {
					DoAmplitude(p->amp, NULL);
					DoPitch(envelope_data[p->env], p->pitch1, p->pitch2);
				}
			}

			if ((next->type == phVOWEL) || ((next->type == phLIQUID) && (next->newword == 0))) // ?? test 14.Aug.2007
				StartSyllable();
			else
				p->synthflags |= SFLAG_NEXT_PAUSE;
			InterpretPhonemeWithLength(NULL, 0, p, phoneme_list, &phdata, &worddata, *n_ph);
			memset(&fmtp, 0, sizeof(fmtp));
			fmtp.std_length = phdata.pd_param[i_SET_LENGTH]*2;
			fmtp.fmt_addr = phdata.sound_addr[pd_FMT];
			fmtp.fmt_amp = phdata.sound_param[pd_FMT];
			fmtp.wav_addr = phdata.sound_addr[pd_ADDWAV];
			fmtp.wav_amp = phdata.sound_param[pd_ADDWAV];

			if (p->synthflags & SFLAG_LENGTHEN)
				DoSpect2(p->ph, 0, &fmtp, p, 0);
			DoSpect2(p->ph, 0, &fmtp, p, 0);
			break;
		case phNASAL:
			memset(&fmtp, 0, sizeof(fmtp));
			if (!(p->synthflags & SFLAG_SEQCONTINUE)) {
				DoAmplitude(p->amp, NULL);
				DoPitch(envelope_data[p->env], p->pitch1, p->pitch2);
			}

			if (prev->type == phNASAL)
				last_frame = NULL;

			InterpretPhonemeWithLength(NULL, 0, p, phoneme_list, &phdata, &worddata, *n_ph);
			fmtp.std_length = phdata.pd_param[i_SET_LENGTH]*2;
			fmtp.fmt_addr = phdata.sound_addr[pd_FMT];
			fmtp.fmt_amp = phdata.sound_param[pd_FMT];

			if (next->type == phVOWEL) {
				StartSyllable();
				DoSpect2(p->ph, 0, &fmtp, p, 0);
			} else if (prev->type == phVOWEL && (p->synthflags & SFLAG_SEQCONTINUE))
				DoSpect2(p->ph, 0, &fmtp, p, 0);
			else {
				last_frame = NULL; // only for nasal ?
				DoSpect2(p->ph, 0, &fmtp, p, 0);
				last_frame = NULL;
			}

			break;
		case phLIQUID:
			memset(&fmtp, 0, sizeof(fmtp));
			modulation = 0;
			if (p->ph->phflags & phTRILL)
				modulation = 5;

			if (!(p->synthflags & SFLAG_SEQCONTINUE)) {
				DoAmplitude(p->amp, NULL);
				DoPitch(envelope_data[p->env], p->pitch1, p->pitch2);
			}

			if (prev->type == phNASAL)
				last_frame = NULL;

			if (next->type == phVOWEL)
				StartSyllable();
			InterpretPhonemeWithLength(NULL, 0, p, phoneme_list, &phdata, &worddata, *n_ph);

			if ((value = (phdata.pd_param[i_PAUSE_BEFORE] - p->prepause)) > 0)
				DoPause(value, 1);
			fmtp.std_length = phdata.pd_param[i_SET_LENGTH]*2;
			fmtp.fmt_addr = phdata.sound_addr[pd_FMT];
			fmtp.fmt_amp = phdata.sound_param[pd_FMT];
			fmtp.wav_addr = phdata.sound_addr[pd_ADDWAV];
			fmtp.wav_amp = phdata.sound_param[pd_ADDWAV];
			DoSpect2(p->ph, 0, &fmtp, p, modulation);
			break;
		case phVOWEL:
			ph = p->ph;
			stress = p->stresslevel & 0xf;

			memset(&fmtp, 0, sizeof(fmtp));

			InterpretPhonemeWithLength(NULL, 0, p, phoneme_list, &phdata, &worddata, *n_ph);
			fmtp.std_length = phdata.pd_param[i_SET_LENGTH] * 2;
			vowelstart_prev = 0;

			if (((fmtp.fmt_addr = phdata.sound_addr[pd_VWLSTART]) != 0) && ((phdata.pd_control & pd_FORNEXTPH) == 0)) {
				// a vowel start has been specified by the Vowel program
				fmtp.fmt_length = phdata.sound_param[pd_VWLSTART];
			} else if (prev->type != phPAUSE) {
				// check the previous phoneme
				InterpretPhonemeWithLength(NULL, 0, prev, phoneme_list, &phdata_prev, NULL, *n_ph);
				if (((fmtp.fmt_addr = phdata_prev.sound_addr[pd_VWLSTART]) != 0) && (phdata_prev.pd_control & pd_FORNEXTPH)) {
					// a vowel start has been specified by the previous phoneme
					vowelstart_prev = 1;
					fmtp.fmt2_lenadj = phdata_prev.sound_param[pd_VWLSTART];
				}
				fmtp.transition0 = phdata_prev.vowel_transition[0];
				fmtp.transition1 = phdata_prev.vowel_transition[1];
			}

			if (fmtp.fmt_addr == 0) {
				// use the default start for this vowel
				fmtp.use_vowelin = 1;
				fmtp.fmt_control = 1;
				fmtp.fmt_addr = phdata.sound_addr[pd_FMT];
			}

			fmtp.fmt_amp = phdata.sound_param[pd_FMT];

			pitch_env = envelope_data[p->env];
			amp_env = NULL;
			if (p->tone_ph != 0) {
				InterpretPhoneme2WithData(p->tone_ph, TonePhoneme(p), &phdata_tone);
				pitch_env = GetEnvelope(phdata_tone.pitch_env);
				if (phdata_tone.amp_env > 0)
					amp_env = GetEnvelope(phdata_tone.amp_env);
			}

			StartSyllable();

			modulation = 2;
			if (stress <= 1)
				modulation = 1; // 16ths
			else if (stress >= 7)
				modulation = 3;

			if (prev->type == phVSTOP || prev->type == phVFRICATIVE) {
				DoAmplitude(p->amp, amp_env);
				DoPitch(pitch_env, p->pitch1, p->pitch2); // don't use prevocalic rising tone
				DoSpect2(ph, 1, &fmtp, p, modulation);
			} else if (prev->type == phLIQUID || prev->type == phNASAL) {
				DoAmplitude(p->amp, amp_env);
				DoSpect2(ph, 1, &fmtp, p, modulation); // continue with pre-vocalic rising tone
				DoPitch(pitch_env, p->pitch1, p->pitch2);
			} else if (vowelstart_prev) {
				// VowelStart from the previous phoneme, but not phLIQUID or phNASAL
				DoPitch(envelope_data[PITCHrise], p->pitch2 - 15, p->pitch2);
				DoAmplitude(p->amp-1, amp_env);
				DoSpect2(ph, 1, &fmtp, p, modulation); // continue with pre-vocalic rising tone
				DoPitch(pitch_env, p->pitch1, p->pitch2);
			} else {
				if (!(p->synthflags & SFLAG_SEQCONTINUE)) {
					DoAmplitude(p->amp, amp_env);
					DoPitch(pitch_env, p->pitch1, p->pitch2);
				}

				DoSpect2(ph, 1, &fmtp, p, modulation);
			}

			if ((option_phoneme_events) && (done_phoneme_marker == false)) {
				//WritePhMnemonic(phoneme_name, p->ph, p, use_ipa, NULL);
				WritePhMnemonicWithStress(phoneme_name, p->ph, p, use_ipa, NULL);

				DoPhonemeMarker(espeakEVENT_PHONEME, sourceix, 0, phoneme_name);
			}

			fmtp.fmt_addr = phdata.sound_addr[pd_FMT];
			fmtp.fmt_amp = phdata.sound_param[pd_FMT];
			fmtp.transition0 = 0;
			fmtp.transition1 = 0;

			if ((fmtp.fmt2_addr = phdata.sound_addr[pd_VWLEND]) != 0)
				fmtp.fmt2_lenadj = phdata.sound_param[pd_VWLEND];
			else if (next->type != phPAUSE) {
				fmtp.fmt2_lenadj = 0;
				InterpretPhonemeWithLength(NULL, 0, next, phoneme_list, &phdata_next, NULL, *n_ph);

				fmtp.use_vowelin = 1;
				fmtp.transition0 = phdata_next.vowel_transition[2]; // always do vowel_transition, even if ph_VWLEND ??  consider [N]
				fmtp.transition1 = phdata_next.vowel_transition[3];

				if ((fmtp.fmt2_addr = phdata_next.sound_addr[pd_VWLEND]) != 0)
					fmtp.fmt2_lenadj = phdata_next.sound_param[pd_VWLEND];
			}

			DoSpect2(ph, 2, &fmtp, p, modulation);
			break;
		}
		ix++;
	}
	EndPitch(1);
	if (*n_ph > 0) {
		DoMarker(espeakEVENT_END, count_characters, 0, count_sentences); // end of clause
		*n_ph = 0;
	}

	return 0; // finished the phoneme list
}
/* End legacy generate. */
#else
_Static_assert(sizeof(RustGenerateEntry) == 36 && sizeof(FMT_PARAMS) == 48, "generate snapshot layout");

static WORD_PH_DATA generate_worddata;
static int generate_count;

static const unsigned char *GenerateEnvelope(const RustGenerateEffect *e)
{
	if (e->envelope == 1)
		return envelope_data[e->envelope_value];
	if (e->envelope == 2)
		return GetEnvelope(e->envelope_value);
	return NULL;
}

static int GenerateEffect(void *context, RustGenerateEffect *e)
{
	PHONEME_LIST *list = context;
	PHONEME_LIST *p = &list[e->index];

	switch (e->op)
	{
	case 0:
		return WcmdqFree();
	case 1:
		pitch_length = 0;
		amp_length = 0;
		last_frame = NULL;
		last_wcmdq = -1;
		syllable_start = wcmdq_tail;
		syllable_end = wcmdq_tail;
		syllable_centre = -1;
		last_pitch_cmd = -1;
		memset(&generate_worddata, 0, sizeof(generate_worddata));
		break;
	case 2:
		return last_pitch_cmd >= 0;
	case 3:
		if (output_hooks && output_hooks->outputPhoSymbol) {
			char buf[30];
			int dummy = 0;
			WritePhMnemonicWithStress(buf, p->ph, p, 0, &dummy);
			DoPhonemeAlignment(strdup(buf), p->type);
		}
		break;
	case 4: {
		int embix = *e->embedded_ix;
		DoEmbedded(&embix, e->a);
		*e->embedded_ix = embix;
		break;
	}
	case 5:
		last_frame = NULL;
		break;
	case 6:
		DoMarker(e->index, e->a, e->b, e->c);
		break;
	case 7: {
		char phoneme_name[16];
		WritePhMnemonicWithStress(phoneme_name, p->ph, p, e->a, NULL);
		DoPhonemeMarker(espeakEVENT_PHONEME, e->b, 0, phoneme_name);
		break;
	}
	case 8:
		EndAmplitude();
		break;
	case 9:
		EndPitch(e->a);
		break;
	case 10:
		DoPause(e->a, e->b);
		break;
	case 11:
		DoAmplitude(e->a, GenerateEnvelope(e));
		break;
	case 12:
		DoPitch(GenerateEnvelope(e), e->a, e->b);
		break;
	case 13:
		StartSyllable();
		break;
	case 14:
		InterpretPhonemeWithLength(NULL, e->a, p, list, e->data, e->b ? &generate_worddata : NULL, generate_count);
		break;
	case 15: {
		PHONEME_TAB *tone = TonePhoneme(p);
		if (tone == NULL)
			return -1;
		InterpretPhoneme2WithData(p->tone_ph, tone, e->data);
		break;
	}
	case 16:
		DoSpect2(p->ph, e->a, e->fmt, p, e->b);
		break;
	case 17:
		DoSample3(e->data, e->a, e->b);
		break;
	case 18:
		p->synthflags = e->a;
		break;
	case 19:
		p->std_length = e->a;
		break;
	}
	return 0;
}

int Generate(PHONEME_LIST *phoneme_list, int *n_ph, bool resume)
{
	// Position within the clause, kept across suspensions.
	static RustGenerateState state;
	// The engine is serialized; the snapshot is too large for the stack.
	static RustGenerateEntry entries[N_PHONEME_LIST+1];
	RustGenerateSettings settings;
	size_t count;
	size_t m;
	int rc;

#if USE_MBROLA
	if (mbrola_name[0] != 0)
		return MbrolaGenerate(phoneme_list, n_ph, resume);
#endif

	count = *n_ph > 0 ? (size_t)*n_ph : 0;
	// the clause and the two neighbours read after its last entry
	m = count + 2 < N_PHONEME_LIST+1 ? count + 2 : N_PHONEME_LIST+1;
	for (size_t ix = 0; ix < m; ix++) {
		const PHONEME_LIST *p = &phoneme_list[ix];
		RustGenerateEntry *e = &entries[ix];
		memset(e, 0, sizeof(*e));
		if (p->ph != NULL) {
			e->phoneme = *p->ph;
			e->present = 1;
		}
		e->length = p->length;
		e->synthflags = p->synthflags;
		e->source = p->sourceix;
		e->type = p->type;
		e->newword = p->newword;
		e->prepause = p->prepause;
		e->amp = p->amp;
		e->env = p->env;
		e->pitch1 = p->pitch1;
		e->pitch2 = p->pitch2;
		e->stress = p->stresslevel;
		e->tone = p->tone_ph;
	}
	settings.phoneme_events = option_phoneme_events;
	settings.word_merge = translator->langopts.param[LOPT_WORD_MERGE];
	settings.clause_start_char = clause_start_char;
	settings.clause_start_word = clause_start_word;
	settings.count_sentences = count_sentences;
	settings.count_characters = count_characters;
	generate_count = *n_ph;

	rc = espeak_rs_generate(entries, m, &count, resume, &state, &settings, phoneme_list, GenerateEffect);
	if (rc == 1)
		return 1; // wait
	if (rc != 0) {
		// only lists the legacy loop would read out of bounds reach here
		*n_ph = 0;
		return 0;
	}
	*n_ph = (int)count;
	return 0;
}
#endif

int SpeakNextClause(int control)
{
	// Speak text from memory (text_in)
	// control 0: start
	//    text_in is set

	// The other calls have text_in = NULL
	// control 1: speak next text
	//         2: stop

	int clause_tone;
	char *voice_change;

	if (control == 2) {
		// stop speaking
		n_phoneme_list = 0;
		WcmdqStop();

		return 0;
	}

	if (text_decoder_eof(p_decoder)) {
		skipping_text = false;
		return 0;
	}

	SelectPhonemeTable(voice->phoneme_tab_ix);

	// read the next clause from the input text file, translate it, and generate
	// entries in the wavegen command queue
	TranslateClause(translator, &clause_tone, &voice_change);

	CalcPitches(translator, clause_tone);
	CalcLengths(translator);

	if ((option_phonemes & 0xf) || (phoneme_callback != NULL)) {
		const char *phon_out;
		phon_out = GetTranslatedPhonemeString(option_phonemes);
		if (option_phonemes & 0xf)
			fprintf(f_trans, "%s\n", phon_out);
		if (phoneme_callback != NULL)
			phoneme_callback(phon_out);
	}

	if (skipping_text) {
		n_phoneme_list = 0;
		return 1;
	}

	Generate(phoneme_list, &n_phoneme_list, 0);

	if (voice_change != NULL) {
		// voice change at the end of the clause (i.e. clause was terminated by a voice change)
		new_voice = LoadVoiceVariant(voice_change, 0); // add a Voice instruction to wavegen at the end of the clause
	}

	if (new_voice) {
		// finished the current clause, now change the voice if there was an embedded
		// change voice command at the end of it (i.e. clause was broken at the change voice command)
		DoVoiceChange(voice);
		new_voice = NULL;
	}

	return 1;
}

#pragma GCC visibility push(default)
ESPEAK_API void espeak_SetPhonemeCallback(int (*PhonemeCallback)(const char *))
{
	phoneme_callback = PhonemeCallback;
}
#pragma GCC visibility pop
