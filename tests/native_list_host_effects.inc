/* Frozen test fixture: the C list host effects dictionary.c had before its
 * callbacks moved to Rust (rust/list_host_compat.rs), copied as the test generator
 * extracted it. SPDX-License-Identifier: GPL-3.0-or-later */
/* Begin native list host effects. */
static const char *ListRawLookup(void *opaque, const char *key, const char *next, char *phonemes, unsigned *flags)
{
    RustListHost *host = opaque;
    return espeak_rs_lookup_dict(host->tr, key, next, phonemes, flags, host->end_flags, host->words, host->remaining);
}
static void ListAccent(void *opaque, unsigned code, size_t capacity, char *phonemes)
{
    LookupAccentedLetterBounded(((RustListHost *)opaque)->tr, code, phonemes, capacity);
}
static int ListRawTranslate(void *opaque, char *word)
{
    return TranslateWord(((RustListHost *)opaque)->tr, word, NULL, NULL);
}
static void ListRawTrace(void *opaque, const char *word, const char *replacement)
{
    (void)opaque;
    fprintf(f_trans, "Replace: %s  %s\n", word, replacement);
}
static const RustListEffects ListEffects = {ListRawLookup, ListAccent, ListRawTranslate, ListRawTrace};
static RustListGlobals ListGlobals(void)
{
    return (RustListGlobals){&dictionary_skipwords, &option_sayas, &option_phonemes, word_phonemes};
}
int LookupDictListBounded(Translator *tr, char **wordptr, char *ph_out, unsigned int *flags, int end_flags, WORD_TAB *wtab, int wtab_remaining, size_t capacity)
{
    if (tr == NULL || wordptr == NULL || *wordptr == NULL || ph_out == NULL || flags == NULL || capacity == 0) return 0;
    RustListHost host = {tr, *wordptr, 0, wordptr, end_flags, wtab, wtab_remaining};
    RustListGlobals globals = ListGlobals();
    return espeak_rs_lookup_list_host(&host, &globals, &ListEffects, flags, ph_out, capacity);
}
