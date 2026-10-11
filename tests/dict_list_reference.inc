/* Frozen 11367a76 compiledict.c list parser, chains and file driver.
 * Policy below is unchanged; shared translator/encoding primitives remain external.
 * SPDX-License-Identifier: GPL-3.0-or-later */
typedef enum
{
	LINE_PARSER_WORD = 0,
	LINE_PARSER_END_OF_WORD = 1,
	LINE_PARSER_MULTIPLE_WORDS = 2,
	LINE_PARSER_END_OF_WORDS = 3,
	LINE_PARSER_PRONUNCIATION = 4,
	LINE_PARSER_END_OF_PRONUNCIATION = 5,
} LINE_PARSER_STATES;

static int compile_line(CompileContext *ctx, char *linebuf, char *dict_line, int n_dict_line, int *hash)
{
	// Compile a line in the language_list file
	unsigned char c;
	char *p;
	char *word;
	char *phonetic;
	char *phonetic_end;
	unsigned int ix;
	LINE_PARSER_STATES step;
	unsigned int n_flag_codes = 0;
	int flagnum;
	int flag_offset;
	int length;
	int multiple_words = 0;
	bool multiple_numeric_hyphen = false;
	char *multiple_string = NULL;
	char *multiple_string_end = NULL;

	int len_word;
	int len_phonetic;
	bool text_not_phonemes = false; // this word specifies replacement text, not phonemes
	unsigned int wc;
	bool all_upper_case;

	char *mnemptr;
	unsigned char flag_codes[100];
	char encoded_ph[200];
	char bad_phoneme_str[4];
	int bad_phoneme;
	static const char nullstring[] = { 0 };

	phonetic = word = (char*)nullstring;

	p = linebuf;

	step = LINE_PARSER_WORD;

	c = *p;
	while (c != '\n' && c != '\0') {
		c = *p;

		if ((c == '?') && (step == 0)) {
			// conditional rule, allow only if the numbered condition is set for the voice
			flag_offset = 100;

			p++;
			if (*p == '!') {
				// allow only if the numbered condition is NOT set
				flag_offset = 132;
				p++;
			}

			ix = 0;
			if (IsDigit09(*p)) {
				ix += (*p-'0');
				p++;
			}
			if (IsDigit09(*p)) {
				ix = ix*10 + (*p-'0');
				p++;
			}
			flag_codes[n_flag_codes++] = ix + flag_offset;
			c = *p;
		}

		if ((c == '$') && isalnum(p[1])) {
			// read keyword parameter
			mnemptr = p;
			while (!isspace2(c = *p)) p++;
			*p = 0;

			flagnum = LookupMnem(mnem_flags, mnemptr);
			if (flagnum > 0) {
				if (flagnum == 200)
					ctx->text_mode = true;
				else if (flagnum == 201)
					ctx->text_mode = false;
				else if (flagnum == BITNUM_FLAG_TEXTMODE)
					text_not_phonemes = true;
				else
					flag_codes[n_flag_codes++] = flagnum;
			} else {
				fprintf(ctx->f_log, "%5d: Unknown keyword: %s\n", ctx->linenum, mnemptr);
				ctx->error_count++;
			}
		}

		if ((c == '/') && (p[1] == '/') && (multiple_words == 0))
			c = '\n'; // "//" treat comment as end of line

		switch (step)
		{
		case LINE_PARSER_WORD:
			if (c == '(') {
				multiple_words = 1;
				word = p+1;
				step = LINE_PARSER_END_OF_WORD;
			} else if (!isspace2(c)) {
				word = p;
				step = LINE_PARSER_END_OF_WORD;
			}
			break;
		case LINE_PARSER_END_OF_WORD:
			if ((c == '-') && multiple_words) {
				if (IsDigit09(word[0]))
					multiple_numeric_hyphen = true;
				flag_codes[n_flag_codes++] = BITNUM_FLAG_HYPHENATED;
				c = ' ';
			}
			if (isspace2(c)) {
				p[0] = 0; // terminate english word

				if (multiple_words) {
					multiple_string = multiple_string_end = p+1;
					step = LINE_PARSER_MULTIPLE_WORDS;
				} else
					step = LINE_PARSER_END_OF_WORDS;
			} else if (c == ')') {
				if (multiple_words) {
					p[0] = 0;
					multiple_words = 0;
					step = LINE_PARSER_END_OF_WORDS;
				} else if (word[0] != '_') {
					fprintf(ctx->f_log, "%5d: Missing '('\n", ctx->linenum);
					ctx->error_count++;
					step = LINE_PARSER_END_OF_WORDS;
				}
			}
			break;
		case LINE_PARSER_MULTIPLE_WORDS:
			if (isspace2(c))
				multiple_words++;
			else if (c == ')') {
				p[0] = ' '; // terminate extra string
				multiple_string_end = p+1;
				step = LINE_PARSER_END_OF_WORDS;
			}
			break;
		case LINE_PARSER_END_OF_WORDS:
			if (!isspace2(c)) {
				phonetic = p;
				step = LINE_PARSER_PRONUNCIATION;
			}
			break;
		case LINE_PARSER_PRONUNCIATION:
			if (isspace2(c)) {
				phonetic_end = p;
				p[0] = 0; // terminate phonetic
				step = LINE_PARSER_END_OF_PRONUNCIATION;
			}
			break;
		case LINE_PARSER_END_OF_PRONUNCIATION:
			if (!isspace2(c)) {
				*phonetic_end = ' ';
				step = LINE_PARSER_PRONUNCIATION;
			}
			break;
		}
		p++;
	}

	if (word[0] == 0)
		return 0; // blank line

	if (ctx->text_mode)
		text_not_phonemes = true;

	if (text_not_phonemes) {
		if (word[0] == '_') {
			// This is a special word, used by eSpeak.  Translate this into phonemes now
			strcat(phonetic, " "); // need a space to indicate word-boundary

			// PROBLEM  vowel reductions are not applied to the translated phonemes
			// condition rules are not applied
			TranslateWord(translator, phonetic, NULL, NULL);
			text_not_phonemes = false;
			strncpy0(encoded_ph, ctx->word_phonemes, N_WORD_BYTES-4);

			if ((ctx->word_phonemes[0] == 0) && (ctx->error_need_dictionary < 3)) {
				// the dictionary was not loaded, we need a second attempt
				ctx->error_need_dictionary++;
				fprintf(ctx->f_log, "%5d: Need to compile dictionary again\n", ctx->linenum);
			}
		} else
			// this is replacement text, so don't encode as phonemes. Restrict the length of the replacement word
			strncpy0(encoded_ph, phonetic, N_WORD_BYTES-4);
	} else {
		if (EncodePhonemesBounded(phonetic, encoded_ph, sizeof(encoded_ph), &bad_phoneme) == NULL) {
			fprintf(ctx->f_log, "%5d: Encoded phonemes exceed output buffer\n", ctx->linenum);
			ctx->error_count++;
			return 0;
		}
		if (strchr(encoded_ph, phonSWITCH) != 0)
			flag_codes[n_flag_codes++] = BITNUM_FLAG_ONLY_S;  // don't match on suffixes (except 's') when switching languages

		// check for errors in the phonemes codes
		if (bad_phoneme != 0) {
			// unrecognised phoneme, report error
			bad_phoneme_str[utf8_out(bad_phoneme, bad_phoneme_str)] = 0;
			fprintf(ctx->f_log, "%5d: Bad phoneme [%s] (U+%x) in: %s  %s\n", ctx->linenum, bad_phoneme_str, bad_phoneme, word, phonetic);
			ctx->error_count++;
		}
	}

	if (text_not_phonemes != translator->langopts.textmode)
		flag_codes[n_flag_codes++] = BITNUM_FLAG_TEXTMODE;

	if (sscanf(word, "U+%x", &wc) == 1) {
		// Character code
		ix = utf8_out(wc, word);
		word[ix] = 0;
	} else if (word[0] != '_') {
		// convert to lower case, and note if the word is all-capitals
		int c2;

		all_upper_case = true;
		for (p = word;;) {
			// this assumes that the lower case char is the same length as the upper case char
			// OK, except for Turkish "I", but use towlower() rather than towlower2()
			ix = utf8_in(&c2, p);
			if (c2 == 0)
				break;
			if (iswupper(c2))
				utf8_out(towlower2(c2, translator), p);
			else
				all_upper_case = false;
			p += ix;
		}
		if (all_upper_case)
			flag_codes[n_flag_codes++] = BITNUM_FLAG_ALLCAPS;
	}

	{
		// strip variation selectors (U+FE0E text style, U+FE0F emoji style) from
		// the word: TranslateClause discards them from the input text, so they
		// must not appear in dictionary keys (some emoji entries contain U+FE0F)
		char *ps = word;
		char *pd = word;
		int c2;

		while (*ps != 0) {
			ix = utf8_in(&c2, ps);
			if ((c2 != 0xfe0e) && (c2 != 0xfe0f)) {
				if (pd != ps)
					memmove(pd, ps, ix);
				pd += ix;
			}
			ps += ix;
		}
		*pd = 0;
	}

	if (word[0] == 0)
		return 0; // the word contained only variation selectors

	len_word = strlen(word);

	if (translator->transpose_min > 0)
		len_word = TransposeAlphabet(translator, word);

	*hash = HashDictionary(word);
	len_phonetic = strlen(encoded_ph);

	dict_line[1] = len_word; // bit 6 indicates whether the word has been compressed
	len_word &= 0x3f;

	memcpy(&dict_line[2], word, len_word);

	if (len_phonetic == 0) {
		// no phonemes specified. set bit 7
		dict_line[1] |= 0x80;
		length = len_word + 2;
	} else {
		length = len_word + len_phonetic + 3;
		if (length < n_dict_line) {
			strcpy(&dict_line[(len_word)+2], encoded_ph);
		} else {
			fprintf(ctx->f_log, "%5d: Dictionary line length would overflow the data buffer: %d\n", ctx->linenum, length);
			ctx->error_count++;
			// no phonemes specified. set bit 7
			dict_line[1] |= 0x80;
			length = len_word + 2;
		}
	}

	for (ix = 0; ix < n_flag_codes; ix++)
		dict_line[ix+length] = flag_codes[ix];
	length += n_flag_codes;

	if ((multiple_string != NULL) && (multiple_words > 0)) {
		if (multiple_words > 10) {
			fprintf(ctx->f_log, "%5d: Two many parts in a multi-word entry: %d\n", ctx->linenum, multiple_words);
			ctx->error_count++;
		} else {
			dict_line[length++] = 80 + multiple_words;
			ix = multiple_string_end - multiple_string;
			if (multiple_numeric_hyphen)
				dict_line[length++] = ' ';   // ???
			memcpy(&dict_line[length], multiple_string, ix);
			length += ix;
		}
	}
	*((uint8_t *)dict_line) = (uint8_t)length;

	return length;
}

