/* Typed native translator ownership; serialized internal compatibility ABI.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_NG_RUST_TRANSLATOR_H
#define ESPEAK_NG_RUST_TRANSLATOR_H
#include "translate.h"
/* Create returns a Rust allocation with the exact Translator field layout.
 * All native records passed to load/destroy must come from create; a C malloc
 * or stack fixture is not an owned native translator. C-only allocations keep
 * their retained destructor. Name is retained terminated immutable storage.
 * Serialize accesses; no Rust reference crosses host effects. Destroy once
 * after calls/views drain. Pinned Rust dictionaries outlive cache destruction;
 * only successfully rebound views release the previous unique small handle. */
Translator *espeak_rs_translator_create(const char *);
void espeak_rs_translator_destroy(Translator *);
/* Setup/worker I/O through the existing engine proactor. Cache remains uniquely
 * admitted; owner is borrowed only after I/O. Initialized path/name survive the
 * call, name may alias owner's dictionary_name. Failed loads preserve owner
 * views/name. Status 0 success,1 I/O/empty,2 invalid,3 allocation,4 bounds/pins. */
int espeak_rs_translator_load(Translator *,void *,const char *,const char *);
size_t espeak_rs_translator_layout(unsigned,unsigned);
#endif
