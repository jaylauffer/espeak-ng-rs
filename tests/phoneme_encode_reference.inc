/* Actual encoder from 98f1dc3c dictionary.c, unchanged body. */
const char *EncodePhonemes(const char *p, char *outptr, int *bad_phoneme)
{
	int ix;
	unsigned char c;
	int count;     // num. of matching characters
	int max;       // highest num. of matching found so far
	int max_ph;    // corresponding phoneme with highest matching
	int consumed;
	unsigned int mnemonic_word;

	if (bad_phoneme != NULL)
		*bad_phoneme = 0;

	// skip initial blanks
	while ((uint8_t)*p < 0x80 && isspace(*p))
		p++;

	while (((c = *p) != 0) && !isspace(c)) {
		consumed = 0;

		switch (c)
		{
		case '|':
			// used to separate phoneme mnemonics if needed, to prevent characters being treated
			// as a multi-letter mnemonic

			if ((c = p[1]) == '|') {
				// treat double || as a word-break symbol, drop through
				// to the default case with c = '|'
			} else {
				p++;
				break;
			}
		default:
			// lookup the phoneme mnemonic, find the phoneme with the highest number of
			// matching characters
			max = -1;
			max_ph = 0;

			for (ix = 1; ix < n_phoneme_tab; ix++) {
				if (phoneme_tab[ix] == NULL)
					continue;
				if (phoneme_tab[ix]->type == phINVALID)
					continue; // this phoneme is not defined for this language

				count = 0;
				mnemonic_word = phoneme_tab[ix]->mnemonic;

				while (((c = p[count]) > ' ') && (count < 4) &&
				       (c == ((mnemonic_word >> (count*8)) & 0xff)))
					count++;

				if ((count > max) &&
				    ((count == 4) || (((mnemonic_word >> (count*8)) & 0xff) == 0))) {
					max = count;
					max_ph = phoneme_tab[ix]->code;
				}
			}

			if (max_ph == 0) {
				// not recognised, report and ignore
				if (bad_phoneme != NULL)
					utf8_in(bad_phoneme, p);
				*outptr++ = 0;
				return p+1;
			}

			if (max <= 0)
				max = 1;
			p += (consumed + max);
			*outptr++ = (char)(max_ph);

			if (max_ph == phonSWITCH) {
				// Switch Language: this phoneme is followed by a text string
				char *p_lang = outptr;
				while (!isspace(c = *p) && (c != 0)) {
					p++;
					*outptr++ = tolower(c);
				}
				*outptr = 0;
				if (c == 0) {
					if (strcmp(p_lang, ESPEAKNG_DEFAULT_VOICE) == 0) {
						*p_lang = 0; // don't need ESPEAKNG_DEFAULT_VOICE, it's assumed by default
						return p;
					}
				} else
					*outptr++ = '|'; // more phonemes follow, terminate language string with separator
			}
			break;
		}
	}
	// terminate the encoded string
	*outptr = 0;
	return p;
}
