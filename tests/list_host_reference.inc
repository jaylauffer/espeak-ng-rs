/* Native adapters frozen from 6541d604 dictionary.c, unchanged bodies.
 * SPDX-License-Identifier: GPL-3.0-or-later */
typedef struct {
    Translator *tr;
    const char *source;
    size_t length;
    char **wordptr;
    int end_flags;
    WORD_TAB *words;
    int remaining;
} RustListHost;
static int ListByte(void *opaque, size_t position)
{
    RustListHost *host = opaque;
    return position < host->length ? (unsigned char)host->source[position] : -1;
}
static int ListLookup(void *opaque, const char *key, size_t next, unsigned *flags, char *phonemes, size_t *matched)
{
    RustListHost *host = opaque;
    if (next > host->length) return -1;
    const char *found = espeak_rs_lookup_dict(host->tr, key, host->source+next, phonemes,
        flags, host->end_flags, host->words, host->remaining);
    if (found == NULL) return 0;
    uintptr_t address = (uintptr_t)found, base = (uintptr_t)host->source;
    if (address < base || address-base > host->length) return -1;
    *matched = address-base;
    return 1;
}
static int ListRepeat(void *opaque, char *output)
{
    Translator *tr = ((RustListHost *)opaque)->tr;
    memcpy(output, tr->phonemes_repeat, 20);
    return tr->phonemes_repeat_count;
}
static void ListSetRepeat(void *opaque, const char *output, int count)
{
    Translator *tr = ((RustListHost *)opaque)->tr;
    memcpy(tr->phonemes_repeat, output, 20);
    tr->phonemes_repeat_count = count;
}
static int ListTextMode(void *opaque) { return ((RustListHost *)opaque)->tr->langopts.textmode; }
static void ListSkip(void *opaque, int count) { (void)opaque; dictionary_skipwords = count; }
static void ListAccent(void *opaque, unsigned code, size_t capacity, char *phonemes)
{
    LookupAccentedLetterBounded(((RustListHost *)opaque)->tr, code, phonemes, capacity);
}
static void ListReplacement(void *opaque, const char *text)
{
    RustListHost *host = opaque;
    memcpy(host->tr->rust_list_replacement, text, N_WORD_BYTES);
    *host->wordptr = host->tr->rust_list_replacement+2;
}
static void ListTrace(void *opaque, size_t matched)
{
    RustListHost *host = opaque;
    if (!(option_phonemes & espeakPHONEMES_TRACE) || matched >= N_WORD_BYTES || matched > host->length) return;
    char word[N_WORD_BYTES];
    memcpy(word, host->source, matched); word[matched] = 0;
    fprintf(f_trans, "Replace: %s  %s\n", word, *host->wordptr);
}
int LookupDictListBounded(Translator *tr, char **wordptr, char *ph_out, unsigned int *flags, int end_flags, WORD_TAB *wtab, int wtab_remaining, size_t capacity)
{
    if (tr == NULL || wordptr == NULL || *wordptr == NULL || ph_out == NULL || flags == NULL || capacity == 0) return 0;
    RustListHost host = {tr, *wordptr, 0, wordptr, end_flags, wtab, wtab_remaining};
    uintptr_t address = (uintptr_t)host.source, base = (uintptr_t)tr->rule_text_base;
    host.length = tr->rule_text_base != NULL && address >= base && address-base < tr->rule_text_length ?
        tr->rule_text_length-(address-base) : strlen(host.source)+1;
    const RustLookupList callbacks = {&host, ListByte, ListLookup, ListRepeat, ListSetRepeat,
        ListTextMode, ListSkip, ListAccent, ListReplacement, ListTrace};
    int found = espeak_rs_lookup_list(&callbacks, end_flags, flags, ph_out, capacity);
    if (found < 0) { ph_out[0] = 0; return 0; }
    return found;
}
static int SymbolByte(void *opaque, size_t position)
{
    RustListHost *host = opaque;
    const char *word = *host->wordptr;
    if (word == host->source) return position < host->length ? (unsigned char)word[position] : -1;
    return word == host->tr->rust_list_replacement+2 && position < N_WORD_BYTES-2 ? (unsigned char)word[position] : -1;
}
static int SymbolSayAs(void *opaque) { (void)opaque; return option_sayas; }
static void SymbolSetSayAs(void *opaque, int value) { (void)opaque; option_sayas = value; }
static int SymbolTranslate(void *opaque, char *text, char *output)
{
    Translator *tr = ((RustListHost *)opaque)->tr;
    const char *saved_base = tr->rule_text_base;
    size_t saved_length = tr->rule_text_length;
    tr->rule_text_base = text; tr->rule_text_length = 80;
    int flags = TranslateWord(tr, text+3, NULL, NULL);
    const char *end = memchr(word_phonemes, 0, N_WORD_PHONEMES);
    size_t length = end != NULL ? (size_t)(end-word_phonemes) : N_WORD_PHONEMES;
    memcpy(output, word_phonemes, length);
    if (length < N_WORD_PHONEMES) output[length] = 0;
    tr->rule_text_base = saved_base; tr->rule_text_length = saved_length;
    return flags;
}
int LookupBounded(Translator *tr, const char *word, char *ph_out, size_t capacity)
{
    if (tr == NULL || word == NULL || ph_out == NULL || capacity == 0) return 0;
    char *cursor = (char *)word;
    RustListHost host = {tr, word, 0, &cursor, FLAG_ALLOW_TEXTMODE, NULL, 0};
    uintptr_t address = (uintptr_t)word, base = (uintptr_t)tr->rule_text_base;
    host.length = tr->rule_text_base != NULL && address >= base && address-base < tr->rule_text_length ?
        tr->rule_text_length-(address-base) : strlen(word)+1;
    const RustLookupSymbol callbacks = {{&host, ListByte, ListLookup, ListRepeat, ListSetRepeat,
        ListTextMode, ListSkip, ListAccent, ListReplacement, ListTrace},
        SymbolByte, SymbolSayAs, SymbolSetSayAs, SymbolTranslate};
    int result;
    if (espeak_rs_lookup_symbol(&callbacks, ph_out, capacity, &result) != 0) { ph_out[0] = 0; return 0; }
    return result;
}
static int LookupFlags(Translator *tr, const char *word, unsigned int flags_out[2])
{
    char *cursor = (char *)word;
    RustListHost host = {tr, word, strlen(word)+1, &cursor, 0, NULL, 0};
    const RustLookupList callbacks = {&host, ListByte, ListLookup, ListRepeat, ListSetRepeat,
        ListTextMode, ListSkip, ListAccent, ListReplacement, ListTrace};
    if (espeak_rs_lookup_flags(&callbacks, flags_out) != 0) flags_out[0] = flags_out[1] = 0;
    return flags_out[0];
}
