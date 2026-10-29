/* Frozen test fixture: the C word helper projections translateword.c had before
 * its callbacks moved to Rust (rust/translateword_engine_compat.rs), copied as
 * the test generator extracted it. SPDX-License-Identifier: GPL-3.0-or-later */
/* Begin native word helper projections. */
typedef struct {
    RustWordHost source;
    char *phonemes;
    const ALPHABET *alphabet;
} RustWordHelpersHost;
static int HelpersByte(void *opaque, RustWordSource source)
{
    RustWordHelpersHost *host = opaque;
    return WordByte(&host->source, source);
}
static int HelpersLetter(void *opaque, RustWordSource source, unsigned control, size_t *count)
{
    RustWordHelpersHost *host = opaque;
    char *word = WordPointer(&host->source, source);
    if (word == NULL) return -1;
    int consumed = TranslateLetter(host->source.tr, word, host->phonemes, (int)control, host->alphabet);
    if (consumed < 0) return -1;
    *count = (size_t)consumed;
    return 0;
}
static unsigned char HelpersLeading(void *opaque)
{
    RustWordHelpersHost *host = opaque;
    return (unsigned char)host->phonemes[0];
}
static int HelpersCopySwitch(void *opaque)
{
    RustWordHelpersHost *host = opaque;
    const char *end = memchr(host->phonemes, 0, N_WORD_PHONEMES);
    if (end == NULL) return -1;
    memmove(host->source.output, host->phonemes, (size_t)(end-host->phonemes)+1);
    return 0;
}
static int HelpersStress(void *opaque, int mode, int count)
{
    RustWordHelpersHost *host = opaque;
    SetSpellingStress(host->source.tr, host->phonemes, mode, count);
    return 0;
}
static int HelpersPlural(void *opaque, char *source)
{
    RustWordHelpersHost *host = opaque;
    Translator *owner = host->source.tr;
    const char *saved_base = owner->rule_text_base;
    size_t saved_length = owner->rule_text_length;
    owner->rule_text_base = source; owner->rule_text_length = 5;
    TranslateRules(owner, source+2, host->source.output, N_WORD_PHONEMES, NULL, 0, NULL);
    owner->rule_text_base = saved_base; owner->rule_text_length = saved_length;
    return 0;
}
static RustWordHelpers HelpersCallbacks(RustWordHelpersHost *host)
{
    return (RustWordHelpers){host, HelpersByte, HelpersLetter, HelpersLeading,
        HelpersCopySwitch, HelpersStress, HelpersPlural};
}
static char *SpeakIndividualLetters(Translator *tr, char *word, char *phonemes, int spell_word, const ALPHABET *current_alphabet, char word_phonemes[])
{
    if (tr == NULL || phonemes == NULL || word_phonemes == NULL) return NULL;
    RustWordHelpersHost host = {.source = {.tr = tr, .output = word_phonemes},
        .phonemes = phonemes, .alphabet = current_alphabet};
    if (WordWindow(&host.source, word) != 0) return NULL;
    RustWordHelpers callbacks = HelpersCallbacks(&host);
    RustWordSource result = {0}; int switched = 0;
    if (espeak_rs_word_spell(&callbacks, spell_word, &result, &switched) != 0 || switched) return NULL;
    return WordPointer(&host.source, result);
}
