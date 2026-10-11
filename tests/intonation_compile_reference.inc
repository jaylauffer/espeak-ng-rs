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

/* Frozen 82f273a3 intonation tables/compiler; test-only. */
static const char *preset_tune_names[] = {
	"s1", "c1", "q1", "e1", NULL
};

static const TUNE default_tune = {
	{ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 },
	{ 0, 0, 0, 0 },
	{ 0, 40, 24, 8, 0, 0, 0, 0 },
	46, 57, PITCHfall, 16, 0, 0,
	255, 78, 50, 255,
	3, 5,
	{ -7, -7, -7 }, { -7, -7, -7 },
	PITCHfall, 64, 8,
	PITCHfall, 70, 18, 24, 12,
	PITCHfall, 70, 18, 24, 12, 0,
	{ 0, 0, 0, 0, 0, 0, 0, 0 }, 0
};

#define N_TUNE_NAMES  100

static const MNEM_TAB envelope_names[] = {
	{ "fall", 0 },
	{ "rise", 2 },
	{ "fall-rise", 4 },
	{ "fall-rise2", 6 },
	{ "rise-fall",  8 },
	{ "fall-rise3", 10 },
	{ "fall-rise4", 12 },
	{ "fall2", 14 },
	{ "rise2", 16 },
	{ "rise-fall-rise", 18 },
	{ NULL, -1 }
};

static int LookupEnvelopeName(const char *name)
{
	return LookupMnem(envelope_names, name);
}

