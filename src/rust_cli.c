/* Native CLI configuration and process entry; command policy belongs to Rust. */
/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include <getopt.h>
#include <stddef.h>
#include <stdio.h>
#include <espeak-ng/espeak_ng.h>
#include <espeak-ng/speak_lib.h>

#ifndef USE_MBROLA
/* As the original's `#if USE_MBROLA` reads a build that does not define it
   (the Visual Studio solution's CLI project). */
#define USE_MBROLA 0
#endif

#ifndef PROGRAM_NAME
#define PROGRAM_NAME "espeak-ng"
#endif
#ifndef PLAYBACK_MODE
#define PLAYBACK_MODE ENOUTPUT_MODE_SPEAK_AUDIO
#endif

typedef struct {
	int playback_mode, mbrola;
	const char *program_name, *default_voice;
	FILE *input, *output, *error;
	/* The platform's getopt (src/compat/getopt.c with MSVC) and its state. */
	int (*getopt_long)(int, char *const *, const char *, const struct option *, int *);
	char **optarg;
	int *optind;
} RustCliConfig;

_Static_assert(sizeof(RustCliConfig) == 8 + 8 * sizeof(void *) &&
               offsetof(RustCliConfig, optind) == 8 + 7 * sizeof(void *),
               "native CLI configuration layout");
_Static_assert(sizeof(struct option) == 4 * sizeof(void *) &&
               offsetof(struct option, val) == 3 * sizeof(void *),
               "native getopt layout");
_Static_assert(sizeof(espeak_EVENT) == 32 + sizeof(void *) &&
               offsetof(espeak_EVENT, id) == 24 + sizeof(void *) &&
               sizeof(espeak_VOICE) == 8 + 4 * sizeof(void *) &&
               offsetof(espeak_VOICE, spare) == 8 + 3 * sizeof(void *),
               "native CLI event/voice layout");
_Static_assert(AUDIO_OUTPUT_SYNCHRONOUS == 2 && ENOUTPUT_MODE_SYNCHRONOUS == 1 &&
               espeakINITIALIZE_DONT_EXIT == 0x8000 && POS_CHARACTER == 1 &&
               espeakCHARS_8BIT == 2 && espeakPHONEMES == 0x100 &&
               espeakENDPAUSE == 0x1000 && espeakSSML == 0x10 &&
               espeakPHONEMES_SHOW == 1 && espeakPHONEMES_IPA == 2 &&
               espeakPHONEMES_TRACE == 8 && espeakPHONEMES_MBROLA == 16 &&
               espeakPHONEMES_TIE == 128 && espeakRATE == 1 && espeakVOLUME == 2 &&
               espeakPITCH == 3 && espeakRANGE == 4 && espeakPUNCTUATION == 5 &&
               espeakCAPITALS == 6 && espeakWORDGAP == 7 &&
               espeakSSML_BREAK_MUL == 10 && espeakLINELENGTH == 13,
               "native CLI constants");

extern int espeak_rs_cli_main(int, char **, const RustCliConfig *);

int main(int argc, char **argv)
{
	const RustCliConfig config = {
		PLAYBACK_MODE, USE_MBROLA, PROGRAM_NAME, ESPEAKNG_DEFAULT_VOICE,
		stdin, stdout, stderr, getopt_long, &optarg, &optind
	};
	return espeak_rs_cli_main(argc, argv, &config);
}
