/* Frozen complete C bytecode controllers from 13eb13e9:compiledata.c.
 * Copyright (C) 2005-2014 Jonathan Duddington; 2013-2017 Reece H. Dunn.
 * SPDX-License-Identifier: GPL-3.0-or-later
 */
static int NextItemBrackets(CompileContext *ctx, int type, int control)
{
	// Expect a parameter inside parentheses
	// control: bit 0  0= need (
	//          bit 1  1= allow comma

	int value;

	if ((control & 1) == 0) {
		if (!NextItem(ctx, tOPENBRACKET))
			error(ctx, "Expected '('");
	}

	value = NextItem(ctx, type);
	if ((control & 2) && (ctx->item_terminator == ','))
		return value;

	if (ctx->item_terminator != ')')
		error(ctx, "Expected ')'");
	return value;
}

static int Range(int value, int divide, int min, int max)
{
	if (value < 0)
		value -= divide/2;
	else
		value += divide/2;
	value = value / divide;

	if (value > max)
		value = max;
	if (value < min)
		value = min;
	return value - min;
}

static int CompileVowelTransition(CompileContext *ctx, int which)
{
	// Compile a vowel transition
	int len = 0;
	int rms = 0;
	int f1 = 0;
	int f2 = 0;
	int f2_min = 0;
	int f2_max = 0;
	int f3_adj = 0;
	int f3_amp = 0;
	int flags = 0;
	int vcolour = 0;
	int x;
	int instn = i_VOWELIN;
	int word1;
	int word2;

	if (which == 1) {
		len = 50 / 2; // defaults for transition into vowel
		rms = 25 / 2;

		if (ctx->phoneme_out->type == phSTOP) {
			len = 42 / 2; // defaults for transition into vowel
			rms = 30 / 2;
		}
	} else if (which == 2) {
		instn = i_VOWELOUT;
		len = 36 / 2; // defaults for transition out of vowel
		rms = 16 / 2;
	}

	for (;;) {
		int key = NextItem(ctx, tKEYWORD);
		if (ctx->item_type != tTRANSITION) {
			UngetItem(ctx);
			break;
		}

		switch (key & 0xf)
		{
		case 1:
			len = Range(NextItem(ctx, tNUMBER), 2, 0, 63) & 0x3f;
			flags |= 1;
			break;
		case 2:
			rms = Range(NextItem(ctx, tNUMBER), 2, 0, 31) & 0x1f;
			flags |= 1;
			break;
		case 3:
			f1 = NextItem(ctx, tNUMBER);
			break;
		case 4:
			f2 = Range(NextItem(ctx, tNUMBER), 50, 0, 63) & 0x3f;
			f2_min = Range(NextItem(ctx, tSIGNEDNUMBER), 50, -15, 15) & 0x1f;
			f2_max = Range(NextItem(ctx, tSIGNEDNUMBER), 50, -15, 15) & 0x1f;
			if (f2_min > f2_max) {
				x = f2_min;
				f2_min = f2_max;
				f2_max = x;
			}
			break;
		case 5:
			f3_adj = Range(NextItem(ctx, tSIGNEDNUMBER), 50, -15, 15) & 0x1f;
			f3_amp = Range(NextItem(ctx, tNUMBER), 8, 0, 15) & 0x1f;
			break;
		case 6:
			flags |= 2; // break
			break;
		case 7:
			flags |= 4; // rate
			break;
		case 8:
			flags |= 8; // glstop
			break;
		case 9:
			flags |= 16; // lenadd
			break;
		case 10:
			flags |= 32;  // f4
			break;
		case 11:
			flags |= 64;  // pause
			break;
		case 12:
			vcolour = NextItem(ctx, tNUMBER);
			break;
		case 13:
			// set rms of 1st frame as fraction of rms of 2nd frame  (1/30ths)
			rms = (Range(NextItem(ctx, tNUMBER), 1, 0, 31) & 0x1f) | 0x20;
			flags |= 1;
			break;
		}
	}
	word1 = len + (rms << 6) + (flags << 12);
	word2 =  f2 + (f2_min << 6) + (f2_max << 11) + (f3_adj << 16) + (f3_amp << 21) + (f1 << 26) + (vcolour << 29);
	ctx->prog_out[0] = instn + ((word1 >> 16) & 0xff);
	ctx->prog_out[1] = word1;
	ctx->prog_out[2] = word2 >> 16;
	ctx->prog_out[3] = word2;
	ctx->prog_out += 4;

	return 0;
}