static void compile_dictlist_start(CompileContext *ctx)
{
	// initialise dictionary list
	int ix;
	char *p;
	char *p2;

	for (ix = 0; ix < N_HASH_DICT; ix++) {
		p = ctx->hash_chains[ix];
		while (p != NULL) {
			memcpy(&p2, p, sizeof(char *));
			free(p);
			p = p2;
		}
		ctx->hash_chains[ix] = NULL;
	}
}

static void compile_dictlist_end(CompileContext *ctx, FILE *f_out)
{
	// Write out the compiled dictionary list
	int hash;
	int length;
	char *p;

	for (hash = 0; hash < N_HASH_DICT; hash++) {
		p = ctx->hash_chains[hash];

		while (p != NULL) {
			length = *(uint8_t *)(p+sizeof(char *));
			fwrite(p+sizeof(char *), length, 1, f_out);
			memcpy(&p, p, sizeof(char *));
		}
		fputc(0, f_out);
	}
}

static int compile_dictlist_file(CompileContext *ctx, const char *path, const char *filename)
{
	int length;
	int hash;
	char *p;
	int count = 0;
	FILE *f_in;
	char buf[200];
	char fname[N_PATH_BUF];
	char dict_line[256]; // length is uint8_t, so an entry can't take up more than 256 bytes

	ctx->text_mode = false;

	// try with and without '.txt' extension
	sprintf(fname, "%s%s.txt", path, filename);
	if ((f_in = fopen(fname, "r")) == NULL) {
		sprintf(fname, "%s%s", path, filename);
		if ((f_in = fopen(fname, "r")) == NULL)
			return -1;
	}

	if (ctx->f_log != NULL)
		fprintf(ctx->f_log, "Compiling: '%s'\n", fname);

	ctx->linenum = 0;

	while (fgets(buf, sizeof(buf), f_in) != NULL) {
		ctx->linenum++;

		size_t buf_len = strlen(buf);
		if (buf_len > 0 && buf[buf_len-1] != '\n' && !feof(f_in)) {
			// The line was longer than buf and has been split by fgets.
			// Discard the remainder so it is not parsed as a separate
			// (bogus) entry on the next iteration. Without this, a long
			// entry with a trailing comment (e.g. a Sinhala or Myanmar
			// emoji line) leaks a spurious entry whose key can hijack
			// another word, such as a bare digit.
			int ch;
			while ((ch = fgetc(f_in)) != EOF && ch != '\n')
				;
		}

		length = compile_line(ctx, buf, dict_line, sizeof(dict_line), &hash);
		if (length == 0)  continue; // blank line

		p = (char *)malloc(length+sizeof(char *));
		if (p == NULL) {
			if (ctx->f_log != NULL) {
				fprintf(ctx->f_log, "Can't allocate memory\n");
				ctx->error_count++;
			}
			break;
		}

		memcpy(p, &ctx->hash_chains[hash], sizeof(char *));
		ctx->hash_chains[hash] = p;
		// NOTE: dict_line[0] is the entry length (0-255)
		memcpy(p+sizeof(char *), dict_line, length);
		count++;
	}

	if (ctx->f_log != NULL)
		fprintf(ctx->f_log, "\t%d entries\n", count);
	fclose(f_in);
	return 0;
}

/* End frozen dictionary list policy. */
