/* Native legacy API and owned error-context adapters.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_LEGACY_API_H
#define ESPEAK_NG_RUST_LEGACY_API_H
#include <stdio.h>
#include "error.h"
typedef struct {
    void (*path)(const char *);
    espeak_ng_STATUS (*initialize)(espeak_ng_ERROR_CONTEXT *);
    espeak_ng_STATUS (*output)(espeak_ng_OUTPUT_MODE, int, const char *);
    int (*rate)(void);
    void (*print)(espeak_ng_STATUS, FILE *, espeak_ng_ERROR_CONTEXT);
    void (*clear)(espeak_ng_ERROR_CONTEXT *);
    espeak_ng_STATUS (*compile)(const char *, const char *, FILE *, int, espeak_ng_ERROR_CONTEXT *);
    void (*exit_process)(int);
} RustLegacyApi;
typedef struct {
    void (*lock)(FILE *);
    void (*write)(FILE *, const unsigned char *, size_t);
    void (*unlock)(FILE *);
    void (*errno_message)(espeak_ng_STATUS, char *, size_t);
} RustStatusIo;
int espeak_rs_legacy_status(espeak_ng_STATUS);
int espeak_rs_legacy_initialize(const RustLegacyApi *, FILE *, int *, int, int, const char *, int);
void espeak_rs_legacy_compile(const RustLegacyApi *, FILE *, const char *, const char *, FILE *, int);
espeak_ng_STATUS espeak_rs_error_file(espeak_ng_ERROR_CONTEXT *, espeak_ng_STATUS, const char *);
espeak_ng_STATUS espeak_rs_error_version(espeak_ng_ERROR_CONTEXT *, const char *, int, int);
void espeak_rs_error_clear(espeak_ng_ERROR_CONTEXT *);
void espeak_rs_error_message(espeak_ng_STATUS, char *, size_t, void (*)(espeak_ng_STATUS, char *, size_t));
void espeak_rs_error_print(espeak_ng_STATUS, FILE *, espeak_ng_ERROR_CONTEXT, const RustStatusIo *);
#endif
