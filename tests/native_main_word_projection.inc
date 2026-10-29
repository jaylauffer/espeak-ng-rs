/* Frozen test fixture: the C main word projection translateword.c had before
 * its callbacks moved to Rust (rust/translateword_engine_compat.rs), copied as
 * the test generator extracted it. SPDX-License-Identifier: GPL-3.0-or-later */
/* Begin native main word projection. */
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
/* End native word source projections. */
static int WordValue(void *opaque, unsigned field, unsigned index)
{
    RustWordHost *host = opaque;
    Translator *tr = host->tr;
    switch (field) {
    case 0: return tr->data_dictlist != NULL;
    case 1: return index == 0 || (host->remaining > 0 && index < (unsigned)host->remaining) ? host->rows[index].flags : 0;
    case 2: return host->remaining;
    case 3: return option_sayas;
    case 4: return dictionary_skipwords;
    case 5: return tr->langopts.numbers;
    case 6: return tr->langopts.numbers2;
    case 7: return option_tone_flags;
    case 8: return tr->clause_lower_count;
    case 9: return tr->clause_upper_count;
    case 10: return tr->langopts.param[LOPT_PREFIXES];
    case 11: return (option_phonemes & espeakPHONEMES_TRACE) != 0;
    case 12: return tr->langopts.stress_flags;
    case 13: return host->any_stressed != NULL; /* legacy tests pointer presence */
    case 14: return tr->translator_name;
    case 15: return tr->langopts.param[LOPT_ALT];
    case 16: return tr->expect_verb;
    case 17: return tr->expect_verb_s;
    case 18: return tr->expect_noun;
    case 19: return tr->expect_past;
    default: return 0;
    }
}
static void WordStore(void *opaque, unsigned field, int value)
{
    RustWordHost *host = opaque;
    switch (field) {
    case 0: dictionary_skipwords = value; break;
    case 1: host->tr->expect_verb = value; break;
    case 2: host->tr->expect_verb_s = value; break;
    case 3: host->tr->expect_noun = value; break;
    case 4: host->tr->expect_past = value; break;
    }
}
static int WordLocale(void *opaque, unsigned code, unsigned digit)
{
    (void)opaque;
    return digit ? iswdigit(code) != 0 : iswalpha(code) != 0;
}
static int WordList(void *opaque, RustWordSource *source, char *phonemes, unsigned *flags, int ending, int *found)
{
    RustWordHost *host = opaque;
    char *p = WordPointer(host, *source);
    if (p == NULL) return -1;
    *found = LookupDictListBounded(host->tr, &p, phonemes, flags, ending, host->rows, host->remaining, N_WORD_PHONEMES);
    return WordSource(host, p, source);
}
static int WordEmoji(void *opaque, RustWordSource *source, unsigned *flags)
{
    RustWordHost *host = opaque;
    char *p = WordPointer(host, *source);
    if (p == NULL) return -1;
    if (LookupEmojiBaseSequence(host->tr, &p, flags, host->rows, host->remaining))
        host->spans[1] = (RustWordSpan){p-2, p, N_WORD_BYTES};
    return WordSource(host, p, source);
}
static int WordText(void *opaque, RustWordSource source)
{
    RustWordHost *host = opaque;
    if (host->text_output == NULL) return 0;
    size_t count = 0;
    for (;;) {
        RustWordSource at = source;
        if (count > N_WORD_BYTES || count > (size_t)INTPTR_MAX || source.offset > INTPTR_MAX-(intptr_t)count) return -1;
        at.offset += (intptr_t)count;
        char *p = WordPointer(host, at);
        if (p == NULL) return -1;
        if (*p == 0) break;
        count++;
    }
    char *p = WordPointer(host, source);
    memcpy(host->text_output, p, count+1);
    return 0;
}
static int WordDotted(void *opaque, RustWordSource source, int *result)
{
    char *p = WordPointer(opaque, source);
    if (p == NULL) return -1;
    *result = CheckDottedAbbrev(p);
    return 0;
}
static int WordNumberLanguage(void *opaque)
{
    RustWordHost *host = opaque;
    LookupBounded(host->tr, "_0lang", host->output, N_WORD_PHONEMES);
    return 0;
}
static int WordNumber(void *opaque, unsigned roman, RustWordSource source, char *phonemes, unsigned *flags, int *found)
{
    RustWordHost *host = opaque;
    char *p = WordPointer(host, source);
    if (p == NULL) return -1;
    *found = roman ? TranslateRoman(host->tr, p, phonemes, phonemes+N_WORD_PHONEMES, host->rows, host->remaining) :
        TranslateNumber(host->tr, p, phonemes, phonemes+N_WORD_PHONEMES, flags, host->rows, host->remaining, 0);
    return 0;
}
static int WordSpell(void *opaque, RustWordSource *source, char *phonemes, int mode, int *switched)
{
    RustWordHost *host = opaque;
    char *p = WordPointer(host, *source);
    if (p == NULL) return -1;
    p = SpeakIndividualLetters(host->tr, p, phonemes, mode, host->alphabet, host->output);
    *switched = p == NULL;
    return p == NULL ? 0 : WordSource(host, p, source);
}
static int WordLetter(void *opaque, RustWordSource source, char *phonemes, unsigned non_initial, size_t *consumed)
{
    RustWordHost *host = opaque;
    char *p = WordPointer(host, source);
    if (p == NULL) return -1;
    int count = TranslateLetter(host->tr, p, phonemes, non_initial, host->alphabet);
    if (count < 0) return -1;
    *consumed = (size_t)count;
    return 0;
}
static int WordUnpronounceable(void *opaque, RustWordSource source, int position, int *result)
{
    RustWordHost *host = opaque;
    char *p = WordPointer(host, source);
    if (p == NULL) return -1;
    *result = Unpronouncable(host->tr, p, position);
    return 0;
}
static int WordSpellingStress(void *opaque, char *phonemes, int position)
{
    RustWordHost *host = opaque;
    SetSpellingStress(host->tr, phonemes, 0, position);
    return 0;
}
static int WordRules(void *opaque, RustWordSource source, char *phonemes, char *ending, unsigned word_flags, unsigned *flags, int *result)
{
    RustWordHost *host = opaque;
    char *p = WordPointer(host, source);
    if (p == NULL) return -1;
    *result = TranslateRules(host->tr, p, phonemes, N_WORD_PHONEMES, ending, word_flags, flags);
    return 0;
}
static int WordRemove(void *opaque, RustWordSource source, int ending, char *copy, int *result)
{
    RustWordHost *host = opaque;
    char *p = WordPointer(host, source);
    if (p == NULL) return -1;
    *result = RemoveEnding(host->tr, p, ending, copy);
    return 0;
}
static int WordPrefix(void *opaque, const char *prefix, RustWordSource *result)
{
    RustWordHost *host = opaque;
    memcpy(host->prefix, prefix, sizeof(host->prefix));
    host->spans[2] = (RustWordSpan){host->prefix, host->prefix+1, sizeof(host->prefix)};
    *result = (RustWordSource){2, 0};
    return 0;
}
static void WordTraceSuffix(void *opaque, const char *phonemes)
{
    (void)opaque;
    char decoded[N_WORD_PHONEMES];
    DecodePhonemes(phonemes, decoded);
    fprintf(f_trans, "  suffix [%s]\n\n", decoded);
}
static int WordAppend(void *opaque, char *phonemes, const char *ending)
{
    RustWordHost *host = opaque;
    AppendPhonemes(host->tr, phonemes, N_WORD_PHONEMES, ending);
    return 0;
}
static int WordPlural(void *opaque, unsigned flags, unsigned last)
{
    RustWordHost *host = opaque;
    addPluralSuffixes(flags, host->tr, (char)last, host->output);
    return 0;
}
static int WordStress(void *opaque, char *phonemes, unsigned *flags, int position, int control)
{
    RustWordHost *host = opaque;
    SetWordStress(host->tr, phonemes != NULL ? phonemes : host->output, flags, position, control);
    return 0;
}
static int WordSnapshot(void *opaque, char *phonemes)
{
    RustWordHost *host = opaque;
    char *end = memchr(host->output, 0, N_WORD_PHONEMES);
    if (end == NULL) return -1;
    memcpy(phonemes, host->output, (size_t)(end-host->output)+1);
    return 0;
}
static int WordPublish(void *opaque, const char *phonemes, unsigned joined)
{
    RustWordHost *host = opaque;
    const char *end = memchr(phonemes, 0, N_WORD_PHONEMES);
    if (end == NULL) return -1;
    memcpy(host->output, phonemes, (size_t)(end-phonemes)+1);
    if (joined) host->output[N_WORD_PHONEMES-1] = 0;
    return 0;
}
static int WordChangeStress(void *opaque, int level)
{
    RustWordHost *host = opaque;
    ChangeWordStress(host->tr, host->output, level);
    return 0;
}
static int WordSpecial(void *opaque, unsigned flags)
{
    RustWordHost *host = opaque;
    ApplySpecialAttribute2(host->tr, host->output, flags);
    return 0;
}
int TranslateWord3(Translator *tr, char *word_start, WORD_TAB *wtab, int wtab_remaining, char *word_out, bool *any_stressed_words, ALPHABET *current_alphabet, char word_phonemes[], size_t size_word_phonemes)
{
    if (tr == NULL || word_phonemes == NULL || size_word_phonemes < N_WORD_PHONEMES) return 0;
    WORD_TAB empty[8] = {{0}};
    RustWordHost host = {.tr = tr, .rows = wtab != NULL ? wtab : empty,
        .remaining = wtab != NULL ? wtab_remaining : 0, .any_stressed = any_stressed_words,
        .alphabet = current_alphabet, .output = word_phonemes, .text_output = word_out};
    if (tr->data_dictlist != NULL) {
        if (WordWindow(&host, word_start) != 0) return 0;
    }
    RustTranslateWord callbacks = {&host, WordByte, WordWrite, WordValue, WordStore, WordLocale,
        WordList, WordEmoji, WordText, WordDotted, WordNumberLanguage, WordNumber, WordSpell,
        WordLetter, WordUnpronounceable, WordSpellingStress, WordRules, WordRemove, WordPrefix,
        WordTraceSuffix, WordAppend, WordPlural, WordStress, WordSnapshot, WordPublish, WordChangeStress, WordSpecial};
    unsigned result = 0;
    return espeak_rs_translate_word(&callbacks, &result) == 0 ? (int)result : 0;
}