static void CompileToneSpec(CompileContext *ctx)
{
	int pitch1 = 0;
	int pitch2 = 0;
	int pitch_env = 0;
	int amp_env = 0;

	pitch1 = NextItemBrackets(ctx, tNUMBER, 2);
	pitch2 = NextItemBrackets(ctx, tNUMBER, 3);

	if (ctx->item_terminator == ',') {
		NextItemBrackets(ctx, tSTRING, 3);
		LoadDataFile(ctx, ctx->item_string, 0, &pitch_env);
	}

	if (ctx->item_terminator == ',') {
		NextItemBrackets(ctx, tSTRING, 1);
		LoadDataFile(ctx, ctx->item_string, 0, &amp_env);
	}

	if (pitch1 < pitch2) {
		ctx->phoneme_out->start_type = pitch1;
		ctx->phoneme_out->end_type = pitch2;
	} else {
		ctx->phoneme_out->start_type = pitch2;
		ctx->phoneme_out->end_type = pitch1;
	}

	if (pitch_env != 0) {
		*ctx->prog_out++ = i_PITCHENV + ((pitch_env >> 16) & 0xf);
		*ctx->prog_out++ = pitch_env;
	}
	if (amp_env != 0) {
		*ctx->prog_out++ = i_AMPENV + ((amp_env >> 16) & 0xf);
		*ctx->prog_out++ = amp_env;
	}
}



static void CompileSound(CompileContext *ctx, int keyword, int isvowel)
{
	int addr = 0;
	int value = 0;
	char path[N_ITEM_STRING];
	static const int sound_instns[] = { i_FMT, i_WAV, i_VWLSTART, i_VWLENDING, i_WAVADD };

	NextItemBrackets(ctx, tSTRING, 2);
	strcpy(path, ctx->item_string);
	if (ctx->item_terminator == ',') {
		if ((keyword == kVOWELSTART) || (keyword == kVOWELENDING)) {
			value = NextItemBrackets(ctx, tSIGNEDNUMBER, 1);
			if (value > 127) {
				value = 127;
				error(ctx, "Parameter > 127");
			}
			if (value < -128) {
				value = -128;
				error(ctx, "Parameter < -128");
			}
		} else {
			value = NextItemBrackets(ctx, tNUMBER, 1);
			if (value > 255) {
				value = 255;
				error(ctx, "Parameter > 255");
			}
		}
	}
	LoadDataFile(ctx, path, isvowel, &addr);
	addr = addr / 4; // addr is words not bytes

	*ctx->prog_out++ = sound_instns[keyword-kFMT] + ((value & 0xff) << 4) + ((addr >> 16) & 0xf);
	*ctx->prog_out++ = addr & 0xffff;
}

/*
   Condition
   bits 14,15   1
   bit 13       1 = AND, 0 = OR
   bit 12       spare
   bit 8-11
   =0-3       p,t,n,n2   data=phoneme code
   =4-7       p,t,n,n2   data=(bits5-7: phtype, place, property, special) (bits0-4: data)
   =8         data = stress bitmap
   =9         special tests
 */
