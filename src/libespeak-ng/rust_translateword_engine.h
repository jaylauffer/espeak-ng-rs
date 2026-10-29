/* The translateword.c entry point that runs in Rust against the engine's
 * state (rust/translateword_engine_compat.rs). SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_TRANSLATEWORD_ENGINE_H
#define ESPEAK_NG_RUST_TRANSLATEWORD_ENGINE_H
#include <stdbool.h>
#include <stddef.h>
int espeak_rs_translate_word3_engine(void *, char *, void *, int, char *, bool *, const void *, char *, size_t);
#endif
