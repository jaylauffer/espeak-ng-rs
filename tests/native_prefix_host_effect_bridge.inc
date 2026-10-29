/* Frozen test fixture: the C prefix host effect bridge dictionary.c had before its
 * callbacks moved to Rust (rust/list_host_compat.rs), copied as the test generator
 * extracted it. SPDX-License-Identifier: GPL-3.0-or-later */
/* Begin native prefix host effect bridge. */
static int LookupFlags(Translator *tr, const char *word, unsigned int flags_out[2])
{
    char *cursor = (char *)word;
    RustListHost host = {tr, word, 0, &cursor, 0, NULL, 0};
    RustListGlobals globals = ListGlobals();
    return espeak_rs_lookup_flags_host(&host, &globals, &ListEffects, flags_out);
}
