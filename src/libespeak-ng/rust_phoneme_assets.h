/* Complete owned resident phoneme reload/free/table selection.
 * SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_PHONEME_ASSETS_H
#define ESPEAK_RUST_PHONEME_ASSETS_H
#include <stddef.h>
#include <stdint.h>
enum {RUST_ASSETS_LOAD,RUST_ASSETS_RELEASE,RUST_ASSETS_SELECT,RUST_ASSETS_LOOKUP,RUST_ASSETS_SELECT_NAME};
typedef struct {unsigned char name[32];void *records;int32_t count,includes;} RustPhonemeTableView;
typedef struct {
 unsigned char *buffers[4];int32_t lengths[4];unsigned char *wave;
 int32_t tables_count,active,number,current,tune_count;
 RustPhonemeTableView tables[150];void *selected[256];
} RustPhonemeAssetsView;
typedef struct {
 void *context;const char *home,*name;size_t width;
 int32_t separator,number,argument,operation;
 void (*project)(void *,const RustPhonemeAssetsView *);
 int (*file_error)(void *,int,const char *);
 int (*version_error)(void *,const char *,int,int);
} RustPhonemeAssets;
typedef struct {int32_t rate,number;} RustPhonemeAssetsResult;
/* Serialize the whole engine and all raw resident consumers. Paths initialized
 * through NUL within width (2..4096), names within 4096; full respective extent
 * initialized on rejection. Descriptor/text copied before callbacks/I/O.
 * Project scratch must be copied synchronously, never retained. Its buffer/
 * row pointers are quiescent owned views, valid only until next load/release;
 * C may read/mutate bytes after control returns, never during callbacks/I/O.
 * Outcome aligned/writable, disjoint from globals/context, may overlap inputs.
 * Admission/reentry leaves outcome/global views untouched. Runtime failure
 * publishes a valid revoked/partial resident projection, outcome untouched.
 * Release revokes all pointers before storage destruction. No C owner token.
 * number imports the caller's requested table; argument selects a table.
 * Successful lookup/name returns its choice in outcome.number; the frontend
 * mirrors that scalar. Queries, misses and cached selections do not project.
 */
int espeak_rs_phoneme_assets(const RustPhonemeAssets *,RustPhonemeAssetsResult *);
#endif
