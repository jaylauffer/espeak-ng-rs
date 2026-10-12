/* Complete original source-file controller from c8393cc0.
 * Copyright (C) 2005-2014 Jonathan Duddington; 2013-2017 Reece H. Dunn.
 * SPDX-License-Identifier: GPL-3.0-or-later */
static void CompilePhonemeFiles(CompileContext *ctx)
{
	CompilerInput f;
	char buf[N_PATH_BUF];

	ctx->linenum = 1;

	ctx->count_references = 0;
	ctx->duplicate_references = 0;
	ctx->count_frames = 0;
	#ifndef USE_RUST_CORE
	ctx->n_procs = 0;
	#endif

	for (;;) {
		if (SourceEof(ctx)) {
			// end of file, go back to previous from, from which this was included

			if (ctx->stack_ix == 0)
				break; // end of top level, finished
			SourceClose(ctx);
			ctx->f_in = ctx->stack[--ctx->stack_ix].file;
			strcpy(ctx->current_fname, ctx->stack[ctx->stack_ix].fname);
			ctx->linenum = ctx->stack[ctx->stack_ix].linenum;
		}

		int item = NextItem(ctx, tKEYWORD);

		switch (item)
		{
		case kUTF8_BOM:
			break; // ignore bytes 0xef 0xbb 0xbf
		case kINCLUDE:
			NextItem(ctx, tSTRING);
			snprintf(buf, sizeof(buf), "%s/%s", ctx->phsrc, ctx->item_string);

			if ((ctx->stack_ix < N_STACK) && (f = SourceOpen(ctx, buf, ctx->stack_ix + 1, 0)) != 0) {
				ctx->stack[ctx->stack_ix].linenum = ctx->linenum;
				strcpy(ctx->stack[ctx->stack_ix].fname, ctx->current_fname);
				ctx->stack[ctx->stack_ix++].file = ctx->f_in;

				ctx->f_in = f;
				strncpy0(ctx->current_fname, ctx->item_string, sizeof(ctx->current_fname));
				ctx->linenum = 1;
			} else
				error(ctx, "Missing file: %s", ctx->item_string);
			break;
		case kPHONEMETABLE:
			EndPhonemeTable(ctx);
			NextItem(ctx, tSTRING); // name of the new phoneme table
			StartPhonemeTable(ctx, ctx->item_string);
			break;
		case kPHONEMESTART:
			if (ctx->n_phoneme_tabs == 0) {
				error(ctx, "phonemetable is missing");
				return;
			}
			CompilePhoneme(ctx, 1);
			break;
		case kPROCEDURE:
			CompilePhoneme(ctx, 0);
			break;
		default:
			if (!SourceEof(ctx))
				error(ctx, "Keyword 'phoneme' expected");
			break;
		}
	}
#ifdef USE_RUST_CORE
	TableCall(ctx, RUST_TABLE_TERMINATE, NULL, 0);
#else
	memset(&ctx->phoneme_tab2[ctx->n_phcodes+1], 0, sizeof(ctx->phoneme_tab2[ctx->n_phcodes+1]));
	ctx->phoneme_tab2[ctx->n_phcodes+1].mnemonic = 0; // terminator
#endif
}
