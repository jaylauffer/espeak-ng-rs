/*
 * Copyright (C) 2005 to 2014 by Jonathan Duddington
 * email: jonsd@users.sourceforge.net
 * Copyright (C) 2013-2017 Reece H. Dunn
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program; if not, see: <http://www.gnu.org/licenses/>.
 */

/* Frozen complete reserve/lookup/find/import/start/end/packing controllers
 * from 92980b44 compiledata.c; test adapters select retained C output. */
static const MNEM_TAB reserved_phonemes[] = {
	{ "_\001",  phonCONTROL },      // NOT USED
	{ "%",      phonSTRESS_U },
	{ "%%",     phonSTRESS_D },
	{ ",",      phonSTRESS_2 },
	{ ",,",     phonSTRESS_3 },
	{ "'",      phonSTRESS_P },
	{ "''",     phonSTRESS_P2 },
	{ "=",      phonSTRESS_PREV },  //  stress previous syllable
	{ "_:",     phonPAUSE },        //  pause
	{ "_",      phonPAUSE_SHORT },  //  short pause
	{ "_!",     phonPAUSE_NOLINK }, //  short pause, no link
	{ ":",      phonLENGTHEN },
	{ "@",      phonSCHWA },
	{ "@-",     phonSCHWA_SHORT },
	{ "||",     phonEND_WORD },
	{ "1",      phonDEFAULTTONE },  // (numeral 1)  default tone (for tone language)
	{ "#X1",    phonCAPITAL },      // capital letter indication
	{ "?",      phonGLOTTALSTOP },  // glottal stop
	{ "-",      phonSYLLABIC },     // syllabic consonant
	{ "_^_",    phonSWITCH },       //  Change language
	{ "_X1",    phonX1 },           // a language specific action
	{ "_|",     phonPAUSE_VSHORT }, // very short pause
	{ "_::",    phonPAUSE_LONG },   // long pause
	{ "t#",     phonT_REDUCED },    // reduced version of [t]
	{ "'!",     phonSTRESS_TONIC }, // stress - emphasized
	{ "_;_",    phonPAUSE_CLAUSE }, // clause pause

	{ "#@",     phonVOWELTYPES },   // vowel type groups, these must be consecutive
	{ "#a",     phonVOWELTYPES+1 },
	{ "#e",     phonVOWELTYPES+2 },
	{ "#i",     phonVOWELTYPES+3 },
	{ "#o",     phonVOWELTYPES+4 },
	{ "#u",     phonVOWELTYPES+5 },

	{ NULL, 0 }
};

static void ReservePhCodes(CompileContext *ctx)
{
	// Reserve phoneme codes which have fixed numbers so that they can be
	// referred to from the program code.
	const MNEM_TAB *p;

	p = reserved_phonemes;
	while (p->mnem != NULL) {
		ctx->phoneme_tab2[p->value].mnemonic = StringToWord(p->mnem);
		ctx->phoneme_tab2[p->value].code = p->value;
		if (ctx->n_phcodes <= p->value)
			ctx->n_phcodes = p->value+1;
		p++;
	}
}

static int LookupPhoneme(CompileContext *ctx, const char *string, int control)
{
	// control = 0   explicit declaration
	// control = 1   declare phoneme if not found
	// control = 2   start looking after control & stress phonemes

	int ix;
	int start;
	int use;
	unsigned int word;

	if (strcmp(string, "NULL") == 0)
		return 1;

	ix = strlen(string);
	if ((ix == 0) || (ix > 4))
		error(ctx, "Bad phoneme name '%s'", string);
	word = StringToWord(string);

	// don't use phoneme number 0, reserved for string terminator
	start = 1;

	if (control == 2) {
		// don't look for control and stress phonemes (allows these characters to be
		// used for other purposes)
		start = 8;
	}

	use = 0;
	for (ix = start; ix < ctx->n_phcodes; ix++) {
		if (ctx->phoneme_tab2[ix].mnemonic == word)
			return ix;

		if ((use == 0) && (ctx->phoneme_tab2[ix].mnemonic == 0))
			use = ix;
	}

	if (use == 0) {
		if (control == 0)
			return -1;
		if (ctx->n_phcodes >= N_PHONEME_TAB-1)
			return -1; // phoneme table is full
		use = ctx->n_phcodes++;
	}

	// add this phoneme to the phoneme table
	ctx->phoneme_tab2[use].mnemonic = word;
	ctx->phoneme_tab2[use].type = phINVALID;
	ctx->phoneme_tab2[use].program = ctx->linenum; // for error report if the phoneme remains undeclared
	return use;
}


