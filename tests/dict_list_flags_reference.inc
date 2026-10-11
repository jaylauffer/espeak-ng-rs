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

/* End frozen dictionary list flags. */
