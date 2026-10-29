/* Frozen test fixture: the C word admission projection translateword.c had before
 * its callbacks moved to Rust (rust/translateword_engine_compat.rs), copied as
 * the test generator extracted it. SPDX-License-Identifier: GPL-3.0-or-later */
/* Begin native word admission projection. */
static int AdmissionValue(void *opaque, unsigned field)
{
    RustWordHost *host = opaque;
    if (host->tr == NULL) return 0;
    switch (field) {
    case 0: return host->tr->letter_bits_offset;
    case 1: return host->tr->langopts.param[LOPT_UNPRONOUNCABLE];
    case 2: return host->tr->langopts.max_initial_consonants;
    default: return 0;
    }
}
static int AdmissionAlpha(void *opaque, unsigned code)
{
    (void)opaque;
    return iswalpha(code) != 0;
}
static int AdmissionVowel(void *opaque, unsigned code)
{
    RustWordHost *host = opaque;
    return host->tr != NULL ? IsVowel(host->tr, code) : 0;
}
static int AdmissionProbe(void *opaque, char *scratch, int *result)
{
    RustWordHost *host = opaque;
    char *source = WordPointer(host, (RustWordSource){0, 0});
    if (host->tr == NULL || source == NULL) return -1;
    *result = TranslateRules(host->tr, source, scratch, N_WORD_PHONEMES, NULL, FLAG_UNPRON_TEST, NULL);
    return 0;
}
static void AdmissionSkip(void *opaque, int value)
{
    (void)opaque;
    dictionary_skipwords = value;
}
static RustWordAdmission AdmissionCallbacks(RustWordHost *host)
{
    return (RustWordAdmission){host, WordByte, WordWrite, AdmissionValue, AdmissionAlpha,
        AdmissionVowel, AdmissionProbe, AdmissionSkip};
}
static int CheckDottedAbbrev(char *word)
{
    RustWordHost host = {0};
    if (WordWindow(&host, word) != 0) return 0;
    RustWordAdmission table = AdmissionCallbacks(&host);
    int result = 0;
    return espeak_rs_word_dotted(&table, &result) == 0 ? result : 0;
}
