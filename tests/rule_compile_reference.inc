/* Frozen df14a1fc compiledict.c mnemonic table and complete individual rule compiler.
 * Copyright (C) 2005-2014 Jonathan Duddington; 2015-2017 Reece H. Dunn.
 * SPDX-License-Identifier: GPL-3.0-or-later */
static const MNEM_TAB mnem_rules[] = {
	{ "unpr",     DOLLAR_UNPR },
	{ "noprefix", DOLLAR_NOPREFIX },  // rule fails if a prefix has been removed
	{ "list",     DOLLAR_LIST },    // a pronunciation is given in the *_list file

	{ "w_alt1", 0x11 },
	{ "w_alt2", 0x12 },
	{ "w_alt3", 0x13 },
	{ "w_alt4", 0x14 },
	{ "w_alt5", 0x15 },
	{ "w_alt6", 0x16 },
	{ "w_alt",  0x11 }, // note: put longer names before their sub-strings

	{ "p_alt1", 0x21 },
	{ "p_alt2", 0x22 },
	{ "p_alt3", 0x23 },
	{ "p_alt4", 0x24 },
	{ "p_alt5", 0x25 },
	{ "p_alt6", 0x26 },
	{ "p_alt",  0x21 },

	{ NULL, -1 }
};

static int isHexDigit(int c)
{
	if ((c >= '0') && (c <= '9'))
		return c - '0';
	if ((c >= 'a') && (c <= 'f'))
		return c - 'a' + 10;
	if ((c >= 'A') && (c <= 'F'))
		return c - 'A' + 10;
	return -1;
}

