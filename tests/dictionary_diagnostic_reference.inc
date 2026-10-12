/* Previous complete native reporters from 86680cc4:compiledict.c.
 * Fragment SHA256: a6b0b170f7e4789d8111d15f82d539035526b71a20d4b16d090da02811001a3a
 * SPDX-License-Identifier: GPL-3.0-or-later */
static void list_report(void *opaque, int line, int kind, int value, const char *first, const char *second)
{
	CompileContext *ctx = ((DictListIo *)opaque)->ctx;
	char bad[8];
	switch (kind) {
	case 1: fprintf(ctx->f_log, "%5d: Unknown keyword: %s\n", line, first); break;
	case 2: fprintf(ctx->f_log, "%5d: Missing '('\n", line); break;
	case 3: fprintf(ctx->f_log, "%5d: Need to compile dictionary again\n", line); return;
	case 4: fprintf(ctx->f_log, "%5d: Encoded phonemes exceed output buffer\n", line); break;
	case 5:
		bad[utf8_out(value, bad)] = 0;
		fprintf(ctx->f_log, "%5d: Bad phoneme [%s] (U+%x) in: %s  %s\n", line, bad, value, first, second); break;
	case 6: fprintf(ctx->f_log, "%5d: Dictionary line length would overflow the data buffer: %d\n", line, value); break;
	case 7: fprintf(ctx->f_log, "%5d: Two many parts in a multi-word entry: %d\n", line, value); break;
	case 8: fprintf(ctx->f_log, "%5d: Dictionary compiler resource or record bound exceeded\n", line); break;
	case 9: fprintf(ctx->f_log, "Compiling: '%s'\n", first); return;
	case 10: fprintf(ctx->f_log, "\t%d entries\n", value); return;
	default: fprintf(ctx->f_log, "%5d: Dictionary list I/O failed\n", line); break;
	}
	ctx->error_count++;
}

static void rule_report(void *opaque, int kind, int value, const char *text)
{
	CompileContext *ctx = opaque;
	switch (kind) {
	case 1: fprintf(ctx->f_log, "%5d: Expected 2 digits after 'L'\n", ctx->linenum); break;
	case 2: fprintf(ctx->f_log, "%5d: Letter group L%.2d not defined\n", ctx->linenum, value); break;
	case 3: fprintf(ctx->f_log, "%5d: $ command not recognized\n", ctx->linenum); break;
	case 4: fprintf(ctx->f_log, "%5d: Syntax error. Space after (, or negative score for previous rule\n", ctx->linenum); break;
	case 5: fprintf(ctx->f_log, "%5d: Syntax error\n", ctx->linenum); break;
	case 6: fprintf(ctx->f_log, "%5d: Encoded phonemes exceed output buffer\n", ctx->linenum); break;
	case 7: {
		char spelling[8];
		spelling[utf8_out(value, spelling)] = 0;
		/* text is the original compiler input, supplied by the bridge below. */
		fprintf(ctx->f_log, "%5d: Bad phoneme [%s] (U+%x) in: %s\n", ctx->linenum, spelling, value, text);
		break;
	}
	case 8: fprintf(ctx->f_log, "%5d: Wrong initial letters '%s' for group '%s'\n", ctx->linenum, text, ctx->group_name); break;
	case 9: fprintf(ctx->f_log, "%5d: bad condition number ?%d\n", ctx->linenum, value); break;
	case 10: fprintf(ctx->f_log, "%5d: Rule exceeds compiler buffer or numeric limit\n", ctx->linenum); break;
	case 11: fprintf(ctx->f_log, "%5d: Incomplete rule escape or letter group\n", ctx->linenum); break;
	}
	ctx->error_count++;
}

static void rule_file_report(void *opaque, int line, const uint8_t *group, int kind, int value, const char *text)
{
	RuleFileIo *io = opaque;
	CompileContext *ctx = io->compiler;
	ctx->linenum = line;
	memcpy(ctx->group_name, group, sizeof(ctx->group_name));
	if (kind <= 11) { rule_report(ctx, kind, value, text); return; }
	switch (kind) {
	case 12: fprintf(ctx->f_log, "%5d: Expected 2 digits after '.L'\n", line); break;
	case 13: fprintf(ctx->f_log, "%5d: lettergroup out of range (01-%.2d)\n", line, N_LETTER_GROUPS-1); break;
	case 14: fprintf(ctx->f_log, "%5d: lettergroup L%.2d is already defined\n", line, value); break;
	case 15: fprintf(ctx->f_log, "%5d: Group name longer than 2 bytes (UTF8)", line); break;
	case 16: fprintf(stderr, "\nExceeded limit of rules (%d) in group '%s'\n", value, ctx->group_name); break;
	case 17: fprintf(ctx->f_log, "%5d: Rule-file compiler resource limit or reentry\n", line); break;
	case 18: fputs(text, ctx->f_log); return;
	case 19: fprintf(ctx->f_log, "%5d: Rule-file input/output failed\n", line); break;
	}
	ctx->error_count++;
}

static void dictionary_log(void *opaque, int kind, const char *text)
{
	FILE *log = ((DictionaryIo *)opaque)->compiler->f_log;
	if (kind == 1) fprintf(log, "Using phonemetable: '%s'\n", text);
	else if (kind == 2) fprintf(log, "Compiling: '%s'\n", text);
	else fflush(log);
}

/* End frozen native dictionary reporters. */
