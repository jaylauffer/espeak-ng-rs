/* Complete definition compiler. SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_COMPILER_DEFINITION_H
#define ESPEAK_RUST_COMPILER_DEFINITION_H
#include "rust_compiler_program.h"
#include "phoneme.h"
typedef struct {
 RustCompilerProgram program;
 const unsigned char *alnum; /* initialized locale map, 256 bytes */
 int32_t (*eof)(void *);
 int32_t (*lookup)(void *,const unsigned char *,size_t,int32_t);
 int32_t (*row)(void *,int32_t,PHONEME_TAB *);
 int32_t (*project)(void *,const PHONEME_TAB *);
 int32_t (*find)(void *,const unsigned char *,size_t,PHONEME_TAB *);
 int32_t (*import)(void *,const unsigned char *,size_t,PHONEME_TAB *);
 void (*unknown)(void *,const unsigned char *,size_t);
} RustCompilerDefinition;
typedef struct { PHONEME_TAB row; int32_t compiled, procedures, status; } RustDefinitionResult;
/* Same serialized callback/input/publication contract as the assembler ABI.
 * Descriptor and locale map copied before effects. No row/word views escape.
 * Uses the existing program owner; compiler/procedure state lasts until End.
 * project publishes a copied current row before dependent primitive effects;
 * find returns 0 missing/1 found; other nonzero row/project/import codes fail.
 * phoneme selects a table definition or procedure; program.control enables log. */
int32_t espeak_rs_compiler_definition(const RustCompilerDefinition *,int32_t phoneme,RustDefinitionResult *);
#endif
