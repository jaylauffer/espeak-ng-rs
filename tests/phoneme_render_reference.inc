/* Actual 392b5575 renderer, unchanged policy. */
static const unsigned short ipa1[96] = {
	0x20,  0x21,  0x22,  0x2b0, 0x24,  0x25,  0x0e6, 0x2c8, 0x28,  0x29,  0x27e, 0x2b,  0x2cc, 0x2d,  0x2e,  0x2f,
	0x252, 0x31,  0x32,  0x25c, 0x34,  0x35,  0x36,  0x37,  0x275, 0x39,  0x2d0, 0x2b2, 0x3c,  0x3d,  0x3e,  0x294,
	0x259, 0x251, 0x3b2, 0xe7,  0xf0,  0x25b, 0x46,  0x262, 0x127, 0x26a, 0x25f, 0x4b,  0x26b, 0x271, 0x14b, 0x254,
	0x3a6, 0x263, 0x280, 0x283, 0x3b8, 0x28a, 0x28c, 0x153, 0x3c7, 0xf8,  0x292, 0x32a, 0x5c,  0x5d,  0x5e,  0x5f,
	0x60,  0x61,  0x62,  0x63,  0x64,  0x65,  0x66,  0x261, 0x68,  0x69,  0x6a,  0x6b,  0x6c,  0x6d,  0x6e,  0x6f,
	0x70,  0x71,  0x72,  0x73,  0x74,  0x75,  0x76,  0x77,  0x78,  0x79,  0x7a,  0x7b,  0x7c,  0x7d,  0x303, 0x7f
};

#define N_PHON_OUT  500  // realloc increment
static char *phon_out_buf = NULL;   // passes the result of GetTranslatedPhonemeString()
static unsigned int phon_out_size = 0;

char *WritePhMnemonic(char *phon_out, PHONEME_TAB *ph, PHONEME_LIST *plist, int use_ipa, int *flags)
{
	int c;
	int mnem;
	int len;
	bool first;
	int ix = 0;
	char *p;
	PHONEME_DATA phdata;

	if (ph->code == phonEND_WORD) {
		// ignore
		phon_out[0] = 0;
		return phon_out;
	}

	if (ph->code == phonSWITCH) {
		// the tone_ph field contains a phoneme table number
		p = phoneme_tab_list[plist->tone_ph].name;
		sprintf(phon_out, "(%s)", p);
		return phon_out + strlen(phon_out);
	}

	if (use_ipa) {
		// has an ipa name been defined for this phoneme ?
		phdata.ipa_string[0] = 0;

		if (plist == NULL)
			InterpretPhoneme2(ph->code, &phdata);
		else
			InterpretPhonemeWithLength(NULL, 0, plist, phoneme_list, &phdata, NULL, n_phoneme_list);

		p = phdata.ipa_string;
		if (*p == 0x20) {
			// indicates no name for this phoneme
			*phon_out = 0;
			return phon_out;
		}
		if ((*p != 0) && ((*p & 0xff) < 0x20)) {
			// name starts with a flags byte
			if (flags != NULL)
				*flags = *p;
			p++;
		}

		len = strlen(p);
		if (len > 0) {
			strcpy(phon_out, p);
			phon_out += len;
			*phon_out = 0;
			return phon_out;
		}
	}

	first = true;
	for (mnem = ph->mnemonic; (c = mnem & 0xff) != 0; mnem = mnem >> 8) {
		if (c == '/')
			break; // discard phoneme variant indicator

		if (use_ipa) {
			// convert from ascii to ipa
			if (first && (c == '_'))
				break; // don't show pause phonemes

			if ((c == '#') && (ph->type == phVOWEL))
				break; // # is subscript-h, but only for consonants

			// ignore digits after the first character
			if (!first && IsDigit09(c))
				continue;

			if ((c >= 0x20) && (c < 128))
				c = ipa1[c-0x20];

			ix += utf8_out(c, &phon_out[ix]);
		} else
			phon_out[ix++] = c;
		first = false;
	}

	phon_out = &phon_out[ix];
	*phon_out = 0;
	return phon_out;
}

//// Extension: write phone mnemonic with stress
char *WritePhMnemonicWithStress(char *phon_out, PHONEME_TAB *ph, PHONEME_LIST *plist, int use_ipa, int *flags) {
	if (plist->synthflags & SFLAG_SYLLABLE) {
		unsigned char stress = plist->stresslevel;

		if (stress > 1) {
			int c = 0;

			if (stress > STRESS_IS_PRIORITY) {
				stress = STRESS_IS_PRIORITY;
			}

			if (use_ipa) {
				c = 0x2cc; // ipa, secondary stress

				if (stress > STRESS_IS_SECONDARY) {
					c = 0x02c8; // ipa, primary stress
				}
			} else {
				const char stress_chars[] = "==,,''";

				c = stress_chars[stress];
			}

			if (c != 0) {
				phon_out += utf8_out(c, phon_out);
			}
		}
	}

	return WritePhMnemonic(phon_out, ph, plist, use_ipa, flags);
}
////

