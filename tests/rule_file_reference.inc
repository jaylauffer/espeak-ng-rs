/* Frozen unchanged 6bc208e6 compiledict.c rule-file compiler/group functions.
 * Copyright (C) 2005-2014 Jonathan Duddington; 2015-2017 Reece H. Dunn.
 * SPDX-License-Identifier: GPL-3.0-or-later */
static int __cdecl string_sorter(char **a, char **b)
{
	char *pa, *pb;
	int ix;

	if ((ix = strcmp(pa = *a, pb = *b)) != 0)
		return ix;
	pa += (strlen(pa)+1);
	pb += (strlen(pb)+1);
	return strcmp(pa, pb);
}

static int __cdecl rgroup_sorter(RGROUP *a, RGROUP *b)
{
	// Sort long names before short names
	int ix;
	ix = strlen(b->name) - strlen(a->name);
	if (ix != 0) return ix;
	ix = strcmp(a->name, b->name);
	if (ix != 0) return ix;
	return a->index - b->index;
}

static void* output_rule_group(int n_rules, char **rules, char *name, size_t *outsize)
{
	int ix;
	int len1;
	int len2;
	int len_name;
	char *p;
	char *p2, *p3;
	const char *common;
	char *outptr = NULL;
	size_t outpos, outlen = 0;

	short nextchar_count[256];
	memset(nextchar_count, 0, sizeof(nextchar_count));

	len_name = strlen(name);

	// sort the rules in this group by their phoneme string
	common = "";
	qsort((void *)rules, n_rules, sizeof(char *), (int(__cdecl *)(const void *, const void *))string_sorter);

	if (strcmp(name, "9") == 0)
		len_name = 0; //  don't remove characters from numeric match strings

	for (ix = 0; ix < n_rules; ix++) {
		p = rules[ix];
		len1 = strlen(p) + 1; // phoneme string
		p3 = &p[len1];
		p2 = p3 + len_name; // remove group name from start of match string
		len2 = strlen(p2);

		nextchar_count[(unsigned char)(p2[0])]++; // the next byte after the group name

		outpos = outlen;
		if ((common[0] != 0) && (strcmp(p, common) == 0)) {
			outlen += len2 + 1;
			outptr = realloc(outptr, outlen);
			memmove(outptr + outpos, p2, len2);
			outptr[outlen-1] = 0;
		} else {
			if ((ix < n_rules-1) && (strcmp(p, rules[ix+1]) == 0)) {
				outlen ++;
				outptr = realloc(outptr, outlen);
				common = rules[ix]; // phoneme string is same as next, set as common
				outptr[outpos++] = RULE_PH_COMMON;
			}

			outlen += len2 + 1 + len1;
			outptr = realloc(outptr, outlen);
			memmove(outptr + outpos, p2, len2);
			outpos += len2;
			outptr[outpos++] = RULE_PHONEMES;
			memmove(outptr + outpos, p, len1);
		}
	}
	if (outsize) *outsize = outlen;
	return outptr;
}

static int compile_lettergroup(CompileContext *ctx, char *input, FILE *f_out)
{
	char *p;
	char *p_start;
	int group;
	int ix;
	int n_items;
	int length;
	int max_length = 0;

	#define N_LETTERGP_ITEMS 200
	char *items[N_LETTERGP_ITEMS];
	char item_length[N_LETTERGP_ITEMS];

	p = input;
	if (!IsDigit09(p[0]) || !IsDigit09(p[1])) {
		fprintf(ctx->f_log, "%5d: Expected 2 digits after '.L'\n", ctx->linenum);
		ctx->error_count++;
		return 1;
	}

	group = atoi(&p[0]);
	if (group >= N_LETTER_GROUPS) {
		fprintf(ctx->f_log, "%5d: lettergroup out of range (01-%.2d)\n", ctx->linenum, N_LETTER_GROUPS-1);
		ctx->error_count++;
		return 1;
	}

	while (!isspace2(*p)) p++;

	fputc(RULE_GROUP_START, f_out);
	fputc(RULE_LETTERGP2, f_out);
	fputc(group + 'A', f_out);
	if (ctx->letterGroupsDefined[group] != 0) {
		fprintf(ctx->f_log, "%5d: lettergroup L%.2d is already defined\n", ctx->linenum, group);
		ctx->error_count++;
	}
	ctx->letterGroupsDefined[group] = 1;

	n_items = 0;
	while (n_items < N_LETTERGP_ITEMS) {
		while (isspace2(*p)) p++;
		if (*p == 0)
			break;

		items[n_items] = p_start = p;
		while ((*p & 0xff) > ' ') {
			if (*p == '_') *p = ' '; // allow '_' for word break
			p++;
		}
		*p++ = 0;
		length = p - p_start;
		if (length > max_length)
			max_length = length;
		item_length[n_items++] = length;
	}

	// write out the items, longest first
	while (max_length > 1) {
		for (ix = 0; ix < n_items; ix++) {
			if (item_length[ix] == max_length)
				fwrite(items[ix], 1, max_length, f_out);
		}
		max_length--;
	}

	fputc(RULE_GROUP_END, f_out);

	return 0;
}

static void free_rules(char **rules, int n_rules)
{
	for (int i = 0; i < n_rules; ++i) {
		free(*rules);
		*rules++ = NULL;
	}
}

