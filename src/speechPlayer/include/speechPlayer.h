/*
This file is a part of the NV Speech Player project. 
URL: https://bitbucket.org/nvaccess/speechplayer
Copyright 2014 NV Access Limited.
This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License, as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.
This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
This license can be found at:
http://www.gnu.org/licenses/gpl.html
*/

#ifndef SPEECHPLAYER_H
#define SPEECHPLAYER_H

#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef double speechPlayer_frameParam_t;

typedef struct {
	// voicing and cascaide
	speechPlayer_frameParam_t voicePitch; //  fundermental frequency of voice (phonation) in hz
	speechPlayer_frameParam_t vibratoPitchOffset; // pitch is offset up or down in fraction of a semitone
	speechPlayer_frameParam_t vibratoSpeed; // Speed of vibrato in hz
	speechPlayer_frameParam_t voiceTurbulenceAmplitude; // amplitude of voice breathiness from 0 to 1 
	speechPlayer_frameParam_t glottalOpenQuotient; // fraction between 0 and 1 of a voice cycle that the glottis is open (allows voice turbulance, alters f1...)
	speechPlayer_frameParam_t voiceAmplitude; // amplitude of voice (phonation) source between 0 and 1.
	speechPlayer_frameParam_t aspirationAmplitude; // amplitude of aspiration (voiceless h, whisper) source between 0 and 1.
	speechPlayer_frameParam_t cf1, cf2, cf3, cf4, cf5, cf6, cfN0, cfNP; // frequencies of standard cascaide formants, nasal (anti) 0 and nasal pole in hz
	speechPlayer_frameParam_t cb1, cb2, cb3, cb4, cb5, cb6, cbN0, cbNP; // bandwidths of standard cascaide formants, nasal (anti) 0 and nasal pole in hz
	speechPlayer_frameParam_t caNP; // amplitude from 0 to 1 of cascade nasal pole formant
	// fricatives and parallel
	speechPlayer_frameParam_t fricationAmplitude; // amplitude of frication noise from 0 to 1.
	speechPlayer_frameParam_t pf1, pf2, pf3, pf4, pf5, pf6; // parallel formants in hz
	speechPlayer_frameParam_t pb1, pb2, pb3, pb4, pb5, pb6; // parallel formant bandwidths in hz
	speechPlayer_frameParam_t pa1, pa2, pa3, pa4, pa5, pa6; // amplitude of parallel formants between 0 and 1
	speechPlayer_frameParam_t parallelBypass; // amount of signal which should bypass parallel resonators from 0 to 1
	speechPlayer_frameParam_t preFormantGain; // amplitude from 0 to 1 of all vocal tract sound (voicing, frication) before entering formant resonators. Useful for stopping/starting speech
	speechPlayer_frameParam_t outputGain; // amplitude from 0 to 1 of final output (master volume) 
	speechPlayer_frameParam_t endVoicePitch; //  pitch of voice at the end of the frame length 
} speechPlayer_frame_t;

typedef short sampleVal;

typedef struct {
	sampleVal value;
} sample;

typedef void* speechPlayer_handle_t;

speechPlayer_handle_t speechPlayer_initialize(int sampleRate);
void speechPlayer_queueFrame(speechPlayer_handle_t playerHandle, speechPlayer_frame_t* framePtr, unsigned int minFrameDuration, unsigned int fadeDuration, int userIndex, bool purgeQueue);
int speechPlayer_synthesize(speechPlayer_handle_t playerHandle, unsigned int sampleCount, sample* sampleBuf); 
int speechPlayer_getLastIndex(speechPlayer_handle_t playerHandle);
void speechPlayer_terminate(speechPlayer_handle_t playerHandle);

/* Native Rust-core checked admission. Up to 64 live owners, with 256 pending
 * frame requests per owner. Frame pointers are aligned initialized readable
 * objects. Calls copy frames and retain no caller buffers.
 * A non-null frame needs minFrameDuration > 0; fadeDuration zero selects one.
 * NULL frames fade to silence; NaN target parameters retain the preceding value
 * during interpolation. Callers serialize each owner and drain calls before
 * termination. PCM is initialized, aligned, writable and disjoint from result.
 * Retired initialized owner buffers remain in a bounded process-wide pool and
 * reset in place on reuse. Owners have never-reused identities; stale handles
 * refuse. No engine
 * state loan spans a callback, and synthesis adds no scheduler or worker.
 * Checked APIs:0 accepted, EINVAL invalid/stale, EAGAIN active/capacity refusal.
 * Refusal leaves queue/result unchanged. Legacy void APIs cannot report refusal;
 * bounded callers use these status APIs for backpressure. */
#ifdef USE_RUST_CORE
int espeak_rs_speech_player_queue_frame(speechPlayer_handle_t, const speechPlayer_frame_t *, unsigned int, unsigned int, int, bool);
int espeak_rs_speech_player_synthesize(speechPlayer_handle_t, unsigned int, sample *, unsigned int *);
int espeak_rs_speech_player_terminate(speechPlayer_handle_t);
#endif

#ifdef __cplusplus
}
#endif

#endif