espeak_ng_STATUS
espeak_ng_CompileIntonationPath(const char *source_path,
                                const char *destination_path,
                                FILE *log,
                                espeak_ng_ERROR_CONTEXT *context
                                )
{
	if (!log) log = stderr;
	if (!source_path) source_path = path_home;
	if (!destination_path) destination_path = path_home;

	int ix;
	char *p;
	char c;
	int n_tune_names = 0;
	bool done_split = false;
	bool done_onset = false;
	bool done_last = false;
	int n_preset_tunes = 0;
	int found = 0;
	int tune_number = 0;
	FILE *f_out;
	TUNE *tune_data;
	TUNE new_tune;

	char name[12];
	char tune_names[N_TUNE_NAMES][12];
	char buf[N_PATH_BUF];

	CompileContext *ctx = calloc(1, sizeof(CompileContext));
	if (!ctx) return ENOMEM;

	ctx->error_count = 0;
	ctx->f_errors = log;

	snprintf(buf, sizeof(buf), "%s/../phsource/intonation.txt", source_path);
	if ((ctx->f_in = fopen(buf, "r")) == NULL) {
		snprintf(buf, sizeof(buf), "%s/../phsource/intonation", source_path);
		if ((ctx->f_in = fopen(buf, "r")) == NULL) {
			int error = errno;
			fclose(ctx->f_errors);
			clean_context(ctx);
			return create_file_error_context(context, error, buf);
		}
	}

	for (ix = 0; preset_tune_names[ix] != NULL; ix++)
		strcpy(tune_names[ix], preset_tune_names[ix]);
	n_tune_names = ix;
	n_preset_tunes = ix;

	// make a list of the tune names
	while (!feof(ctx->f_in)) {
		if (fgets(buf, sizeof(buf), ctx->f_in) == NULL)
			break;

		if ((memcmp(buf, "tune", 4) == 0) && isspace(buf[4])) {
			p = &buf[5];
			while (isspace(*p)) p++;

			ix = 0;
			while ((ix < (int)(sizeof(name) - 1)) && !isspace(*p))
				name[ix++] = *p++;
			name[ix] = 0;

			found = 0;
			for (ix = 0; ix < n_tune_names; ix++) {
				if (strcmp(name, tune_names[ix]) == 0) {
					found = 1;
					break;
				}
			}

			if (found == 0) {
				strncpy0(tune_names[n_tune_names++], name, sizeof(name));

				if (n_tune_names >= N_TUNE_NAMES)
					break;
			}
		}
	}
	rewind(ctx->f_in);
	ctx->linenum = 1;

	tune_data = (n_tune_names == 0) ? NULL : (TUNE *)calloc(n_tune_names, sizeof(TUNE));
	if (tune_data == NULL) {
		fclose(ctx->f_in);
		fclose(ctx->f_errors);
		clean_context(ctx);
		return ENOMEM;
	}

	snprintf(buf, sizeof(buf), "%s/intonations", destination_path);
	f_out = fopen(buf, "wb");
	if (f_out == NULL) {
		int error = errno;
		fclose(ctx->f_in);
		fclose(ctx->f_errors);
		free(tune_data);
		clean_context(ctx);
		return create_file_error_context(context, error, buf);
	}

	while (!feof(ctx->f_in)) {
		int keyword = NextItem(ctx, tINTONATION);

		switch (keyword)
		{
		case kTUNE:
			done_split = false;

			memcpy(&new_tune, &default_tune, sizeof(TUNE));
			NextItem(ctx, tSTRING);
			strncpy0(new_tune.name, ctx->item_string, sizeof(new_tune.name));

			found = 0;
			tune_number = 0;
			for (ix = 0; ix < n_tune_names; ix++) {
				if (strcmp(new_tune.name, tune_names[ix]) == 0) {
					found = 1;
					tune_number = ix;

					if (tune_data[ix].name[0] != 0)
						found = 2;
					break;
				}
			}
			if (found == 2)
				error(ctx, "Duplicate tune name: '%s'", new_tune.name);
			if (found == 0)
				error(ctx, "Bad tune name: '%s;", new_tune.name);
			break;
		case kENDTUNE:
			if (!found) continue;
			if (done_onset == false) {
				new_tune.unstr_start[0] = new_tune.unstr_start[1];
				new_tune.unstr_end[0] = new_tune.unstr_end[1];
			}
			if (done_last == false) {
				new_tune.unstr_start[2] = new_tune.unstr_start[1];
				new_tune.unstr_end[2] = new_tune.unstr_end[1];
			}
			memcpy(&tune_data[tune_number], &new_tune, sizeof(TUNE));
			break;
		case kTUNE_PREHEAD:
			new_tune.prehead_start = NextItem(ctx, tNUMBER);
			new_tune.prehead_end = NextItem(ctx, tNUMBER);
			break;
		case kTUNE_ONSET:
			new_tune.onset = NextItem(ctx, tNUMBER);
			new_tune.unstr_start[0] = NextItem(ctx, tSIGNEDNUMBER);
			new_tune.unstr_end[0] = NextItem(ctx, tSIGNEDNUMBER);
			done_onset = true;
			break;
		case kTUNE_HEADLAST:
			new_tune.head_last = NextItem(ctx, tNUMBER);
			new_tune.unstr_start[2] = NextItem(ctx, tSIGNEDNUMBER);
			new_tune.unstr_end[2] = NextItem(ctx, tSIGNEDNUMBER);
			done_last = true;
			break;
		case kTUNE_HEADENV:
			NextItem(ctx, tSTRING);
			if ((ix = LookupEnvelopeName(ctx->item_string)) < 0)
				error(ctx, "Bad envelope name: '%s'", ctx->item_string);
			else
				new_tune.stressed_env = ix;
			new_tune.stressed_drop = NextItem(ctx, tNUMBER);
			break;
		case kTUNE_HEAD:
			new_tune.head_max_steps = NextItem(ctx, tNUMBER);
			new_tune.head_start = NextItem(ctx, tNUMBER);
			new_tune.head_end = NextItem(ctx, tNUMBER);
			new_tune.unstr_start[1] = NextItem(ctx, tSIGNEDNUMBER);
			new_tune.unstr_end[1] = NextItem(ctx, tSIGNEDNUMBER);
			break;
		case kTUNE_HEADEXTEND:
			// up to 8 numbers
			for (ix = 0; ix < (int)(sizeof(new_tune.head_extend)); ix++) {
				if (!isdigit(c = CheckNextChar(ctx)) && (c != '-'))
					break;

				new_tune.head_extend[ix] = (NextItem(ctx, tSIGNEDNUMBER) * 64) / 100; // convert from percentage to 64ths
			}
			new_tune.n_head_extend = ix; // number of values
			break;
		case kTUNE_NUCLEUS0:
			NextItem(ctx, tSTRING);
			if ((ix = LookupEnvelopeName(ctx->item_string)) < 0) {
				error(ctx, "Bad envelope name: '%s'", ctx->item_string);
				break;
			}
			new_tune.nucleus0_env = ix;
			new_tune.nucleus0_max = NextItem(ctx, tNUMBER);
			new_tune.nucleus0_min = NextItem(ctx, tNUMBER);
			break;
		case kTUNE_NUCLEUS1:
			NextItem(ctx, tSTRING);
			if ((ix = LookupEnvelopeName(ctx->item_string)) < 0) {
				error(ctx, "Bad envelope name: '%s'", ctx->item_string);
				break;
			}
			new_tune.nucleus1_env = ix;
			new_tune.nucleus1_max = NextItem(ctx, tNUMBER);
			new_tune.nucleus1_min = NextItem(ctx, tNUMBER);
			new_tune.tail_start = NextItem(ctx, tNUMBER);
			new_tune.tail_end = NextItem(ctx, tNUMBER);

			if (!done_split) {
				// also this as the default setting for 'split'
				new_tune.split_nucleus_env = ix;
				new_tune.split_nucleus_max = new_tune.nucleus1_max;
				new_tune.split_nucleus_min = new_tune.nucleus1_min;
				new_tune.split_tail_start = new_tune.tail_start;
				new_tune.split_tail_end = new_tune.tail_end;
			}
			break;
		case kTUNE_SPLIT:
			NextItem(ctx, tSTRING);
			if ((ix = LookupEnvelopeName(ctx->item_string)) < 0) {
				error(ctx, "Bad envelope name: '%s'", ctx->item_string);
				break;
			}
			done_split = true;
			new_tune.split_nucleus_env = ix;
			new_tune.split_nucleus_max = NextItem(ctx, tNUMBER);
			new_tune.split_nucleus_min = NextItem(ctx, tNUMBER);
			new_tune.split_tail_start = NextItem(ctx, tNUMBER);
			new_tune.split_tail_end = NextItem(ctx, tNUMBER);
			NextItem(ctx, tSTRING);
			ctx->item_string[12] = 0;
			for (ix = 0; ix < n_tune_names; ix++) {
				if (strcmp(ctx->item_string, tune_names[ix]) == 0)
					break;
			}

			if (ix == n_tune_names)
				error(ctx, "Tune '%s' not found", ctx->item_string);
			else
				new_tune.split_tune = ix;
			break;
		default:
			error(ctx, "Unexpected: '%s'", ctx->item_string);
			break;
		}
	}

	for (ix = 0; ix < n_preset_tunes; ix++) {
		if (tune_data[ix].name[0] == 0)
			error(ctx, "Tune '%s' not defined", preset_tune_names[ix]);
	}
	fwrite(tune_data, n_tune_names, sizeof(TUNE), f_out);
	free(tune_data);
	fclose(ctx->f_in);
	fclose(f_out);

	fprintf(log, "Compiled %d intonation tunes: %d errors.\n", n_tune_names, ctx->error_count);

	LoadPhData(NULL, NULL);

	int res = ctx->error_count > 0 ? ENS_COMPILE_ERROR : ENS_OK;
	clean_context(ctx);
	return res;
}
