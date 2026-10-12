/* Owned native dictionary diagnostic bytes. SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef ESPEAK_RUST_DICTIONARY_DIAGNOSTIC_H
#define ESPEAK_RUST_DICTIONARY_DIAGNOSTIC_H
#include <stddef.h>
#include <stdio.h>
typedef struct {
 FILE *log,*error;int *line,*count;unsigned char *group;size_t width;
} RustDictionaryDiagnostic;
enum {RUST_DIAGNOSTIC_LIST,RUST_DIAGNOSTIC_RULE,RUST_DIAGNOSTIC_FILE,RUST_DIAGNOSTIC_DRIVER};
/* Serialized engine; initialized aligned descriptor and live disjoint native
 * selected line/count/group fields, exclusive borrowed FILEs (never closed here).
 * Driver selects only log; list selects count; rule selects line/count and its
 * group for kind8; file selects all fields. Unused fields may be NULL/zero.
 * Selected strings terminate within 4096; all 4096 bytes initialized on reject.
 * Group width 2..256; file input is fully initialized width bytes and contains
 * NUL, rule group is selected only for kind8. Inputs may overlap output views,
 * which are quiescent until all snapshots complete. No input is retained.
 * File family publishes incoming line/group before output; list uses its line
 * argument, rule uses current native line. Preserve stderr rule-cap routing,
 * missing-newline report, unknown-kind counts, ignored FILE errors and driver
 * flushes. Reentry refuses EAGAIN; malformed/overflow admission EINVAL. Counter
 * is read fresh after stream effects; stream callbacks may update scalars but
 * cannot revoke fields/FILEs, retain loans or unwind. Count overflow after a
 * stream callback refuses its undefined old-C increment after the write.
 */
int espeak_rs_dictionary_diagnostic(const RustDictionaryDiagnostic *,int,int,int,int,const char *,const char *,const unsigned char *);
#endif
