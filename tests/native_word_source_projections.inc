/* Frozen test fixture: the C word source projections translateword.c had before
 * its callbacks moved to Rust (rust/translateword_engine_compat.rs), copied as
 * the test generator extracted it. SPDX-License-Identifier: GPL-3.0-or-later */
/* Begin native word source projections. */
typedef struct {
    char *base, *origin;
    size_t length;
} RustWordSpan;
typedef struct {
    Translator *tr;
    WORD_TAB *rows;
    int remaining;
    bool *any_stressed;
    ALPHABET *alphabet;
    char *output, *text_output;
    RustWordSpan spans[3];
    char prefix[65];
} RustWordHost;

/* Numeric identities preserve live owner storage without retaining a Rust
 * pointer loan across nested translator calls. All offset arithmetic precedes
 * dereference and is checked against the admitted initialized source window. */
static char *WordPointer(RustWordHost *host, RustWordSource source)
{
    if (source.slot >= 3) return NULL;
    RustWordSpan *span = &host->spans[source.slot];
    if (span->base == NULL) return NULL;
    uintptr_t address = (uintptr_t)span->origin;
    if (source.offset < 0) {
        uintptr_t before = (uintptr_t)(-(source.offset+1))+1;
        if (before > address) return NULL;
        address -= before;
    } else {
        if ((uintptr_t)source.offset > UINTPTR_MAX-address) return NULL;
        address += (uintptr_t)source.offset;
    }
    uintptr_t base = (uintptr_t)span->base;
    return address >= base && address-base < span->length ? (char *)address : NULL;
}
static int WordSource(RustWordHost *host, char *pointer, RustWordSource *source)
{
    uintptr_t address = (uintptr_t)pointer;
    for (unsigned slot = 0; slot < 3; slot++) {
        RustWordSpan *span = &host->spans[slot];
        uintptr_t base = (uintptr_t)span->base, origin = (uintptr_t)span->origin;
        if (span->base == NULL || address < base || address-base >= span->length) continue;
        uintptr_t delta = address >= origin ? address-origin : origin-address;
        if (delta > INTPTR_MAX) return -1;
        *source = (RustWordSource){slot, address >= origin ? (intptr_t)delta : -(intptr_t)delta};
        return 0;
    }
    char *replacement = host->tr->rust_list_replacement;
    uintptr_t base = (uintptr_t)replacement;
    if (address >= base && address-base < N_WORD_BYTES) {
        host->spans[1] = (RustWordSpan){replacement, replacement+2, N_WORD_BYTES};
        return WordSource(host, pointer, source);
    }
    return -1;
}
static int WordByte(void *opaque, RustWordSource source)
{
    char *p = WordPointer(opaque, source);
    return p != NULL ? (unsigned char)*p : -1;
}
static int WordWrite(void *opaque, RustWordSource source, unsigned char byte)
{
    char *p = WordPointer(opaque, source);
    if (p == NULL) return -1;
    *p = (char)byte;
    return 0;
}
static int WordWindow(RustWordHost *host, char *word)
{
    if (word == NULL) return -1;
    const Translator *owners[4] = {host->tr, translator, translator2, translator3};
    for (unsigned i = 0; i < 4; i++) {
        const Translator *owner = owners[i];
        if (owner == NULL || owner->rule_text_base == NULL) continue;
        uintptr_t address = (uintptr_t)word, base = (uintptr_t)owner->rule_text_base;
        if (address >= base && address-base < owner->rule_text_length) {
            host->spans[0] = (RustWordSpan){(char *)owner->rule_text_base, word, owner->rule_text_length};
            return 0;
        }
    }
    /* Internal callers retain an initialized predecessor and NUL lookahead. */
    host->spans[0] = (RustWordSpan){word-1, word, strlen(word)+3};
    return 0;
}