static int CompileIf(CompileContext *ctx, int elif)
{
	bool finish = false;
	int word = 0;
	int data;
	int bitmap;
	int brackets;
	unsigned short *prog_last_if = NULL;

	ctx->then_count = 2;
	ctx->after_if = true;

	while (!finish) {
		bool not_flag = false;
		int word2 = 0;
		if (ctx->prog_out >= ctx->prog_out_max) {
			error(ctx, "Phoneme program too large");
			return 0;
		}

		int key;
		if ((key = NextItem(ctx, tCONDITION)) < 0)
			error(ctx, "Expected a condition, not '%s'", ctx->item_string);

		if ((ctx->item_type == 0) && (key == k_NOT)) {
			not_flag = true;
			if ((key = NextItem(ctx, tCONDITION)) < 0)
				error(ctx, "Expected a condition, not '%s'", ctx->item_string);
		}

		if (ctx->item_type == tWHICH_PHONEME) {
			// prevPh(), thisPh(), nextPh(), next2Ph() etc
			if (key >= 6) {
				// put the 'which' code in the next instruction
				word2 = key;
				key = 6;
			}
			key = key << 8;

			data = NextItemBrackets(ctx, tPROPERTIES, 0);
			if (data >= 0)
				word = key + data + 0x700;
			else {
				data = LookupPhoneme(ctx, ctx->item_string, 2);
				word = key + data;
			}
		} else if (ctx->item_type == tTEST) {
			if (key == kTHISSTRESS) {
				bitmap = 0;
				brackets = 2;
				do {
					data = NextItemBrackets(ctx, tNUMBER, brackets);
					if (data > 7)
						error(ctx, "Expected list of stress levels");
					bitmap |= (1 << data);

					brackets = 3;
				} while (ctx->item_terminator == ',');
				word = i_StressLevel | bitmap;
			} else
				word = key;
		} else {
			error(ctx, "Unexpected keyword '%s'", ctx->item_string);

			if ((strcmp(ctx->item_string, "phoneme") == 0) || (strcmp(ctx->item_string, "endphoneme") == 0))
				return -1;
		}

		// output the word
		prog_last_if = ctx->prog_out;
		*ctx->prog_out++ = word | i_CONDITION;

		if (word2 != 0)
			*ctx->prog_out++ = word2;
		if (not_flag)
			*ctx->prog_out++ = i_NOT;

		// expect AND, OR, THEN
		switch (NextItem(ctx, tCONDITION))
		{
		case k_AND:
			break;
		case k_OR:
			if (prog_last_if != NULL)
				*prog_last_if |=  i_OR;
			break;
		case k_THEN:
			finish = true;
			break;
		default:
			error(ctx, "Expected AND, OR, THEN");
			break;
		}
	}

	if (elif == 0) {
		ctx->if_level++;
		ctx->if_stack[ctx->if_level].p_else = NULL;
	}

	ctx->if_stack[ctx->if_level].returned = false;
	ctx->if_stack[ctx->if_level].p_then = ctx->prog_out;
	*ctx->prog_out++ = i_JUMP_FALSE;

	return 0;
}

static void FillThen(CompileContext *ctx, int add)
{
	unsigned short *p;

	p = ctx->if_stack[ctx->if_level].p_then;
	if (p != NULL) {
		int offset = ctx->prog_out - p + add;

		if ((ctx->then_count == 1) && (ctx->if_level == 1)) {
			// The THEN part only contains one statement, we can remove the THEN jump
			// and the interpreter will implicitly skip the statement.
			while (p < ctx->prog_out) {
				p[0] = p[1];
				p++;
			}
			ctx->prog_out--;
		} else {
			if (offset > MAX_JUMP)
				error(ctx, "IF block is too long");
			*p = i_JUMP_FALSE + offset;
		}
		ctx->if_stack[ctx->if_level].p_then = NULL;
	}

	ctx->then_count = 0;
}

static int CompileElse(CompileContext *ctx)
{
	unsigned short *ref;

	if (ctx->if_level < 1) {
		error(ctx, "ELSE not expected");
		return 0;
	}

	if (ctx->if_stack[ctx->if_level].returned == false)
		FillThen(ctx, 1);
	else
		FillThen(ctx, 0);

	if (ctx->if_stack[ctx->if_level].returned == false) {
		ref = ctx->prog_out;
		*ctx->prog_out++ = 0;

		unsigned short *p;
		if ((p = ctx->if_stack[ctx->if_level].p_else) != NULL)
			*ref = ref - p; // backwards offset to the previous else
		ctx->if_stack[ctx->if_level].p_else = ref;
	}

	return 0;
}

static int CompileElif(CompileContext *ctx)
{
	if (ctx->if_level < 1) {
		error(ctx, "ELIF not expected");
		return 0;
	}

	CompileElse(ctx);
	CompileIf(ctx, 1);
	return 0;
}

static int CompileEndif(CompileContext *ctx)
{
	unsigned short *p;

	if (ctx->if_level < 1) {
		error(ctx, "ENDIF not expected");
		return 0;
	}

	FillThen(ctx, 0);

	if ((p = ctx->if_stack[ctx->if_level].p_else) != NULL) {
		int chain;
		do {
			chain = *p; // a chain of previous else links

			int offset = ctx->prog_out - p;
			if (offset > MAX_JUMP)
				error(ctx, "IF block is too long");
			*p = i_JUMP + offset;

			p -= chain;
		} while (chain > 0);
	}

	ctx->if_level--;
	return 0;
}

static int CompileSwitch(CompileContext *ctx, int type)
{
	// Type 0:  EndSwitch
	//      1:  SwitchPrevVowelType
	//      2:  SwitchNextVowelType

	if (type == 0) {
		// check the instructions in the Switch
		return 0;
	}

	if (type == 1)
		*ctx->prog_out++ = i_SWITCH_PREVVOWEL+6;
	if (type == 2)
		*ctx->prog_out++ = i_SWITCH_NEXTVOWEL+6;
	return 0;
}
