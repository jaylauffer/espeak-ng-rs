/* Native CLI configuration and process entry; command policy belongs to Rust. */
/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "config.h"
#include <getopt.h>
#include <stddef.h>
#include <stdio.h>
#include <espeak-ng/espeak_ng.h>
#include <espeak-ng/speak_lib.h>
#if defined(_WIN32) || defined(_WIN64)
#include <fcntl.h>
#include <io.h>
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
	int (*next_option)(int, char **, const char *, const struct option *, int *, char **, int *);
	void (*binary_stdout)(FILE *);
} RustCliConfig;

_Static_assert(sizeof(RustCliConfig) == 8 + 7 * sizeof(void *) &&
               offsetof(RustCliConfig, binary_stdout) == 8 + 6 * sizeof(void *),
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

/* Keep platform getopt's permutation, diagnostics and optional-argument ABI. */
static int next_option(int argc, char **argv, const char *short_options,
                       const struct option *options, int *index,
                       char **argument, int *position)
{
	int result = getopt_long(argc, argv, short_options, options, index);
	*argument = optarg;
	*position = optind;
	return result;
}

static void binary_stdout(FILE *stream)
{
#if defined(_WIN32) || defined(_WIN64)
	_setmode(_fileno(stream), _O_BINARY);
#else
	(void)stream;
#endif
}

extern int espeak_rs_cli_main(int, char **, const RustCliConfig *);

int main(int argc, char **argv)
{
	const RustCliConfig config = {
		PLAYBACK_MODE, USE_MBROLA, PROGRAM_NAME, ESPEAKNG_DEFAULT_VOICE,
		stdin, stdout, stderr, next_option, binary_stdout
	};
	return espeak_rs_cli_main(argc, argv, &config);
}
