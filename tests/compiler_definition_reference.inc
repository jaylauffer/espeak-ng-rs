/* Frozen complete definition controllers from 13eb13e9:compiledata.c.
 * Copyright (C) 2005-2014 Jonathan Duddington; 2013-2017 Reece H. Dunn.
 * SPDX-License-Identifier: GPL-3.0-or-later
 */
static int NextItemMax(CompileContext *ctx, int max)
{
	// Get a number, but restrict value to max
	int value;

	value = NextItem(ctx, tNUMBER);
	if (value > max) {
		error(ctx, "Value %d is greater than maximum %d", value, max);
		value = max;
	}
	return value;
}

static void CallPhoneme(CompileContext *ctx)
{
	PHONEME_TAB *ph;
	int ix;
	int addr = 0;

	NextItem(ctx, tSTRING);

	// first look for a procedure name
	for (ix = 0; ix < ctx->n_procs; ix++) {
		if (strcmp(ctx->proc_names[ix], ctx->item_string) == 0) {
			addr = ctx->proc_addr[ix];
			break;
		}
	}
	if (ix == ctx->n_procs) {
		// procedure not found, try a phoneme name
		if ((ph = FindPhoneme(ctx, ctx->item_string)) == NULL)
			return;
		addr = ph->program;

		if (ctx->phoneme_out->type == phINVALID) {
			// Phoneme type has not been set. Copy it from the called phoneme
			ctx->phoneme_out->type = ph->type;
			ctx->phoneme_out->start_type = ph->start_type;
			ctx->phoneme_out->end_type = ph->end_type;
			ctx->phoneme_out->std_length = ph->std_length;
			ctx->phoneme_out->length_mod = ph->length_mod;

			ctx->phoneme_flags = ph->phflags & ~phARTICULATION;
		}
	}

	*ctx->prog_out++ = i_CALLPH + (addr >> 16);
	*ctx->prog_out++ = addr;
}

static void DecThenCount(CompileContext *ctx)
{
	if (ctx->then_count > 0)
		ctx->then_count--;
}

