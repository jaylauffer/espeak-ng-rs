/* Native adapter frozen from 81ab4ecf numbers.c, unchanged bodies.
 * SPDX-License-Identifier: GPL-3.0-or-later */
static int LetterLookup(void *context, char *source, size_t start, unsigned secondary, char *output)
{
    Translator *tr = secondary ? translator3 : context;
    if (tr == NULL) { output[0] = 0; return 0; }
    return LookupBounded(tr, source+start, output, 200);
}
static int LetterNamed(void *context, const char *key, char *output)
{
    return LookupBounded(context, key, output, 200);
}
static int LetterValue(void *context, unsigned field)
{
    Translator *tr = context;
    return field == 0 ? tr->translator_name : tr->langopts.accents;
}
static int LetterSpace(void *context, unsigned code)
{
    (void)context;
    return iswspace(code) != 0;
}
static void LetterRules(void *context, char *source, size_t start, size_t capacity, unsigned flags, char *output)
{
    Translator *tr = context;
    const char *saved_base = tr->rule_text_base;
    size_t saved_length = tr->rule_text_length;
    tr->rule_text_base = source;
    tr->rule_text_length = 10;
    TranslateRules(tr, source+start, output, (int)capacity, NULL, flags, NULL);
    tr->rule_text_base = saved_base;
    tr->rule_text_length = saved_length;
}
static void LetterSelect(void *context, unsigned restore)
{
    (void)context;
    if (restore) SelectPhonemeTable(voice->phoneme_tab_ix);
    else SetTranslator3(ESPEAKNG_DEFAULT_VOICE);
}
static void LetterStress(void *context, char *output, unsigned *flags, int control)
{
    SetWordStress(context, output, flags, -1, control);
}
static RustLetterLookup LetterTable(Translator *tr)
{
    const RustLetterLookup table = { tr, LetterLookup, LetterNamed, LetterValue, LetterSpace, LetterRules, LetterSelect, LetterStress };
    return table;
}
void LookupAccentedLetterBounded(Translator *tr, unsigned letter, char *output, size_t capacity)
{
    const RustLetterLookup table = LetterTable(tr);
    (void)espeak_rs_lookup_letter(&table, letter, 0, 0, 1, output, capacity);
}
void LookupLetterBounded(Translator *tr, unsigned letter, int next, char *output, int control, size_t capacity)
{
    const RustLetterLookup table = LetterTable(tr);
    (void)espeak_rs_lookup_letter(&table, letter, next, control, 0, output, capacity);
}