static void copy_rule_string(CompileContext *ctx, char *string, int *state_out)
{
	// state 0: conditional, 1=pre, 2=match, 3=post, 4=phonemes
	char * const outbuf[5] = { ctx->rule_cond, ctx->rule_pre, ctx->rule_match, ctx->rule_post, ctx->rule_phonemes };
	static const int next_state[5] = { 2, 2, 4, 4, 4 };
	char *output;
	char *p;
	int ix;
	int len;
	char c;
	int c2, c3;
	int sxflags;
	int value;
	bool literal;
	bool hexdigit_input = false;
	int state = *state_out;
	const MNEM_TAB *mr;

	if (string[0] == 0) return;

	output = outbuf[state];
	if (state == 4) {
		// append to any previous phoneme string, i.e. allow spaces in the phoneme string
		len = strlen(ctx->rule_phonemes);
		if (len > 0)
			ctx->rule_phonemes[len++] = ' ';
		output = &ctx->rule_phonemes[len];
	}
	sxflags = 0x808000; // to ensure non-zero bytes

	for (p = string, ix = 0;;) {
		literal = false;
		c = *p++;
		if ((c == '0') && (p[0] == 'x') && (isHexDigit(p[1]) >= 0) && (isHexDigit(p[2]) >= 0)) {
			hexdigit_input = true;
			c = p[1];
			p += 2;
		}
		if (c == '\\') {
			c = *p++; // treat next character literally
			if ((c >= '0') && (c <= '3') && (p[0] >= '0') && (p[0] <= '7') && (p[1] >= '0') && (p[1] <= '7')) {
				// character code given by 3 digit octal value;
				c = (c-'0')*64 + (p[0]-'0')*8 + (p[1]-'0');
				p += 2;
			}
			literal = true;
		}
		if (hexdigit_input) {
			if (((c2 = isHexDigit(c)) >= 0) && ((c3 = isHexDigit(p[0])) >= 0)) {
				c = c2 * 16 + c3;
				literal = true;
				p++;
			} else
				hexdigit_input = false;
		}
		if ((state == 1) || (state == 3)) {
			// replace special characters (note: 'E' is reserved for a replaced silent 'e')
			if (literal == false) {
				static const char lettergp_letters[9] = { LETTERGP_A, LETTERGP_B, LETTERGP_C, 0, 0, LETTERGP_F, LETTERGP_G, LETTERGP_H, LETTERGP_Y };
				switch (c)
				{
				case '_':
					c = RULE_SPACE;
					break;

				case 'Y':
					c = 'I';
					// fallthrough:
				case 'A': // vowel
				case 'B':
				case 'C':
				case 'H':
				case 'F':
				case 'G':
					if (state == 1) {
						// pre-rule, put the number before the RULE_LETTERGP;
						output[ix++] = lettergp_letters[c-'A'] + 'A';
						c = RULE_LETTERGP;
					} else {
						output[ix++] = RULE_LETTERGP;
						c = lettergp_letters[c-'A'] + 'A';
					}
					break;
				case 'D':
					c = RULE_DIGIT;
					break;
				case 'K':
					c = RULE_NOTVOWEL;
					break;
				case 'N':
					c = RULE_NO_SUFFIX;
					break;
				case 'V':
					c = RULE_IFVERB;
					break;
				case 'Z':
					c = RULE_NONALPHA;
					break;
				case '+':
					c = RULE_INC_SCORE;
					break;
				case '<': // Can't use - as opposite for + because it is used literally as part of word
					c = RULE_DEC_SCORE;
					break;
				case '@':
					c = RULE_SYLLABLE;
					break;
				case '&':
					c = RULE_STRESSED;
					break;
				case '%':
					c = RULE_DOUBLE;
					break;
				case '#':
					c = RULE_DEL_FWD;
					break;
				case '!':
					c = RULE_CAPITAL;
					break;
				case 'T':
					output[ix++] = RULE_DOLLAR;
					c = 0x11;
					break;
				case 'W':
					c = RULE_SPELLING;
					break;
				case 'X':
					c = RULE_NOVOWELS;
					break;
				case 'J':
					c = RULE_SKIPCHARS;
					break;
				case 'L':
					// expect two digits
					c = *p++ - '0';
					value = *p++ - '0';
					c = c * 10 + value;
					if ((value < 0) || (value > 9)) {
						c = 0;
						fprintf(ctx->f_log, "%5d: Expected 2 digits after 'L'\n", ctx->linenum);
						ctx->error_count++;
					} else if ((c <= 0) || (c >= N_LETTER_GROUPS) || (ctx->letterGroupsDefined[(int)c] == 0)) {
						fprintf(ctx->f_log, "%5d: Letter group L%.2d not defined\n", ctx->linenum, c);
						ctx->error_count++;
					}
					c += 'A';
					if (state == 1) {
						// pre-rule, put the group number before the RULE_LETTERGP command
						output[ix++] = c;
						c = RULE_LETTERGP2;
					} else
						output[ix++] = RULE_LETTERGP2;
					break;
				case '$':
					value = 0;
					mr = mnem_rules;
					while (mr->mnem != NULL) {
						len = strlen(mr->mnem);
						if (strncmp(p, mr->mnem, len) == 0) {
							value = mr->value;
							p += len;
							break;
						}
						mr++;
					}

					if (state == 1) {
						// pre-rule, put the number before the RULE_DOLLAR
						output[ix++] = value;
						c = RULE_DOLLAR;
					} else {
						output[ix++] = RULE_DOLLAR;
						c = value;
					}

					if (value == 0) {
						fprintf(ctx->f_log, "%5d: $ command not recognized\n", ctx->linenum);
						ctx->error_count++;
					}
					break;
				case 'P': // Prefix
					sxflags |= SUFX_P;
					// fallthrough
				case 'S': // Suffix
					output[ix++] = RULE_ENDING;
					value = 0;
					while (!isspace2(c = *p++) && (c != 0)) {
						switch (c)
						{
						case 'e':
							sxflags |= SUFX_E;
							break;
						case 'i':
							sxflags |= SUFX_I;
							break;
						case 'p': // obsolete, replaced by 'P' above
							sxflags |= SUFX_P;
							break;
						case 'v':
							sxflags |= SUFX_V;
							break;
						case 'd':
							sxflags |= SUFX_D;
							break;
						case 'f':
							sxflags |= SUFX_F;
							break;
						case 'q':
							sxflags |= SUFX_Q;
							break;
						case 't':
							sxflags |= SUFX_T;
							break;
						case 'b':
							sxflags |= SUFX_B;
							break;
						case 'a':
							sxflags |= SUFX_A;
							break;
						case 'm':
							sxflags |= SUFX_M;
							break;
						default:
							if (IsDigit09(c))
								value = (value*10) + (c - '0');
							break;
						}
					}
					p--;
					output[ix++] = sxflags >> 16;
					output[ix++] = sxflags >> 8;
					c = value | 0x80;
					break;
				}
			}
		}
		output[ix++] = c;
		if (c == 0) break;
	}

	*state_out = next_state[state];
}

