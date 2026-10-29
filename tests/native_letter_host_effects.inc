/* Frozen test fixture: the C letter host numbers.c passed before its
 * primitives moved to Rust (rust/letter_host_compat.rs), copied as the test
 * generator extracted it. SPDX-License-Identifier: GPL-3.0-or-later */
/* Begin native letter host effects. */
static int LetterRawLookup(void *tr, const char *word, char *output)
{
    return LookupBounded(tr, word, output, 200);
}
static int LetterSpace(unsigned code)
{
    return iswspace(code) != 0;
}
static void LetterRawRules(void *tr, char *word, size_t capacity, unsigned flags, char *output)
{
    TranslateRules(tr, word, output, (int)capacity, NULL, flags, NULL);
}
static void LetterSetup(void)
{
    SetTranslator3(ESPEAKNG_DEFAULT_VOICE);
}
static void LetterRestore(int table)
{
    SelectPhonemeTable(table);
}
static void LetterStress(void *tr, char *output, unsigned *flags, int control)
{
    SetWordStress(tr, output, flags, -1, control);
}
static const RustLetterEffects LetterEffects = {LetterRawLookup, LetterSpace, LetterRawRules, LetterSetup, LetterRestore, LetterStress};
static RustLetterGlobals LetterGlobals(void)
{
    return (RustLetterGlobals){(void **)&translator3, (void **)&voice};
}
void LookupAccentedLetterBounded(Translator *tr, unsigned letter, char *output, size_t capacity)
{
    RustLetterGlobals globals = LetterGlobals();
    (void)espeak_rs_lookup_letter_host(tr, &globals, &LetterEffects, letter, 0, 0, 1, output, capacity);
}
void LookupLetterBounded(Translator *tr, unsigned letter, int next, char *output, int control, size_t capacity)
{
    RustLetterGlobals globals = LetterGlobals();
    (void)espeak_rs_lookup_letter_host(tr, &globals, &LetterEffects, letter, next, control, 0, output, capacity);
}
