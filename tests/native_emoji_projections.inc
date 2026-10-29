/* Frozen test fixture: the C emoji projections translateword.c had before
 * its callbacks moved to Rust (rust/translateword_engine_compat.rs), copied as
 * the test generator extracted it. SPDX-License-Identifier: GPL-3.0-or-later */
/* Begin native emoji projections. */
typedef struct { RustWordHost source; char **wordptr; } RustEmojiHost;
static int EmojiByte(void *opaque, size_t offset)
{
    RustEmojiHost *host = opaque;
    if (offset > (size_t)INTPTR_MAX) return -1;
    return WordByte(&host->source, (RustWordSource){0, (intptr_t)offset});
}
static int EmojiList(void *opaque, char *input, char *phonemes, unsigned *flags, char *text)
{
    RustEmojiHost *host = opaque;
    char *word = input;
    LookupDictListBounded(host->source.tr, &word, phonemes, flags, FLAG_ALLOW_TEXTMODE,
        host->source.rows, host->source.remaining, N_WORD_PHONEMES);
    if (!(flags[0] & FLAG_TEXTMODE)) return 0;
    uintptr_t address = (uintptr_t)word, base = (uintptr_t)input;
    size_t available;
    if (address >= base && address-base < N_WORD_BYTES+2) available = N_WORD_BYTES+2-(address-base);
    else {
        base = (uintptr_t)host->source.tr->rust_list_replacement;
        if (address < base || address-base >= N_WORD_BYTES) return -1;
        available = N_WORD_BYTES-(address-base);
    }
    const char *end = memchr(word, 0, available);
    if (end == NULL) return -1;
    memcpy(text, word, (size_t)(end-word)+1);
    return 0;
}
static int EmojiWrite(void *opaque, size_t offset, const char *bytes, size_t length)
{
    RustEmojiHost *host = opaque;
    if (offset > N_WORD_BYTES || length > N_WORD_BYTES-offset) return -1;
    memcpy(host->source.tr->rust_emoji_replacement+offset, bytes, length);
    return 0;
}
static int EmojiTracing(void *opaque)
{
    (void)opaque;
    return (option_phonemes & espeakPHONEMES_TRACE) != 0;
}
static int EmojiSnapshot(void *opaque, char *output)
{
    RustEmojiHost *host = opaque;
    memcpy(output, host->source.tr->rust_emoji_replacement, N_WORD_BYTES);
    return 0;
}
static void EmojiTrace(void *opaque, const char *source, const char *replacement)
{
    (void)opaque;
    fprintf(f_trans, "Replace: %s  %s\n", source, replacement+2);
}
static int EmojiFinish(void *opaque)
{
    RustEmojiHost *host = opaque;
    *host->wordptr = host->source.tr->rust_emoji_replacement+2;
    return 0;
}
static bool LookupEmojiBaseSequence(Translator *tr, char **wordptr, unsigned int *flags, WORD_TAB *wtab, int wtab_remaining)
{
    if (tr == NULL || wordptr == NULL || *wordptr == NULL || flags == NULL) return false;
    RustEmojiHost host = {.source = {.tr = tr, .rows = wtab, .remaining = wtab_remaining}, .wordptr = wordptr};
    if (WordWindow(&host.source, *wordptr) != 0) return false;
    RustEmojiFallback callbacks = {&host, EmojiByte, EmojiList, EmojiWrite, EmojiTracing,
        EmojiSnapshot, EmojiTrace, EmojiFinish};
    return espeak_rs_lookup_emoji(&callbacks, flags) == 1;
}
