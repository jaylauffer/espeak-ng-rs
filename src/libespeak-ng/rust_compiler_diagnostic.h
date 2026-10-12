/* Native compiler diagnostic policy and bounded loans.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_COMPILER_DIAGNOSTIC_H
#define ESPEAK_RUST_COMPILER_DIAGNOSTIC_H
#include "rust_compiler_context.h"
typedef struct {
 RustCompilerContext *context;int status;const char *name;size_t width;
 void (*message)(int,char *,size_t);
} RustCompilerDiagnosticStatus;
/* Live creating-thread Rust context; serialized engine/FILE access. Byte loans
 * initialized for length, copied before publication, may overlap context fields.
 * Text/item max 4096 bytes, table <256, resource <4096. NUL precision matches C.
 * Diagnostic FILE is borrowed, never closed; ignored short writes match C.
 * Prefix/count read between effects, no retained native context references.
 * Status descriptor/name copies before effects; selected name terminates within
 * width (1..4096), full width initialized on refusal. Message callback fills a
 * terminated 512-byte result, cannot retain loans, invalidate views or unwind.
 * Diagnostic/source/resource/end reentry and wrong-thread admission refuse;
 * diagnostics may run under an already admitted native resource effect.
 * Counter/line overflow and unterminated results refuse without C replay.
 */
int espeak_rs_compiler_diagnostic_text(void *,const unsigned char *,size_t);
int espeak_rs_compiler_diagnostic_item(void *,int,const unsigned char *,size_t);
void espeak_rs_compiler_diagnostic_table(void *,int,int,const unsigned char *,size_t);
void espeak_rs_compiler_diagnostic_resource(void *,int,int,int,const unsigned char *,size_t);
int espeak_rs_compiler_diagnostic_status(const RustCompilerDiagnosticStatus *);
#endif
