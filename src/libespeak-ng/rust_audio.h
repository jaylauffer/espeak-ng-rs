/*
 * Playback through the Rust core's audio sink, with pcaudio's
 * audio_object calls so speech.c keeps one code path. Writes and drains
 * wait on the sink's proactor; a flush from any thread releases them.
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 3 of the License, or
 * (at your option) any later version.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#ifndef ESPEAK_NG_RUST_AUDIO_H
#define ESPEAK_NG_RUST_AUDIO_H

#include <stddef.h>

struct audio_object;

struct audio_object *espeak_rs_audio_create(const char *device);
void espeak_rs_audio_destroy(struct audio_object *audio);
int espeak_rs_audio_open(struct audio_object *audio, int rate);
void espeak_rs_audio_close(struct audio_object *audio);
int espeak_rs_audio_write(struct audio_object *audio, const short *samples, size_t bytes);
int espeak_rs_audio_drain(struct audio_object *audio);
int espeak_rs_audio_flush(struct audio_object *audio);
const char *espeak_rs_audio_strerror(struct audio_object *audio, int error);
// Milliseconds of audio queued for the device.
int espeak_rs_audio_latency_ms(struct audio_object *audio);

#define AUDIO_OBJECT_FORMAT_S16LE 0

static inline struct audio_object *
create_audio_device_object(const char *device, const char *application_name, const char *description)
{
	(void)application_name;
	(void)description;
	return espeak_rs_audio_create(device);
}

static inline int
audio_object_open(struct audio_object *audio, int format, int rate, int channels)
{
	(void)format; // always 16-bit mono
	(void)channels;
	return espeak_rs_audio_open(audio, rate);
}

static inline void audio_object_close(struct audio_object *audio) { espeak_rs_audio_close(audio); }
static inline void audio_object_destroy(struct audio_object *audio) { espeak_rs_audio_destroy(audio); }
static inline int audio_object_drain(struct audio_object *audio) { return espeak_rs_audio_drain(audio); }
static inline int audio_object_flush(struct audio_object *audio) { return espeak_rs_audio_flush(audio); }

static inline int
audio_object_write(struct audio_object *audio, const void *data, size_t bytes)
{
	return espeak_rs_audio_write(audio, (const short *)data, bytes);
}

static inline const char *
audio_object_strerror(struct audio_object *audio, int error)
{
	return espeak_rs_audio_strerror(audio, error);
}

#endif
