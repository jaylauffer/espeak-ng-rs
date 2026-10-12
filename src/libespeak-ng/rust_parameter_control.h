/* Native parameter/embedded-speed/length selection. GPL-3.0-or-later. */
#ifndef ESPEAK_RUST_PARAMETER_CONTROL_H
#define ESPEAK_RUST_PARAMETER_CONTROL_H
#include <stddef.h>
#include <stdint.h>
typedef struct {
 const int32_t *defaults;int32_t *current,*saved,*embedded,*line,*gap,*tone,*group;
 void (*speed)(int);int (*amplitude)(void);
} RustParameterControl;
typedef struct {
 const uint32_t *words;size_t length;int32_t *cursor;const int32_t *speeds;
 void (*set)(int,int);void (*speed)(int);
} RustLengthEmbedded;
/* Whole engine access stays serialized. Rust owns the stable 20x64-byte
 * parameter stack, 15 saved values and 15 immutable defaults, under existing
 * C symbol names; unported C reads/writes initialized views in place.
 * Descriptors/initialized input snapshots precede effects/publication. Parameter
 * arrays (15 each) and option scalars are mutually disjoint and writable;
 * defaults/descriptor may overlap outputs. group may be NULL if no group update
 * is required. Rate/volume primitives are synchronous, cannot retain loans or
 * unwind; no foreign reference spans them. Out-of-range index, overflow or
 * missing needed primitive/group refuses with EINVAL before any publication.
 * Unsupported in-range parameters preserve previous current/saved writes before
 * returning EINVAL. Stored pitch/range retain pre-clamp values.
 * Embedded words initialize through first bit-7 terminator, or the entire
 * admitted suffix on rejection. length<=250. Cursor/output/speeds are initialized,
 * aligned and mutually disjoint; words/descriptor may overlap them. Copy source
 * prefix before effects; cursor advances before SetEmbedded and SetSpeed(1).
 * Incomplete span refuses without effects; output reads fresh speeds afterward.
 * Length pointer outputs are disjoint; refusal preserves both. Returned table
 * pointers are immutable Rust storage valid for process lifetime.
 */
int espeak_rs_set_parameter(const RustParameterControl *,int,int,int);
int espeak_rs_length_embedded(const RustLengthEmbedded *,int32_t *);
int espeak_rs_length_mods(int,const unsigned char **,const unsigned char **);
#endif
