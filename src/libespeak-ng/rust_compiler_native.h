/* Native compiler host owns context and all compiler leases in Rust.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_COMPILER_NATIVE_H
#define ESPEAK_RUST_COMPILER_NATIVE_H
#include "rust_compiler_context.h"
#include "rust_compiler_driver.h"
typedef struct {
 void *context;const char *home,*source,*destination;
 const unsigned char *alpha,*spaces;
 size_t width;int windows,keep_prefix_x;uint32_t version;
 FILE *log_file;
 int (*engine)(void *,RustCompilerContext *,int,int);
 void (*log)(void *,const unsigned char *,size_t);
 int (*error)(void *,int,const char *);
 int (*fallback)(void *,const unsigned char *,size_t);
 void (*classes)(unsigned char *,unsigned char *);
} RustNativePhonemeCompiler;
/* Inputs follow RustPhonemeCompiler's bounded initialized admission contract.
 * Descriptor/paths/maps are copied before effects. Native engine callbacks
 * receive a quiescent initialized context only during synchronous primitives;
 * cannot retain/copy/free it, consume its leases or unwind. Whole engine access
 * stays serialized; nested compilation refuses before native allocation.
 * Output-open primitive returns current samplerate; other engine primitives
 * preserve their original status/effects. Classes fills two 256-byte maps.
 * fallback's context is the compiler view, live through output end.
 * log_file is borrowed by context teardown; close-log engine primitive retains
 * prior public API ownership semantics. Rust consumes every admitted compiler
 * context exactly once, including failed setup/source/output admission.
 * Generic espeak_rs_compile_phonemes remains available with its existing ABI.
 */
int espeak_rs_compile_phonemes_native(const RustNativePhonemeCompiler *);
#endif
