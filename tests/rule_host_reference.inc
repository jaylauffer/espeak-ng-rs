/* Previous production rule host, verbatim from 244ef3b6 dictionary.c.
 * Test-only oracle; SPDX-License-Identifier: GPL-3.0-or-later */
typedef struct {
    Translator *tr;
    char *source, *output, *ending, *base;
    size_t length, match_length, capacity;
    unsigned *flags;
} RustRulesHost;
static char *RulesPointer(RustRulesHost *host, intptr_t position)
{
    uintptr_t address = (uintptr_t)host->source;
    if (position < 0) {
        uintptr_t before = (uintptr_t)(-(position+1))+1;
        if (before > address) return NULL;
        address -= before;
    } else {
        if ((uintptr_t)position > UINTPTR_MAX-address) return NULL;
        address += (uintptr_t)position;
    }
    uintptr_t base = (uintptr_t)host->base;
    return address >= base && address-base < host->length ? (char *)address : NULL;
}
static int RulesByte(void *opaque, intptr_t position)
{
    char *pointer = RulesPointer(opaque, position);
    return pointer != NULL ? (unsigned char)*pointer : -1;
}
static int RulesWrite(void *opaque, intptr_t position, unsigned char byte)
{
    char *pointer = RulesPointer(opaque, position);
    if (pointer == NULL) return -1;
    *pointer = (char)byte; return 0;
}
static int RulesValue(void *opaque, unsigned field, unsigned index)
{
    RustRulesHost *host = opaque; Translator *tr = host->tr;
    switch (field) {
    case 0: return tr->data_dictrules != NULL;
    case 1: return tr->letter_bits_offset;
    case 2: return tr->langopts.tone_numbers;
    case 3: return option_sayas;
    case 4: return tr->langopts.param[LOPT_DIERESES];
    case 5: return tr->langopts.alt_alphabet;
    case 6: return tr->langopts.alt_alphabet_lang;
    case 7: return (option_phonemes & espeakPHONEMES_TRACE) != 0;
    case 8: return tr->langopts.param[LOPT_BRACKET_PAUSE_ANNOUNCED];
    case 9: return tr->langopts.param[LOPT_BRACKET_PAUSE];
    case 10: return pre_pause;
    case 11: return index < 256 ? tr->groups2_count[index] : -1;
    case 12: return index < 256 ? tr->groups2_start[index] : -1;
    case 13: return index < N_RULE_GROUP2 ? (int)tr->groups2_name[index] : -1;
    case 14: return host->flags != NULL ? (int)host->flags[0] : 0;
    case 15: return CHAR_MIN < 0;
    default: return 0;
    }
}
static void RulesStore(void *opaque, unsigned field, int value)
{
    RustRulesHost *host = opaque;
    switch (field) {
    case 0: host->tr->word_vowel_count = value; break;
    case 1: host->tr->word_stressed_count = value; break;
    case 2: host->tr->phonemes_repeat_count = value; break;
    case 3: pre_pause = value; break;
    case 4: if (host->flags != NULL) host->flags[0] = (unsigned)value; break;
    }
}
static int RulesLocale(void *opaque, unsigned code, unsigned digit)
{
    (void)opaque; return digit != 0 ? iswdigit(code) != 0 : iswalpha(code) != 0;
}
static size_t RulesGroup(void *opaque, unsigned kind, unsigned index)
{
    Translator *tr = ((RustRulesHost *)opaque)->tr; char *rule = NULL;
    if (kind == 0 && index < 128) rule = tr->groups3[index];
    else if (kind == 1 && index < N_RULE_GROUP2) rule = tr->groups2[index];
    else if (kind == 2 && index < 256) rule = tr->groups1[index];
    else return SIZE_MAX-1;
    if (rule == NULL) return SIZE_MAX;
    uintptr_t address = (uintptr_t)rule, base = (uintptr_t)tr->data_dictlist;
    return address >= base && address-base < tr->data_dict_size ? address-base : SIZE_MAX-1;
}
static int RulesMatched(void *opaque, size_t group, size_t width, unsigned flags, unsigned dictionary, RustRulesMatch *output)
{
    RustRulesHost *host = opaque; Translator *tr = host->tr;
    if (width > 4 || output->cursor > INTPTR_MAX) return -1;
    char *cursor = RulesPointer(host, (intptr_t)output->cursor);
    char *deleted = output->delete_offset == RUST_RULES_NO_DELETE ? NULL : RulesPointer(host, output->delete_offset);
    if (cursor == NULL || (output->delete_offset != RUST_RULES_NO_DELETE && deleted == NULL)) return -1;
    if (group != SIZE_MAX && group >= tr->data_dict_size) return -1;
    MatchRecord result = {output->points, output->phonemes, output->ending, deleted};
    espeak_rs_match_rule(tr, &cursor, host->source, (int)width,
        group == SIZE_MAX ? NULL : tr->data_dictlist+group, &result, flags, dictionary, host->match_length);
    uintptr_t address = (uintptr_t)cursor, start = (uintptr_t)host->source;
    if (address < start || address-start > INTPTR_MAX || RulesPointer(host, (intptr_t)(address-start)) == NULL) return -1;
    output->cursor = address-start; output->points = result.points; output->ending = result.end_type;
    output->delete_offset = RUST_RULES_NO_DELETE;
    if (result.del_fwd != NULL) {
        address = (uintptr_t)result.del_fwd;
        uintptr_t lower = (uintptr_t)host->base, distance = address >= start ? address-start : start-address;
        if (address < lower || address-lower >= host->length || distance > INTPTR_MAX) return -1;
        intptr_t offset = address >= start ? (intptr_t)(address-start) : -(intptr_t)(start-address);
        if (RulesPointer(host, offset) != result.del_fwd) return -1;
        output->delete_offset = offset;
    }
    if (result.phonemes == NULL || result.phonemes[0] == 0) { output->phonemes[0] = 0; return 0; }
    if (result.phonemes == output->phonemes) return 0;
    address = (uintptr_t)result.phonemes; uintptr_t base = (uintptr_t)tr->data_dictlist;
    if (address < base || address-base >= tr->data_dict_size) return -1;
    size_t available = tr->data_dict_size-(address-base);
    if (available > 200) available = 200;
    const char *end = memchr(result.phonemes, 0, available);
    if (end == NULL) return -1;
    memcpy(output->phonemes, result.phonemes, (size_t)(end-result.phonemes)+1); return 0;
}
static int RulesSymbol(void *opaque, const char *key, char *output)
{
    LookupBounded(((RustRulesHost *)opaque)->tr, key, output, 40); return 0;
}
static int RulesLetter(void *opaque, unsigned code, char *output)
{
    LookupLetterBounded(((RustRulesHost *)opaque)->tr, code, -1, output, 0, 160); return 0;
}
static int RulesPublish(void *opaque, unsigned kind, const char *text)
{
    RustRulesHost *host = opaque;
    char *destination = kind == 0 ? host->output : host->ending;
    size_t capacity = kind == 0 ? host->capacity : N_WORD_PHONEMES;
    const char *end = memchr(text, 0, 200);
    if (destination == NULL || end == NULL || (size_t)(end-text) >= capacity) return -1;
    memcpy(destination, text, (size_t)(end-text)+1); return 0;
}
static int RulesHasEnding(void *opaque) { return ((RustRulesHost *)opaque)->ending != NULL; }
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
    if (tr == NULL || source == NULL || phonemes == NULL || capacity <= 0 || capacity > 200 || tr->data_dictrules == NULL) return 0;
    RustRulesHost host = {tr, source, phonemes, ending, source-1, strlen(source)+2, 0, (size_t)capacity, dictionary};
    host.match_length = host.length;
    Translator *owners[2] = {tr, translator};
    for (unsigned index = 0; index < 2; index++) {
        Translator *owner = owners[index];
        if (owner == NULL || owner->rule_text_base == NULL) continue;
        uintptr_t address = (uintptr_t)source, base = (uintptr_t)owner->rule_text_base;
        if (address > base && address-base < owner->rule_text_length && host.length-1 <= owner->rule_text_length-(address-base)) {
            host.base = (char *)owner->rule_text_base; host.length = owner->rule_text_length; break;
        }
    }
    const RustTranslateRules callbacks = {&host, RulesByte, RulesWrite, RulesValue, RulesStore, RulesLocale,
        RulesGroup, RulesMatched, RulesSymbol, RulesLetter, RulesPublish, RulesHasEnding, RulesAppend, RulesTrace};
    int result;
    return espeak_rs_translate_rules(&callbacks, (unsigned)flags, &result) == 0 ? result : 0;
}
