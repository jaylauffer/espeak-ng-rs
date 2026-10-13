/* Native serialized clause postprocessing. GPL-3.0-or-later. */
#ifndef ESPEAK_RUST_CLAUSE_POSTPROCESS_H
#define ESPEAK_RUST_CLAUSE_POSTPROCESS_H
#include "translate.h"
#include <stdbool.h>
/* Stable Rust-owned first/synthesis lists, replacements and speed factors;
 * foreign users borrow them only under whole-engine serialization. Source,
 * translator, table/program/tune/envelope views stay initialized and live;
 * resource reload/teardown must drain calls first. No reference into mutable
 * engine rows crosses callbacks. Retained scratch owns all snapshots and is
 * restored after effects, with no per-clause allocation. EAGAIN refuses a busy
 * scratch owner; EINVAL rejects malformed settings/programs/bounds. Legacy
 * void entry points cannot report that refusal. Invalid counts retain the
 * previous host's no-op/empty-clause behavior. Native errors do not replay C. */
int espeak_rs_postprocess_list(const Translator *, int, bool);
int espeak_rs_postprocess_pitch(const Translator *, int);
int espeak_rs_postprocess_lengths(const Translator *);
#endif
