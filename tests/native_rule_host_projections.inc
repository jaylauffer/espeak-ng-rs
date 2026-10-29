/* Frozen test fixture: the C rule host projections dictionary.c had before its
 * callbacks moved to Rust (rust/translate_rules_host_compat.rs), copied as the test generator
 * extracted it. SPDX-License-Identifier: GPL-3.0-or-later */
/* Begin native rule host projections. */
static int RulesLocale(void *opaque, unsigned code, unsigned digit)
{
    (void)opaque; return digit != 0 ? iswdigit(code) != 0 : iswalpha(code) != 0;
}

static int RulesMatched(void *opaque, size_t group, size_t width, unsigned flags, unsigned dictionary, char **cursor, void *record)
{
    RustRulesHost *host = opaque; Translator *tr = host->tr;
    espeak_rs_match_rule(tr, cursor, host->source, (int)width,
        group == SIZE_MAX ? NULL : tr->data_dictlist+group, record, flags, dictionary, host->match_length);
    return 0;
}
static int RulesSymbol(void *opaque, const char *key, char *output)
{
    LookupBounded(((RustRulesHost *)opaque)->tr, key, output, 40); return 0;
}
static int RulesLetter(void *opaque, unsigned code, char *output)
{
    LookupLetterBounded(((RustRulesHost *)opaque)->tr, code, -1, output, 0, 160); return 0;
}


static int RulesAppend(void *opaque, const char *addition)
{
    RustRulesHost *host = opaque;
    if (memchr(host->output, 0, host->capacity) == NULL || memchr(addition, 0, 200) == NULL) return -1;
    AppendPhonemes(host->tr, host->output, (int)host->capacity, addition); return 0;
}
static void RulesTrace(void *opaque, unsigned kind, const char *word)
{
    (void)opaque;
    if (kind == 0) fprintf(f_trans, "Unpronouncable? '%s'\n", word);
    else if (kind == 1) fprintf(f_trans, "Translate '%s'\n", word);
    else fprintf(f_trans, "\n");
}
int TranslateRules(Translator *tr, char *source, char *phonemes, int capacity, char *ending, int flags, unsigned *dictionary)
{
    const RustRulesHost host = {tr, source, phonemes, ending, NULL, 0, 0, capacity > 0 ? (size_t)capacity : 0, dictionary};
    const RustRulesGlobals globals = {&option_sayas, &option_phonemes, &pre_pause, CHAR_MIN < 0};
    const RustRulesEffects effects = {RulesLocale, RulesMatched, RulesSymbol, RulesLetter, RulesAppend, RulesTrace};
    int result;
    return espeak_rs_translate_rules_host(&host, translator, &globals, &effects, (unsigned)flags, &result) == 0 ? result : 0;
}
