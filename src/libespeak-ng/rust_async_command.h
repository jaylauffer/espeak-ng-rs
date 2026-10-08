/* Owned Rust commands; C sees only the espeak_command.h prefix.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_ASYNC_COMMAND_H
#define ESPEAK_NG_RUST_ASYNC_COMMAND_H
#include "espeak_command.h"
typedef struct {
    espeak_ng_STATUS (*synth)(unsigned int, const void *, unsigned int,
                            espeak_POSITION_TYPE, unsigned int, unsigned int, void *);
    espeak_ng_STATUS (*mark)(unsigned int, const void *, const char *,
                           unsigned int, unsigned int, void *);
    espeak_ng_STATUS (*key)(const char *);
    espeak_ng_STATUS (*character)(wchar_t);
    espeak_ng_STATUS (*parameter)(int, int, int);
    void (*punctuation)(const wchar_t *);
    espeak_ERROR (*voice_name)(const char *);
    espeak_ERROR (*voice)(espeak_VOICE *);
    int (*terminated)(unsigned int, void *);
} RustAsyncCommandCallbacks;
t_espeak_command *espeak_rs_async_command_create(const t_espeak_command *, size_t);
void espeak_rs_async_command_process(t_espeak_command *, const RustAsyncCommandCallbacks *);
int espeak_rs_async_command_delete(t_espeak_command *, const RustAsyncCommandCallbacks *);
#endif
