/* Frozen SetParameter/DoEmbedded2/length tables and selection from 86294094. GPL-3.0-or-later. */
espeak_ng_STATUS SetParameter(int parameter, int value, int relative)
{
	// parameter: reset-all, amp, pitch, speed, linelength, expression, capitals, number grouping
	// relative 0=absolute  1=relative

	int new_value = value;
	extern const int param_defaults[N_SPEECH_PARAM];

	if (relative) {
		if (parameter < 5) {
			int default_value;
			default_value = param_defaults[parameter];
			new_value = default_value + (default_value * value)/100;
		}
	}
	param_stack[0].parameter[parameter] = new_value;
	saved_parameters[parameter] = new_value;

	switch (parameter)
	{
	case espeakRATE:
		embedded_value[EMBED_S] = new_value;
		embedded_value[EMBED_S2] = new_value;
		SetSpeed(3);
		break;
	case espeakVOLUME:
		embedded_value[EMBED_A] = new_value;
		GetAmplitude();
		break;
	case espeakPITCH:
		if (new_value > 99) new_value = 99;
		if (new_value < 0) new_value = 0;
		embedded_value[EMBED_P] = new_value;
		break;
	case espeakRANGE:
		if (new_value > 99) new_value = 99;
		embedded_value[EMBED_R] = new_value;
		break;
	case espeakLINELENGTH:
		option_linelength = new_value;
		break;
	case espeakWORDGAP:
		option_wordgap = new_value;
		break;
	case espeakINTONATION:
		if ((new_value & 0xff) != 0)
			translator->langopts.intonation_group = new_value & 0xff;
		option_tone_flags = new_value;
		break;
  case espeakSSML_BREAK_MUL:
    break;
	default:
		return EINVAL;
	}
	return ENS_OK;
}

static void DoEmbedded2(int *embix)
{
	// There were embedded commands in the text at this point

	unsigned int word;

	do {
		word = embedded_list[(*embix)++];

		if ((word & 0x1f) == EMBED_S) {
			// speed
			SetEmbedded(word & 0x7f, word >> 8); // adjusts embedded_value[EMBED_S]
			SetSpeed(1);
		}
	} while ((word & 0x80) == 0);
}

// Tables of the relative lengths of vowels, depending on the
// type of the two phonemes that follow
// indexes are the "length_mod" value for the following phonemes

// use this table if vowel is not the last in the word
static const unsigned char length_mods_en[LENGTH_MOD_LIMIT * LENGTH_MOD_LIMIT] = {
//	a    ,    t    s    n    d    z    r    N    <- next
	100, 120, 100, 105, 100, 110, 110, 100,  95, 100, // a  <- next2
	105, 120, 105, 110, 125, 130, 135, 115, 125, 100, // ,
	105, 120,  75, 100,  75, 105, 120,  85,  75, 100, // t
	105, 120,  85, 105,  95, 115, 120, 100,  95, 100, // s
	110, 120,  95, 105, 100, 115, 120, 100, 100, 100, // n
	105, 120, 100, 105,  95, 115, 120, 110,  95, 100, // d
	105, 120, 100, 105, 105, 122, 125, 110, 105, 100, // z
	105, 120, 100, 105, 105, 122, 125, 110, 105, 100, // r
	105, 120,  95, 105, 100, 115, 120, 110, 100, 100, // N
	100, 120, 100, 100, 100, 100, 100, 100, 100, 100
};

// as above, but for the last syllable in a word
static const unsigned char length_mods_en0[LENGTH_MOD_LIMIT * LENGTH_MOD_LIMIT] = {
//	a    ,    t    s    n    d    z    r    N    <- next
	100, 150, 100, 105, 110, 115, 110, 110, 110, 100, // a  <- next2
	105, 150, 105, 110, 125, 135, 140, 115, 135, 100, // ,
	105, 150,  90, 105,  90, 122, 135, 100,  90, 100, // t
	105, 150, 100, 105, 100, 122, 135, 100, 100, 100, // s
	105, 150, 100, 105, 105, 115, 135, 110, 105, 100, // n
	105, 150, 100, 105, 105, 122, 130, 120, 125, 100, // d
	105, 150, 100, 105, 110, 122, 125, 115, 110, 100, // z
	105, 150, 100, 105, 105, 122, 135, 120, 105, 100, // r
	105, 150, 100, 105, 105, 115, 135, 110, 105, 100, // N
	100, 100, 100, 100, 100, 100, 100, 100, 100, 100
};


static const unsigned char length_mods_equal[LENGTH_MOD_LIMIT * LENGTH_MOD_LIMIT] = {
//	a    ,    t    s    n    d    z    r    N    <- next
	110, 120, 100, 110, 110, 110, 110, 110, 110, 110, // a  <- next2
	110, 120, 100, 110, 110, 110, 110, 110, 110, 110, // ,
	110, 120, 100, 110, 100, 110, 110, 110, 100, 110, // t
	110, 120, 100, 110, 110, 110, 110, 110, 110, 110, // s
	110, 120, 100, 110, 110, 110, 110, 110, 110, 110, // n
	110, 120, 100, 110, 110, 110, 110, 110, 110, 110, // d
	110, 120, 100, 110, 110, 110, 110, 110, 110, 110, // z
	110, 120, 100, 110, 110, 110, 110, 110, 110, 110, // r
	110, 120, 100, 110, 110, 110, 110, 110, 110, 110, // N
	110, 120, 100, 110, 110, 110, 110, 110, 110, 110
};

static const unsigned char *const length_mod_tabs[6] = {
	length_mods_en,
	length_mods_en,    // 1
	length_mods_en0,   // 2
	length_mods_equal, // 3
	length_mods_equal, // 4
	length_mods_equal  // 5
};

void SetLengthMods(Translator *tr, int value)
{
	int value2;

	tr->langopts.length_mods0 = tr->langopts.length_mods = length_mod_tabs[value % 100];
	if ((value2 = value / (LENGTH_MOD_LIMIT * LENGTH_MOD_LIMIT)) != 0)
		tr->langopts.length_mods0 = length_mod_tabs[value2];
}