static espeak_ng_STATUS compile_dictrules(CompileContext *ctx, FILE *f_in, FILE *f_out)
{
	char *prule;
	unsigned char *p;
	int ix;
	int c;
	int gp;
	int n_rules = 0;
	int count = 0;
	int different;
	int wc;
	int err_n_rules = 0;
	const char *prev_rgroup_name;
	unsigned int char_code;
	int compile_mode = 0;
	char *buf;
	char buf1[500];
	char *rules[N_RULES];

	int n_rgroups = 0;
	int n_groups3 = 0;
	RGROUP rgroup[N_RULE_GROUP2];

	ctx->linenum = 0;
	ctx->group_name[0] = 0;

	for (;;) {
		ctx->linenum++;
		buf = fgets(buf1, sizeof(buf1), f_in);
		if (buf != NULL) {
			if ((p = (unsigned char *)strstr(buf, "//")) != NULL)
				*p = 0;

			if (buf[0] == '\r') buf++; // ignore extra \r in \r\n
		}

		if ((buf == NULL) || (buf[0] == '.')) {
			// next .group or end of file, write out the previous group

			if (n_rules > 0) {
				strcpy(rgroup[n_rgroups].name, ctx->group_name);
				rgroup[n_rgroups].group3_ix = ctx->group3_ix;
				rgroup[n_rgroups].start = output_rule_group(n_rules, rules, ctx->group_name, &rgroup[n_rgroups].length);
				rgroup[n_rgroups].index = n_rgroups;
				n_rgroups++;

				count += n_rules;
				free_rules(rules, n_rules);
			}
			n_rules = 0;
			err_n_rules = 0;

			if (compile_mode == 2) {
				// end of the character replacements section
				fwrite(&n_rules, 1, 4, f_out); // write a zero word to terminate the replacemenmt list
				fputc(RULE_GROUP_END, f_out);
				compile_mode = 0;
			}

			if (buf == NULL) break; // end of file

			if (memcmp(buf, ".L", 2) == 0) {
				compile_lettergroup(ctx, &buf[2], f_out);
				continue;
			}

			if (memcmp(buf, ".replace", 8) == 0) {
				compile_mode = 2;
				fputc(RULE_GROUP_START, f_out);
				fputc(RULE_REPLACEMENTS, f_out);

				// advance to next word boundary
				while ((ftell(f_out) & 3) != 0)
					fputc(0, f_out);
			}

			if (memcmp(buf, ".group", 6) == 0) {
				compile_mode = 1;

				p = (unsigned char *)&buf[6];
				while ((p[0] == ' ') || (p[0] == '\t')) p++; // Note: Windows isspace(0xe1) gives TRUE !
				ix = 0;
				while ((*p > ' ') && (ix < LEN_GROUP_NAME))
					ctx->group_name[ix++] = *p++;
				ctx->group_name[ix] = 0;
				ctx->group3_ix = 0;

				if (sscanf(ctx->group_name, "0x%x", &char_code) == 1) {
					// group character is given as a character code (max 16 bits)
					p = (unsigned char *)ctx->group_name;

					if (char_code > 0x100)
						*p++ = (char_code >> 8);
					*p++ = char_code;
					*p = 0;
				} else {
					if (translator->letter_bits_offset > 0) {
						utf8_in(&wc, ctx->group_name);
						if (((ix = (wc - translator->letter_bits_offset)) >= 0) && (ix < 128))
							ctx->group3_ix = ix+1; // not zero
					}
				}

				if ((ctx->group3_ix == 0) && (strlen(ctx->group_name) > 2)) {
					if (utf8_in(&c, ctx->group_name) < 2) {
						fprintf(ctx->f_log, "%5d: Group name longer than 2 bytes (UTF8)", ctx->linenum);
						ctx->error_count++;
					}

					ctx->group_name[2] = 0;
				}
			}

			continue;
		}

		switch (compile_mode)
		{
		case 1: //  .group
			prule = compile_rule(ctx, buf);
			if (prule != NULL) {
				if (n_rules < N_RULES)
					rules[n_rules++] = prule;
				else {
					if (err_n_rules == 0) {
						fprintf(stderr, "\nExceeded limit of rules (%d) in group '%s'\n", N_RULES, ctx->group_name);
						ctx->error_count++;
						err_n_rules = 1;
					}
				}

			}
			break;
		case 2: //  .replace
			p = (unsigned char *)buf;

			while (isspace2(*p)) p++;
			if ((unsigned char)(*p) > 0x20) {
				while ((unsigned char)(*p) > 0x20) { // not space or zero-byte
					fputc(*p, f_out);
					p++;
				}
				fputc(0, f_out);

				while (isspace2(*p)) p++;
				while ((unsigned char)(*p) > 0x20) {
					fputc(*p, f_out);
					p++;
				}
				fputc(0, f_out);
			}
			break;
		}
	}

	qsort((void *)rgroup, n_rgroups, sizeof(rgroup[0]), (int(__cdecl *)(const void *, const void *))rgroup_sorter);

	prev_rgroup_name = "\n";

	for (gp = 0; gp < n_rgroups; gp++) {
		if ((different = strcmp(rgroup[gp].name, prev_rgroup_name)) != 0) {
			// not the same as the previous group
			if (gp > 0)
				fputc(RULE_GROUP_END, f_out);
			fputc(RULE_GROUP_START, f_out);

			if (rgroup[gp].group3_ix != 0) {
				n_groups3++;
				fputc(1, f_out);
				fputc(rgroup[gp].group3_ix, f_out);
			} else
				fprintf(f_out, "%s", prev_rgroup_name = rgroup[gp].name);
			fputc(0, f_out);
		}

		fwrite(rgroup[gp].start, rgroup[gp].length, 1, f_out);
	}
	fputc(RULE_GROUP_END, f_out);
	fputc(0, f_out);

	fprintf(ctx->f_log, "\t%d rules, %d groups (%d)\n\n", count, n_rgroups, n_groups3);
	free_rules(rules, n_rules);
	for (gp = 0; gp < n_rgroups; gp++) {
		free(rgroup[gp].start);
	}
	return ENS_OK;
}

/* End frozen rule-file compiler. */