static PHONEME_TAB_LIST *FindPhonemeTable(CompileContext *ctx, const char *string)
{
	int ix;

	for (ix = 0; ix < ctx->n_phoneme_tabs; ix++) {
		if (strcmp(ctx->phoneme_tab_list2[ix].name, string) == 0)
			return &ctx->phoneme_tab_list2[ix];
	}
	error(ctx, "compile: unknown phoneme table: '%s'", string);
	return NULL;
}

static PHONEME_TAB *FindPhoneme(CompileContext *ctx, const char *string)
{
	PHONEME_TAB_LIST *phtab = NULL;
	int ix;
	unsigned int mnem;
	char *phname;
	char buf[200];

	// is this the name of a phoneme which is in scope
	if ((strlen(string) <= 4) && ((ix = LookupPhoneme(ctx, string, 0)) != -1))
		return &ctx->phoneme_tab2[ix];

	// no, treat the name as phonemetable/phoneme
	strcpy(buf, string);
	if ((phname = strchr(buf, '/')) != 0)
		*phname++ = 0;

	phtab = FindPhonemeTable(ctx, buf);
	if (phtab == NULL)
		return NULL; // phoneme table not found

	mnem = StringToWord(phname);
	for (ix = 1; ix < 256; ix++) {
		if (mnem == phtab->phoneme_tab_ptr[ix].mnemonic)
			return &phtab->phoneme_tab_ptr[ix];
	}

	error(ctx, "Phoneme reference not found: '%s'", string);
	return NULL;
}

static void ImportPhoneme(CompileContext *ctx)
{
	unsigned int ph_mnem;
	unsigned int ph_code;
	PHONEME_TAB *ph;

	NextItem(ctx, tSTRING);

	if ((ph = FindPhoneme(ctx, ctx->item_string)) == NULL) {
		error(ctx, "Cannot find phoneme '%s' to import.", ctx->item_string);
		return;
	}

	if (ctx->phoneme_out->phflags != 0 ||
	    ctx->phoneme_out->type != phINVALID ||
	    ctx->phoneme_out->start_type != 0 ||
	    ctx->phoneme_out->end_type != 0 ||
	    ctx->phoneme_out->std_length != 0 ||
	    ctx->phoneme_out->length_mod != 0) {
		error(ctx, "Phoneme import will override set properties.");
	}

	ph_mnem = ctx->phoneme_out->mnemonic;
	ph_code = ctx->phoneme_out->code;
	memcpy(ctx->phoneme_out, ph, sizeof(PHONEME_TAB));
	ctx->phoneme_out->mnemonic = ph_mnem;
	ctx->phoneme_out->code = ph_code;
	if (ctx->phoneme_out->type != phVOWEL)
		ctx->phoneme_out->end_type = 0;  // voicingswitch, this must be set later to refer to a local phoneme
}


static void WritePhonemeTables(CompileContext *ctx)
{
	int ix;
	int j;
	int value;
	PHONEME_TAB *p;

	value = ctx->n_phoneme_tabs;
#ifdef USE_RUST_CORE
	unsigned char header[4] = {value, 0, 0, 0};
	espeak_rs_compiler_output_write(ctx->output_owner, RUST_OUTPUT_TABLE, header, sizeof(header));
#else
	fputc(value, ctx->f_phtab);
	fputc(0, ctx->f_phtab);
	fputc(0, ctx->f_phtab);
	fputc(0, ctx->f_phtab);
#endif

	for (ix = 0; ix < ctx->n_phoneme_tabs; ix++) {
		p = ctx->phoneme_tab_list2[ix].phoneme_tab_ptr;
		int n = ctx->n_phcodes_list[ix];
		memset(&p[n], 0, sizeof(p[n]));
		p[n].mnemonic = 0; // terminate the phoneme table

		// count number of locally declared phonemes
		int count = 0;
		for (j = 0; j < n; j++) {
			if (ix == 0)
				p[j].phflags |= phLOCAL; // write all phonemes in the base phoneme table

			if (p[j].phflags & phLOCAL)
				count++;
		}
		ctx->phoneme_tab_list2[ix].n_phonemes = count+1;

#ifdef USE_RUST_CORE
		unsigned char table_header[4] = {count+1, ctx->phoneme_tab_list2[ix].includes, 0, 0};
		espeak_rs_compiler_output_write(ctx->output_owner, RUST_OUTPUT_TABLE, table_header, sizeof(table_header));
		espeak_rs_compiler_output_write(ctx->output_owner, RUST_OUTPUT_TABLE, (const unsigned char *)ctx->phoneme_tab_list2[ix].name, N_PHONEME_TAB_NAME);
#else
		fputc(count+1, ctx->f_phtab);
		fputc(ctx->phoneme_tab_list2[ix].includes, ctx->f_phtab);
		fputc(0, ctx->f_phtab);
		fputc(0, ctx->f_phtab);

		fwrite(ctx->phoneme_tab_list2[ix].name, 1, N_PHONEME_TAB_NAME, ctx->f_phtab);
#endif

		for (j = 0; j < n; j++) {
			if (p[j].phflags & phLOCAL) {
				// this bit is set temporarily to incidate a local phoneme, declared in
				// in the current phoneme file
				p[j].phflags &= ~phLOCAL;
#ifdef USE_RUST_CORE
				espeak_rs_compiler_output_write(ctx->output_owner, RUST_OUTPUT_TABLE, (const unsigned char *)&p[j], sizeof(PHONEME_TAB));
#else
			fwrite(&p[j], sizeof(PHONEME_TAB), 1, ctx->f_phtab);
#endif
			}
		}
#ifdef USE_RUST_CORE
		espeak_rs_compiler_output_write(ctx->output_owner, RUST_OUTPUT_TABLE, (const unsigned char *)&p[n], sizeof(PHONEME_TAB)); // include the extra list-terminator entry
#else
		fwrite(&p[n], sizeof(PHONEME_TAB), 1, ctx->f_phtab); // include the extra list-terminator phoneme entry
#endif
		free(p);
	}
}