static char *compile_rule(CompileContext *ctx, char *input)
{
	int ix;
	unsigned char c;
	int wc;
	char *p;
	char *prule;
	int len;
	int len_name;
	int start;
	int state = 2;
	bool finish = false;
	char buf[80];
	char output[150];
	int bad_phoneme;
	char bad_phoneme_str[4];

	buf[0] = 0;
	ctx->rule_cond[0] = 0;
	ctx->rule_pre[0] = 0;
	ctx->rule_post[0] = 0;
	ctx->rule_match[0] = 0;
	ctx->rule_phonemes[0] = 0;

	p = buf;

	for (ix = 0; finish == false; ix++) {
		switch (c = input[ix])
		{
		case ')': // end of prefix section
			*p = 0;
			state = 1;
			copy_rule_string(ctx, buf, &state);
			p = buf;
			break;
		case '(': // start of suffix section
			*p = 0;
			state = 2;
			copy_rule_string(ctx, buf, &state);
			state = 3;
			p = buf;
			if (input[ix+1] == ' ') {
				fprintf(ctx->f_log, "%5d: Syntax error. Space after (, or negative score for previous rule\n", ctx->linenum);
				ctx->error_count++;
			}
			break;
		case '\n': // end of line
		case '\r':
		case 0:    // end of line
			*p = 0;
			copy_rule_string(ctx, buf, &state);
			finish = true;
			break;
		case '\t': // end of section section
		case ' ':
			*p = 0;
			copy_rule_string(ctx, buf, &state);
			p = buf;
			break;
		case '?':
			if (state == 2)
				state = 0;
			else
				*p++ = c;
			break;
		default:
			*p++ = c;
			break;
		}
	}

	if (strcmp(ctx->rule_match, "$group") == 0)
		strcpy(ctx->rule_match, ctx->group_name);

	if (ctx->rule_match[0] == 0) {
		if (ctx->rule_post[0] != 0) {
			fprintf(ctx->f_log, "%5d: Syntax error\n", ctx->linenum);
			ctx->error_count++;
		}
		return NULL;
	}

	if (EncodePhonemesBounded(ctx->rule_phonemes, buf, sizeof(buf), &bad_phoneme) == NULL) {
		fprintf(ctx->f_log, "%5d: Encoded phonemes exceed output buffer\n", ctx->linenum);
		ctx->error_count++;
		return NULL;
	}
	if (bad_phoneme != 0) {
		bad_phoneme_str[utf8_out(bad_phoneme, bad_phoneme_str)] = 0;
		fprintf(ctx->f_log, "%5d: Bad phoneme [%s] (U+%x) in: %s\n", ctx->linenum, bad_phoneme_str, bad_phoneme, input);
		ctx->error_count++;
	}
	strcpy(output, buf);
	len = strlen(buf)+1;

	len_name = strlen(ctx->group_name);
	if ((len_name > 0) && (memcmp(ctx->rule_match, ctx->group_name, len_name) != 0)) {
		utf8_in(&wc, ctx->rule_match);
		if ((ctx->group_name[0] == '9') && IsDigit(wc)) {
			// numeric group, rule_match starts with a digit, so OK
		} else {
			fprintf(ctx->f_log, "%5d: Wrong initial letters '%s' for group '%s'\n", ctx->linenum, ctx->rule_match, ctx->group_name);
			ctx->error_count++;
		}
	}
	strcpy(&output[len], ctx->rule_match);
	len += strlen(ctx->rule_match);

	if (ctx->debug_flag) {
		output[len] = RULE_LINENUM;
		output[len+1] = (ctx->linenum % 255) + 1;
		output[len+2] = (ctx->linenum / 255) + 1;
		len += 3;
	}

	if (ctx->rule_cond[0] != 0) {
		if (ctx->rule_cond[0] == '!') {
			// allow the rule only if the condition number is NOT set for the voice
			ix = atoi(&ctx->rule_cond[1]) + 32;
		} else {
			// allow the rule only if the condition number is set for the voice
			ix = atoi(ctx->rule_cond);
		}

		if ((ix > 0) && (ix < 255)) {
			output[len++] = RULE_CONDITION;
			output[len++] = ix;
		} else {
			fprintf(ctx->f_log, "%5d: bad condition number ?%d\n", ctx->linenum, ix);
			ctx->error_count++;
		}
	}
	if (ctx->rule_pre[0] != 0) {
		start = 0;
		if (ctx->rule_pre[0] == RULE_SPACE) {
			// omit '_' at the beginning of the pre-string and imply it by using RULE_PRE_ATSTART
			c = RULE_PRE_ATSTART;
			start = 1;
		} else
			c = RULE_PRE;
		output[len++] = c;

		// output PRE string in reverse order
		for (ix = strlen(ctx->rule_pre)-1; ix >= start; ix--)
			output[len++] = ctx->rule_pre[ix];
	}

	if (ctx->rule_post[0] != 0) {
		sprintf(&output[len], "%c%s", RULE_POST, ctx->rule_post);
		len += (strlen(ctx->rule_post)+1);
	}
	output[len++] = 0;
	if ((prule = (char *)malloc(len)) != NULL)
		memcpy(prule, output, len);
	return prule;
}

/* End frozen individual rule compiler. */
