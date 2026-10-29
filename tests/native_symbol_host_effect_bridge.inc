/* Frozen test fixture: the C symbol host effect bridge dictionary.c had before its
 * callbacks moved to Rust (rust/list_host_compat.rs), copied as the test generator
 * extracted it. SPDX-License-Identifier: GPL-3.0-or-later */
/* Begin native symbol host effect bridge. */
int LookupBounded(Translator *tr, const char *word, char *ph_out, size_t capacity)
{
    if (tr == NULL || word == NULL || ph_out == NULL || capacity == 0) return 0;
    char *cursor = (char *)word;
    RustListHost host = {tr, word, 0, &cursor, FLAG_ALLOW_TEXTMODE, NULL, 0};
    RustListGlobals globals = ListGlobals();
    return espeak_rs_lookup_symbol_host(&host, &globals, &ListEffects, ph_out, capacity);
}