static int CompilePhoneme(CompileContext *ctx, int compile_phoneme)
{
	int endphoneme = 0;
	int value;
	int phcode = 0;
	int flags;
	int ix;
	int start;
	int count;
	int c;
	char *p;
	int vowel_length_factor = 100; // for testing
	char number_buf[12];
	char ipa_buf[N_ITEM_STRING+1];
	PHONEME_TAB phoneme_out2;
	PHONEME_PROG_LOG phoneme_prog_log;

	ctx->prog_out = ctx->prog_buf;
	ctx->prog_out_max = &ctx->prog_buf[MAX_PROG_BUF-1];
	ctx->if_level = 0;
	ctx->if_stack[0].returned = false;
	ctx->after_if = false;
	ctx->phoneme_flags = 0;

	NextItem(ctx, tSTRING);
	if (compile_phoneme) {
		phcode = LookupPhoneme(ctx, ctx->item_string, 1); // declare phoneme if not already there
		if (phcode == -1) return 0;
		ctx->phoneme_out = &ctx->phoneme_tab2[phcode];
	} else {
		// declare a procedure
		if (ctx->n_procs >= N_PROCS) {
			error(ctx, "Too many procedures");
			return 0;
		}
		strcpy(ctx->proc_names[ctx->n_procs], ctx->item_string);
		ctx->phoneme_out = &phoneme_out2;
		sprintf(number_buf, "%.3dP", ctx->n_procs);
		ctx->phoneme_out->mnemonic = StringToWord(number_buf);
	}

	ctx->phoneme_out->code = phcode;
	ctx->phoneme_out->program = 0;
	ctx->phoneme_out->type = phINVALID;
	ctx->phoneme_out->std_length = 0;
	ctx->phoneme_out->start_type = 0;
	ctx->phoneme_out->end_type = 0;
	ctx->phoneme_out->length_mod = 0;
	ctx->phoneme_out->phflags = 0;

	while (!endphoneme && !SourceEof(ctx)) {
		int keyword;
		if ((keyword = NextItem(ctx, tKEYWORD)) < 0) {
			if (keyword == -2) {
				error(ctx, "Missing 'endphoneme' before end-of-file"); // end of file
				break;
			}

			phoneme_feature_t feature = phoneme_feature_from_string(ctx->item_string);
			espeak_ng_STATUS status = phoneme_add_feature(ctx->phoneme_out, feature);
			if (status == ENS_OK)
				continue;
			error_from_status(ctx, status, ctx->item_string);
			continue;
		}

		switch (ctx->item_type)
		{
		case tPHONEME_TYPE:
			if (ctx->phoneme_out->type != phINVALID) {
				if (ctx->phoneme_out->type == phFRICATIVE && keyword == phLIQUID)
					; // apr liquid => ok
				else
					error(ctx, "More than one phoneme type: %s", ctx->item_string);
			}
			ctx->phoneme_out->type = keyword;
			break;
		case tPHONEME_FLAG:
			ctx->phoneme_flags |= keyword;
			break;
		case tINSTRN1:
			// instruction group 0, with 8 bit operands which set data in PHONEME_DATA
			switch (keyword)
			{
			case i_CHANGE_PHONEME:
			case i_APPEND_PHONEME:
			case i_APPEND_IFNEXTVOWEL:
			case i_INSERT_PHONEME:
			case i_REPLACE_NEXT_PHONEME:
			case i_VOICING_SWITCH:
			case i_CHANGE_IF | STRESS_IS_DIMINISHED:
			case i_CHANGE_IF | STRESS_IS_UNSTRESSED:
			case i_CHANGE_IF | STRESS_IS_NOT_STRESSED:
			case i_CHANGE_IF | STRESS_IS_SECONDARY:
			case i_CHANGE_IF | STRESS_IS_PRIMARY:
				value = NextItemBrackets(ctx, tPHONEMEMNEM, 0);
				*ctx->prog_out++ = (keyword << 8) + value;
				DecThenCount(ctx);
				break;
			case i_PAUSE_BEFORE:
				value = NextItemMax(ctx, 255);
				*ctx->prog_out++ = (i_PAUSE_BEFORE << 8) + value;
				DecThenCount(ctx);
				break;
			case i_PAUSE_AFTER:
				value = NextItemMax(ctx, 255);
				*ctx->prog_out++ = (i_PAUSE_AFTER << 8) + value;
				DecThenCount(ctx);
				break;
			case i_SET_LENGTH:
				value = NextItemMax(ctx, 511);
				if (ctx->phoneme_out->type == phVOWEL)
					value = (value * vowel_length_factor)/100;

				if (ctx->after_if == false)
					ctx->phoneme_out->std_length = value/2;
				else {
					*ctx->prog_out++ = (i_SET_LENGTH << 8) + value/2;
					DecThenCount(ctx);
				}
				break;
			case i_ADD_LENGTH:
				value = NextItem(ctx, tSIGNEDNUMBER) / 2;
				*ctx->prog_out++ = (i_ADD_LENGTH << 8) + (value & 0xff);
				DecThenCount(ctx);
				break;
			case i_LENGTH_MOD:
				value = NextItem(ctx, tNUMBER);
				assert(value < LENGTH_MOD_LIMIT);
				ctx->phoneme_out->length_mod = value;
				break;
			case i_IPA_NAME:
				NextItem(ctx, tSTRING);

				if (strcmp(ctx->item_string, "NULL") == 0)
					strcpy(ctx->item_string, " ");

				// copy the string, recognize characters in the form U+9999
				flags = 0;
				count = 0;
				ix = 1;

				for (p = ctx->item_string; *p != 0;) {
					p += utf8_in(&c, p);

					if ((c == '|') && (count > 0)) {
						// '|' means don't allow a tie or joiner before this letter
						flags |= (1 << (count -1));
					} else if ((c == 'U') && (p[0] == '+')) {
						int j;
						// U+9999
						p++;
						memcpy(number_buf, p, 4); // U+ should be followed by 4 hex digits
						number_buf[4] = 0;
						c = '#';
						sscanf(number_buf, "%x", (unsigned int *)&c);

						// move past the 4 hexdecimal digits
						for (j = 0; j < 4; j++) {
							if (!isalnum(*p))
								break;
							p++;
						}
						ix += utf8_out(c, &ipa_buf[ix]);
						count++;
					} else {
						ix += utf8_out(c, &ipa_buf[ix]);
						count++;
					}
				}
				ipa_buf[0] = flags;
				ipa_buf[ix] = 0;

				start = 1;
				if (flags != 0)
					start = 0; // only include the flags byte if bits are set
				value = strlen(&ipa_buf[start]); // number of UTF-8 bytes

				*ctx->prog_out++ = (i_IPA_NAME << 8) + value;
				for (ix = 0; ix < value; ix += 2)
					*ctx->prog_out++ = (ipa_buf[ix+start] << 8) + (ipa_buf[ix+start+1] & 0xff);
				DecThenCount(ctx);
				break;
			}
			break;
		case tSTATEMENT:
			switch (keyword)
			{
			case kIMPORT_PH:
				ImportPhoneme(ctx);
				ctx->phoneme_flags = ctx->phoneme_out->phflags;
				break;
			case kSTARTTYPE:
				phcode = NextItem(ctx, tPHONEMEMNEM);
				if (phcode == -1)
					phcode = LookupPhoneme(ctx, ctx->item_string, 1);
				ctx->phoneme_out->start_type = phcode;
				if (ctx->phoneme_out->type == phINVALID)
					error(ctx, "a phoneme type or manner of articulation must be specified before starttype");
				break;
			case kENDTYPE:
				phcode = NextItem(ctx, tPHONEMEMNEM);
				if (phcode == -1)
					phcode = LookupPhoneme(ctx, ctx->item_string, 1);
				if (ctx->phoneme_out->type == phINVALID)
					error(ctx, "a phoneme type or manner of articulation must be specified before endtype");
				else if (ctx->phoneme_out->type == phVOWEL)
					ctx->phoneme_out->end_type = phcode;
				else if (phcode != ctx->phoneme_out->start_type)
					error(ctx, "endtype must equal starttype for consonants");
				break;
			case kVOICINGSWITCH:
				phcode = NextItem(ctx, tPHONEMEMNEM);
				if (phcode == -1)
					phcode = LookupPhoneme(ctx, ctx->item_string, 1);
				if (ctx->phoneme_out->type == phVOWEL)
					error(ctx, "voicingswitch cannot be used on vowels");
				else
					ctx->phoneme_out->end_type = phcode; // use end_type field for consonants as voicing_switch
				break;
			case kSTRESSTYPE:
				value = NextItem(ctx, tNUMBER);
				ctx->phoneme_out->std_length = value;
				if (ctx->prog_out > ctx->prog_buf) {
					error(ctx, "stress phonemes can't contain program instructions");
					ctx->prog_out = ctx->prog_buf;
				}
				break;
			case kIF:
				endphoneme = CompileIf(ctx, 0);
				break;
			case kELSE:
				endphoneme = CompileElse(ctx);
				break;
			case kELIF:
				endphoneme = CompileElif(ctx);
				break;
			case kENDIF:
				endphoneme = CompileEndif(ctx);
				break;
			case kENDSWITCH:
				break;
			case kSWITCH_PREVVOWEL:
				endphoneme = CompileSwitch(ctx, 1);
				break;
			case kSWITCH_NEXTVOWEL:
				endphoneme = CompileSwitch(ctx, 2);
				break;
			case kCALLPH:
				CallPhoneme(ctx);
				DecThenCount(ctx);
				break;
			case kFMT:
				ctx->if_stack[ctx->if_level].returned = true;
				DecThenCount(ctx);
				if (ctx->phoneme_out->type == phVOWEL)
					CompileSound(ctx, keyword, 1);
				else
					CompileSound(ctx, keyword, 0);
				break;
			case kWAV:
				ctx->if_stack[ctx->if_level].returned = true;
				// fallthrough:
			case kVOWELSTART:
			case kVOWELENDING:
			case kANDWAV:
				DecThenCount(ctx);
				CompileSound(ctx, keyword, 0);
				break;
			case kVOWELIN:
				DecThenCount(ctx);
				endphoneme = CompileVowelTransition(ctx, 1);
				break;
			case kVOWELOUT:
				DecThenCount(ctx);
				endphoneme = CompileVowelTransition(ctx, 2);
				break;
			case kTONESPEC:
				DecThenCount(ctx);
				CompileToneSpec(ctx);
				break;
			case kCONTINUE:
				*ctx->prog_out++ = INSTN_CONTINUE;
				DecThenCount(ctx);
				break;
			case kRETURN:
				*ctx->prog_out++ = INSTN_RETURN;
				DecThenCount(ctx);
				break;
			case kINCLUDE:
			case kPHONEMETABLE:
				error(ctx, "Missing 'endphoneme' before '%s'", ctx->item_string);  // drop through to endphoneme
				// fallthrough:
			case kENDPHONEME:
			case kENDPROCEDURE:
				endphoneme = 1;
				if (ctx->if_level > 0)
					error(ctx, "Missing ENDIF");
				if ((ctx->prog_out > ctx->prog_buf) && (ctx->if_stack[0].returned == false))
					*ctx->prog_out++ = INSTN_RETURN;
				break;
			}
			break;
		}
	}

	if (endphoneme != 1)
		error(ctx, "'endphoneme' not expected here");

	if (compile_phoneme) {
		if (ctx->phoneme_out->type == phINVALID) {
			error(ctx, "Phoneme type is missing");
			ctx->phoneme_out->type = 0;
		}
		ctx->phoneme_out->phflags |= ctx->phoneme_flags;

		if (ctx->phoneme_out->phflags & phVOICED) {
			if (ctx->phoneme_out->type == phSTOP)
				ctx->phoneme_out->type = phVSTOP;
			else if (ctx->phoneme_out->type == phFRICATIVE)
				ctx->phoneme_out->type = phVFRICATIVE;
		}

		if (ctx->phoneme_out->std_length == 0) {
			if (ctx->phoneme_out->type == phVOWEL)
				ctx->phoneme_out->std_length = 180/2; // default length for vowel
		}

		ctx->phoneme_out->phflags |= phLOCAL; // declared in this phoneme table

		if (ctx->phoneme_out->type == phDELETED)
			ctx->phoneme_out->mnemonic = 0x01; // will not be recognised
	}

	if (ctx->prog_out > ctx->prog_buf) {
		// write out the program for this phoneme
#ifdef USE_RUST_CORE
		espeak_rs_compiler_output_flush(ctx->output_owner, RUST_OUTPUT_INDEX);
		int64_t position = espeak_rs_compiler_output_position(ctx->output_owner, RUST_OUTPUT_INDEX);
		ctx->phoneme_out->program = position / sizeof(unsigned short);
		if (ctx->output_mask & (1u << RUST_OUTPUT_LOG)) {
			phoneme_prog_log.addr = ctx->phoneme_out->program;
			phoneme_prog_log.length = ctx->prog_out - ctx->prog_buf;
			espeak_rs_compiler_output_write(ctx->output_owner, RUST_OUTPUT_LOG, (const unsigned char *)&phoneme_prog_log, sizeof(phoneme_prog_log));
		}
		if (compile_phoneme == 0) ctx->proc_addr[ctx->n_procs++] = position / sizeof(unsigned short);
		espeak_rs_compiler_output_write(ctx->output_owner, RUST_OUTPUT_INDEX, (const unsigned char *)ctx->prog_buf, sizeof(unsigned short) * (ctx->prog_out - ctx->prog_buf));
#else
		fflush(ctx->f_phindex);
		ctx->phoneme_out->program = ftell(ctx->f_phindex) / sizeof(unsigned short);

		if (ctx->f_prog_log != NULL) {
			phoneme_prog_log.addr = ctx->phoneme_out->program;
			phoneme_prog_log.length = ctx->prog_out - ctx->prog_buf;
			fwrite(&phoneme_prog_log, 1, sizeof(phoneme_prog_log), ctx->f_prog_log);
		}

		if (compile_phoneme == 0)
			ctx->proc_addr[ctx->n_procs++] =  ftell(ctx->f_phindex) / sizeof(unsigned short);
		fwrite(ctx->prog_buf, sizeof(unsigned short), ctx->prog_out - ctx->prog_buf, ctx->f_phindex);
#endif
	}

	return 0;
}
