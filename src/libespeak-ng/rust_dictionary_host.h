/* Owned native dictionary context and callback host.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_DICTIONARY_HOST_H
#define ESPEAK_RUST_DICTIONARY_HOST_H
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
typedef struct {
 FILE *log, *error;
 char word_phonemes[200];
 int line, count, text_mode, debug, need_dictionary;
 unsigned char letters[95], group[13];
 int group3;
} RustDictionaryContext;
typedef struct {
 unsigned char alnums[256], spaces[256];
 int language_text, listx, letter_offset;
} RustDictionaryEnvironment;
typedef struct {
 RustDictionaryContext *context;
 const char *source, *name, *home, *table;
 size_t width;
 int separator, windows, signed_char;
 void *error_context;
 void (*environment)(int, RustDictionaryEnvironment *);
 int (*encode)(void *,const char *,char *,size_t,int *);
 void (*translate)(char *);
 int (*case_map)(void *,uint32_t,uint32_t *);
 int (*transpose)(void *,char *);
 int (*digit)(uint32_t);
 void (*reload)(void *,const char *);
 int (*file_error)(void *,int,const char *);
} RustNativeDictionaryHost;
/* begin allocates one zeroed owner with flags&1 debug; FILEs are borrowed and
 * must be non-null/live until end. errno is initialized writable and disjoint.
 * The returned view is valid only for its unique Rust owner on the creating
 * thread; never pass a copy or caller-allocated context to host/end. NULL end
 * is harmless. Host and end refuse wrong-thread/reentrant entry with EAGAIN.
 * Host descriptor and four strings (NUL within4096) copy before effects; width
 * is2..4096. NULL strings mean empty. Views may overlap inputs at admission.
 * Context can be inspected/changed only while quiescent or by a synchronous
 * borrowed FILE callback using raw scalar accesses; no view reference may span
 * that effect. Primitives cannot unwind, retain scratch, or access the context.
 * Environment is initialized writable scratch, filled freshly for lists/rules
 * after each preceding driver log. Writer tokens stay internal, with one write
 * in flight in existing bounded proactor pools. No FILE ownership is transferred.
 * Diagnostic counters read fresh after FILE callbacks. Host keeps its owner
 * alive until all nested controllers and diagnostics return. */
RustDictionaryContext *espeak_rs_dictionary_context_begin(FILE *,FILE *,int,int *);
int espeak_rs_dictionary_context_end(RustDictionaryContext *);
int espeak_rs_dictionary_host_compile(const RustNativeDictionaryHost *);
/* The same with the engine's primitives (the current translator, encoder,
 * word translation, case, transposition, digits, reload, error contexts). */
int espeak_rs_dictionary_compile_engine(RustDictionaryContext *, const char *source, const char *name,
    const char *home, const char *table, size_t width, int separator, void *error_context);
#endif