static void EndPhonemeTable(CompileContext *ctx)
{
	int ix;
	char buf[5];

	if (ctx->n_phoneme_tabs == 0)
		return;

	// check that all referenced phonemes have been declared
	for (ix = 0; ix < ctx->n_phcodes; ix++) {
		if (ctx->phoneme_tab2[ix].type == phINVALID) {
			error(ctx, "Phoneme [%s] not declared, referenced at line %d",
			      WordToString(buf, ctx->phoneme_tab2[ix].mnemonic), (int)(ctx->phoneme_tab2[ix].program));
			ctx->error_count++;
			ctx->phoneme_tab2[ix].type = 0; // prevent the error message repeating
		}
	}

	ctx->n_phcodes_list[ctx->n_phoneme_tabs-1] = ctx->n_phcodes;
}

static void StartPhonemeTable(CompileContext *ctx, const char *name)
{
	PHONEME_TAB *p;

	if (ctx->n_phoneme_tabs >= N_PHONEME_TABS-1) {
		error(ctx, "Too many phonemetables");
		return;
	}
	p = (PHONEME_TAB *)calloc(sizeof(PHONEME_TAB), N_PHONEME_TAB);

	if (p == NULL) {
		error(ctx, "Out of memory");
		return;
	}

	memset(&ctx->phoneme_tab_list2[ctx->n_phoneme_tabs], 0, sizeof(PHONEME_TAB_LIST));
	ctx->phoneme_tab_list2[ctx->n_phoneme_tabs].phoneme_tab_ptr = ctx->phoneme_tab2 = p;
	memset(ctx->phoneme_tab_list2[ctx->n_phoneme_tabs].name, 0, sizeof(ctx->phoneme_tab_list2[ctx->n_phoneme_tabs].name));
	strncpy0(ctx->phoneme_tab_list2[ctx->n_phoneme_tabs].name, name, N_PHONEME_TAB_NAME);
	ctx->n_phcodes = 1;
	ctx->phoneme_tab_list2[ctx->n_phoneme_tabs].includes = 0;

	if (ctx->n_phoneme_tabs > 0) {
		NextItem(ctx, tSTRING); // name of base phoneme table
		int ix;
		for (ix = 0; ix < ctx->n_phoneme_tabs; ix++) {
			if (strcmp(ctx->item_string, ctx->phoneme_tab_list2[ix].name) == 0) {
				ctx->phoneme_tab_list2[ctx->n_phoneme_tabs].includes = ix+1;

				// initialise the new phoneme table with the contents of this one
				memcpy(ctx->phoneme_tab2, ctx->phoneme_tab_list2[ix].phoneme_tab_ptr, sizeof(PHONEME_TAB)*N_PHONEME_TAB);
				ctx->n_phcodes = ctx->n_phcodes_list[ix];

				// clear "local phoneme" bit"
				int j;
				for (j = 0; j < ctx->n_phcodes; j++)
					ctx->phoneme_tab2[j].phflags &= ~phLOCAL;
				break;
			}
		}
		if (ix == ctx->n_phoneme_tabs && strcmp(ctx->item_string, "_") != 0)
			error(ctx, "Can't find base phonemetable '%s'", ctx->item_string);
	} else
		ReservePhCodes(ctx);

	ctx->n_phoneme_tabs++;
}
