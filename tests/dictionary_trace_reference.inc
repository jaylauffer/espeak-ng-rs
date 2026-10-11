/* Frozen 62985518 trace tables and bodies, without policy edits. */
/* Copyright (C) 2005-2014 Jonathan Duddington, 2015-2017 Reece H. Dunn.
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

static const MNEM_TAB mnem_flags[] = {
	// these in the first group put a value in bits0-3 of dictionary_flags
	{ "$1",   0x41 }, // stress on 1st syllable
	{ "$2",   0x42 }, // stress on 2nd syllable
	{ "$3",   0x43 },
	{ "$4",   0x44 },
	{ "$5",   0x45 },
	{ "$6",   0x46 },
	{ "$7",   0x47 },
	{ "$u",   0x48 }, // reduce to unstressed
	{ "$u1",  0x49 },
	{ "$u2",  0x4a },
	{ "$u3",  0x4b },
	{ "$u+",  0x4c }, // reduce to unstressed, but stress at end of clause
	{ "$u1+", 0x4d },
	{ "$u2+", 0x4e },
	{ "$u3+", 0x4f },

	// these set the corresponding numbered bit if dictionary_flags
	{ "$pause",          8 }, // ensure pause before this word
	{ "$strend",         9 }, // full stress if at end of clause
	{ "$strend2",       10 }, // full stress if at end of clause, or only followed by unstressed
	{ "$unstressend",   11 }, // reduce stress at end of clause
	{ "$accent_before", 12 }, // used with accent names, say this accent name before the letter name
	{ "$abbrev",        13 }, // use this pronuciation rather than split into letters

	// language specific
	{ "$double",        14 }, // IT double the initial consonant of next word
	{ "$alt",           15 }, // use alternative pronunciation
	{ "$alt1",          15 }, // synonym for $alt
	{ "$alt2",          16 },
	{ "$alt3",          17 },
	{ "$alt4",          18 },
	{ "$alt5",          19 },
	{ "$alt6",          20 },
	{ "$alt7",          21 },

	{ "$combine",       23 }, // Combine with the next word

	{ "$dot",           24 }, // ignore '.' after this word (abbreviation)
	{ "$hasdot",        25 }, // use this pronunciation if there is a dot after the word

	{ "$max3",          27 }, // limit to 3 repetitions
	{ "$brk",           28 }, // a shorter $pause
	{ "$text",          29 }, // word translates to replcement text, not phonemes

	// flags in dictionary word 2
	{ "$verbf",      0x20 }, // verb follows
	{ "$verbsf",     0x21 }, // verb follows, allow -s suffix
	{ "$nounf",      0x22 }, // noun follows
	{ "$pastf",      0x23 }, // past tense follows
	{ "$verb",       0x24 }, // use this pronunciation when its a verb
	{ "$noun",       0x25 }, // use this pronunciation when its a noun
	{ "$past",       0x26 }, // use this pronunciation when its past tense
	{ "$verbextend", 0x28 }, // extend influence of 'verb follows'
	{ "$capital",    0x29 }, // use this pronunciation if initial letter is upper case
	{ "$allcaps",    0x2a }, // use this pronunciation if initial letter is upper case
	{ "$accent",     0x2b }, // character name is base-character name + accent name
	{ "$sentence",   0x2d }, // only if this clause is a sentence (i.e. terminator is {. ? !} not {, ; :}
	{ "$only",       0x2e }, // only match on this word without suffix
	{ "$onlys",      0x2f }, // only match with none, or with 's' suffix
	{ "$stem",       0x30 }, // must have a suffix
	{ "$atend",      0x31 }, // use this pronunciation if at end of clause
	{ "$atstart",    0x32 }, // use this pronunciation at start of clause
	{ "$native",     0x33 }, // not if we've switched translators

	// doesn't set dictionary_flags
	{ "$?",           100 }, // conditional rule, followed by byte giving the condition number

	{ "$textmode",    200 },
	{ "$phonememode", 201 },

	{ NULL, -1 }
};

void print_dictionary_flags(unsigned int *flags, char *buf, int buf_len)
{
	int stress;
	int ix;
	const char *name;
	int len;
	int total = 0;

	buf[0] = 0;
	if ((stress = flags[0] & 0xf) != 0) {
		sprintf(buf, "%s", LookupMnemName(mnem_flags, stress + 0x40));
		total = strlen(buf);
		buf += total;
	}

	for (ix = 8; ix < 64; ix++) {
		if (((ix < 30) && (flags[0] & (1 << ix))) || ((ix >= 0x20) && (flags[1] & (1 << (ix-0x20))))) {
			name = LookupMnemName(mnem_flags, ix);
			len = strlen(name) + 1;
			total += len;
			if (total >= buf_len)
				continue;
			sprintf(buf, " %s", name);
			buf += len;
		}
	}
}

char *DecodeRule(const char *group_chars, int group_length, char *rule, int control, char *output)
{
	// Convert compiled match template to ascii

	unsigned char rb;
	unsigned char c;
	char *p;
	char *p_end;
	int ix;
	int match_type;
	bool finished = false;
	int value;
	int linenum = 0;
	int flags;
	int suffix_char;
	int condition_num = 0;
	bool at_start = false;
	const char *name;
	char buf[200];
	char buf_pre[200];
	char suffix[20];

	static const char symbols[] = {
		' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
		'&', '%', '+', '#', 'S', 'D', 'Z', 'A', 'L', '!',
		' ', '@', '?', 'J', 'N', 'K', 'V', '?', 'T', 'X',
		'?', 'W'
	};

	static const char symbols_lg[] = { 'A', 'B', 'C', 'H', 'F', 'G', 'Y' };

	match_type = 0;
	buf_pre[0] = 0;

	for (ix = 0; ix < group_length; ix++)
		buf[ix] = group_chars[ix];
	buf[ix] = 0;

	p = &buf[strlen(buf)];
	while (!finished) {
		rb = *rule++;

		if (rb <= RULE_LINENUM) {
			switch (rb)
			{
			case 0:
			case RULE_PHONEMES:
				finished = true;
				break;
			case RULE_PRE_ATSTART:
				at_start = true;
				// fallthrough:
			case RULE_PRE:
				match_type = RULE_PRE;
				*p = 0;
				p = buf_pre;
				break;
			case RULE_POST:
				match_type = RULE_POST;
				*p = 0;
				strcat(buf, " (");
				p = &buf[strlen(buf)];
				break;
			case RULE_PH_COMMON:
				break;
			case RULE_CONDITION:
				// conditional rule, next byte gives condition number
				condition_num = *rule++;
				break;
			case RULE_LINENUM:
				value = (rule[1] & 0xff) - 1;
				linenum = (rule[0] & 0xff) - 1 + (value * 255);
				rule += 2;
				break;
			}
			continue;
		}

		if (rb == RULE_DOLLAR) {
			value = *rule++ & 0xff;
			if ((value != 0x01) || (control & FLAG_UNPRON_TEST)) {
				// TODO write the string backwards if in RULE_PRE
				p[0] = '$';
				name = LookupMnemName(mnem_rules, value);
				strcpy(&p[1], name);
				p += (strlen(name)+1);
			}
			c = ' ';
		} else if (rb == RULE_ENDING) {
			static const char flag_chars[] = "eipvdfq tba ";
			flags = ((rule[0] & 0x7f)<< 8) + (rule[1] & 0x7f);
			suffix_char = 'S';
			if (flags & (SUFX_P >> 8))
				suffix_char = 'P';
			sprintf(suffix, "%c%d", suffix_char, rule[2] & 0x7f);
			rule += 3;
			for (ix = 0; ix < 9; ix++) {
				if (flags & 1)
					sprintf(&suffix[strlen(suffix)], "%c", flag_chars[ix]);
				flags = (flags >> 1);
			}
			strcpy(p, suffix);
			p += strlen(suffix);
			c = ' ';
		} else if (rb == RULE_LETTERGP)
			c = symbols_lg[*rule++ - 'A'];
		else if (rb == RULE_LETTERGP2) {
			value = *rule++ - 'A';
			if (value < 0)
				value += 256;
			p[0] = 'L';
			p[1] = (value / 10) + '0';
			c = (value % 10) + '0';

			if (match_type == RULE_PRE) {
				p[0] = c;
				c = 'L';
			}
			p += 2;
		} else if (rb <= RULE_LAST_RULE)
			c = symbols[rb];
		else if (rb == RULE_SPACE)
			c = '_';
		else
			c = rb;
		*p++ = c;
	}
	*p = 0;

	p = output;
	p_end = p + sizeof(output) - 1;

	if (linenum > 0) {
		sprintf(p, "%5d:\t", linenum);
		p += 7;
	}
	if (condition_num > 0) {
		sprintf(p, "?%d ", condition_num);
		p = &p[strlen(p)];
	}
	if (((ix = strlen(buf_pre)) > 0) || at_start) {
		if (at_start)
			*p++ = '_';
		while ((--ix >= 0) && (p < p_end-3))
			*p++ = buf_pre[ix];
		*p++ = ')';
		*p++ = ' ';
	}
	*p = 0;

	buf[p_end - p] = 0; // prevent overflow in output[]
	strcat(p, buf);
	ix = strlen(output);
	while (ix < 8)
		output[ix++] = ' ';
	output[ix] = 0;
	return output;
}

/* End frozen dictionary trace. */
