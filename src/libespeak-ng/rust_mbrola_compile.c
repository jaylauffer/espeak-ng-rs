/* Native path constants and the borrowed C stderr primitive. */
/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "speech.h"
#include <stdio.h>
static FILE *compiler_stderr(void) { return stderr; }
const struct {
 size_t width, name_max;
 FILE *(*standard_error)(void);
} espeak_rs_mbrola_compile_config = { N_PATH_BUF, NAME_MAX, compiler_stderr };