const char *GetTranslatedPhonemeString(int phoneme_mode)
{
	/* Called after a clause has been translated into phonemes, in order
	   to display the clause in phoneme mnemonic form.

	   phoneme_mode
	                 bit  1:   use IPA phoneme names
	                 bit  7:   use tie between letters in multi-character phoneme names
	                 bits 8-23 tie or separator character

	 */

	int ix;
	unsigned int len;
	int phon_out_ix = 0;
	int stress;
	int c;
	char *p;
	char *buf;
	int count;
	int flags;
	int use_ipa;
	int use_tie;
	int separate_phonemes;
	char phon_buf[30];
	char phon_buf2[30];
	PHONEME_LIST *plist;

	static const char stress_chars[] = "==,,''";

	if (phon_out_buf == NULL) {
		phon_out_size = N_PHON_OUT;
		if ((phon_out_buf = (char *)malloc(phon_out_size)) == NULL) {
			phon_out_size = 0;
			return "";
		}
	}

	use_ipa = phoneme_mode & espeakPHONEMES_IPA;
	if (phoneme_mode & espeakPHONEMES_TIE) {
		use_tie = phoneme_mode >> 8;
		separate_phonemes = 0;
	} else {
		separate_phonemes = phoneme_mode >> 8;
		use_tie = 0;
	}

	for (ix = 1; ix < (n_phoneme_list-2); ix++) {
		buf = phon_buf;

		plist = &phoneme_list[ix];

		WritePhMnemonic(phon_buf2, plist->ph, plist, use_ipa, &flags);
		if (plist->newword & PHLIST_START_OF_WORD && !(plist->newword & (PHLIST_START_OF_SENTENCE | PHLIST_START_OF_CLAUSE)))
			*buf++ = ' ';

		if ((!plist->newword) || (separate_phonemes == ' ')) {
			if ((separate_phonemes != 0) && (ix > 1)) {
				utf8_in(&c, phon_buf2);
				if ((c < 0x2b0) || (c > 0x36f)) // not if the phoneme starts with a superscript letter
					buf += utf8_out(separate_phonemes, buf);
			}
		}

		if (plist->synthflags & SFLAG_SYLLABLE) {
			if ((stress = plist->stresslevel) > 1) {
				c = 0;
				if (stress > STRESS_IS_PRIORITY) stress = STRESS_IS_PRIORITY;

				if (use_ipa) {
					c = 0x2cc; // ipa, secondary stress
					if (stress > STRESS_IS_SECONDARY)
						c = 0x02c8; // ipa, primary stress
				} else
					c = stress_chars[stress];

				if (c != 0)
					buf += utf8_out(c, buf);
			}
		}

		flags = 0;
		count = 0;
		for (p = phon_buf2; *p != 0;) {
			p += utf8_in(&c, p);
			if (use_tie != 0) {
				// look for non-initial alphabetic character, but not diacritic, superscript etc.
				if ((count > 0) && !(flags & (1 << (count-1))) && ((c < 0x2b0) || (c > 0x36f)) && iswalpha(c))
					buf += utf8_out(use_tie, buf);
			}
			buf += utf8_out(c, buf);
			count++;
		}

		if (plist->ph->code != phonSWITCH) {
			if (plist->synthflags & SFLAG_LENGTHEN)
				buf = WritePhMnemonic(buf, phoneme_tab[phonLENGTHEN], plist, use_ipa, NULL);
			if ((plist->synthflags & SFLAG_SYLLABLE) && (plist->type != phVOWEL)) {
				// syllablic consonant
				buf = WritePhMnemonic(buf, phoneme_tab[phonSYLLABIC], plist, use_ipa, NULL);
			}
			if (plist->tone_ph > 0) {
				PHONEME_TAB *tone_ph = TonePhoneme(plist);
				if (tone_ph != NULL)
					buf = WritePhMnemonic(buf, tone_ph, plist, use_ipa, NULL);
			}
		}

		len = buf - phon_buf;
		if ((phon_out_ix + len) >= phon_out_size) {
			// enlarge the phoneme buffer
			phon_out_size = phon_out_ix + len + N_PHON_OUT;
			char *new_phon_out_buf = (char *)realloc(phon_out_buf, phon_out_size);
			if (new_phon_out_buf == NULL) {
				phon_out_size = 0;
				return "";
			} else
				phon_out_buf = new_phon_out_buf;
		}

		phon_buf[len] = 0;
		strcpy(&phon_out_buf[phon_out_ix], phon_buf);
		phon_out_ix += len;
	}

	if (!phon_out_buf)
		return "";

	phon_out_buf[phon_out_ix] = 0;

	return phon_out_buf;
}