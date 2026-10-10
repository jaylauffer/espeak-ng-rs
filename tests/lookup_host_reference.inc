/* Previous native lookup and transpose hosts, verbatim function bodies
 * from 442aecb4 dictionary.c. Test-only oracle; GPL-3.0-or-later. */
int TransposeAlphabet(Translator *tr, char *text)
{
	size_t length = strlen(text), pairs = 0;
	if (tr->transpose_min <= 0 || tr->transpose_max < tr->transpose_min)
		return (int)length;
	if (tr->frequent_pairs != NULL) {
		while (pairs < 128 && tr->frequent_pairs[pairs] != 0x7fff) pairs++;
		if (pairs == 128) return (int)length;
	}
	int result = espeak_rs_transpose((unsigned char *)text, length + 1,
	    tr->transpose_min, tr->transpose_max, (const unsigned char *)tr->transpose_map,
	    tr->transpose_max - tr->transpose_min + 1, (const int16_t *)tr->frequent_pairs, pairs);
	return result < 0 ? (int)length : result;
}
const char *espeak_rs_lookup_dict(Translator *tr, const char *word, const char *word2,
    char *phonetic, unsigned int *flags, int end_flags, WORD_TAB *wtab, int wtab_remaining)
{
	char word_buf[N_WORD_BYTES + 1], dict_flags_buf[80];
	const char *word1 = word;
	size_t wlen;
	if (tr->transpose_min > 0) {
		strncpy0(word_buf, word, N_WORD_BYTES);
		wlen = TransposeAlphabet(tr, word_buf);
		word = word_buf;
	} else wlen = strlen(word);
	const unsigned char *bucket = (const unsigned char *)tr->dict_hashtab[HashDictionary(word)];
	if (bucket == NULL) { flags[0] = 0; return NULL; }
	if (tr->data_dictrules == NULL || (uintptr_t)bucket >= (uintptr_t)tr->data_dictrules) return NULL;
	RustWordInfo words[20] = {{0}};
	size_t word_count = wtab != NULL && wtab_remaining > 0 ? (size_t)wtab_remaining : 0;
	if (word_count > 20) word_count = 20;
	for (size_t ix = 0; ix < word_count; ix++) {
		words[ix].flags = wtab[ix].flags; words[ix].length = wtab[ix].length;
		if (words[ix].length == 0) break;
	}
	int c, first_bytes = utf8_in(&c, word);
	RustLookupContext context = {
		.conditions = tr->dict_condition, .end_flags = end_flags,
		.word_flags = wtab != NULL ? wtab->flags : 0, .lookup_symbol = flags[1] & FLAG_LOOKUP_SYMBOL,
		.language = tr->translator_name, .previous_flags = tr->prev_dict_flags[0],
		.expect_verb = tr->expect_verb, .expect_verb_s = tr->expect_verb_s,
		.expect_past = tr->expect_past, .expect_noun = tr->expect_noun,
		.native_translator = tr == translator, .sentence = translator != NULL && (translator->clause_terminator & CLAUSE_TYPE_SENTENCE) != 0,
		.single_symbol = word[0] != 0 && word[first_bytes] == 0 && !IsAlpha(c),
		.clause_remaining = translator != NULL && (uintptr_t)translator->clause_end > (uintptr_t)word2 ?
		                    (uintptr_t)translator->clause_end - (uintptr_t)word2 : 0,
	};
	RustLookupOutcome outcome;
	/* One record is at most 255 bytes; never rescan the rest of the clause. */
	size_t next_length = 0;
	if (word2[-1] != 0)
		while (next_length < 255 && word2[next_length] != 0) next_length++;
	if (espeak_rs_lookup_bucket(bucket, (uintptr_t)tr->data_dictrules - (uintptr_t)bucket,
	    word, wlen, word2, next_length, &context, word_count ? words : NULL, word_count, &outcome) != 0) return NULL;
	if (outcome.copied && outcome.phonemes_length >= N_WORD_PHONEMES) return NULL;
	if (outcome.copied) {
		memcpy(phonetic, bucket + outcome.phonemes_offset, outcome.phonemes_length);
		phonetic[outcome.phonemes_length] = 0;
	}
	if (outcome.skipwords >= 0) dictionary_skipwords = outcome.skipwords;
	if (!outcome.has_flags) return NULL;
	flags[0] = outcome.flags[0]; flags[1] = outcome.flags[1];
	const char *word_end = word2 + outcome.word_end;
	if (option_phonemes & espeakPHONEMES_TRACE) {
		if (!outcome.found) {
			print_dictionary_flags(outcome.trace_flags, dict_flags_buf, sizeof(dict_flags_buf));
			fprintf(f_trans, "Flags:  %s  %s\n", word1, dict_flags_buf);
		} else {
			char ph_decoded[N_WORD_PHONEMES];
			DecodePhonemes(phonetic, ph_decoded);
			bool textmode = (flags[0] & FLAG_TEXTMODE) != 0;
			if (textmode == translator->langopts.textmode) {
				if ((flags[0] & FLAG_SKIPWORDS) && wtab != NULL) {
					size_t length = outcome.word_end < sizeof(word_buf) ? outcome.word_end : sizeof(word_buf) - 1;
					memcpy(word_buf, word2, length);
					word_buf[length ? length - 1 : 0] = 0;
					fprintf(f_trans, "Found: '%s %s\n", word1, word_buf);
				} else fprintf(f_trans, "Found: '%s", word1);
				print_dictionary_flags(outcome.trace_flags, dict_flags_buf, sizeof(dict_flags_buf));
				fprintf(f_trans, "' [%s]  %s\n", ph_decoded, dict_flags_buf);
			}
		}
	}
	return outcome.found ? word_end : NULL;
}
