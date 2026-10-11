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

/* Frozen 82f273a3 scanner and replay helpers; test-only. */
static unsigned int get_char(CompileContext *ctx)
{
	unsigned int c;
	c = fgetc(ctx->f_in);
	if (c == '\n')
		ctx->linenum++;
	return c;
}

static void unget_char(CompileContext *ctx, unsigned int c)
{
	ungetc(c, ctx->f_in);
	if (c == '\n')
		ctx->linenum--;
}

static int CheckNextChar(CompileContext *ctx)
{
	int c;
	while (((c = get_char(ctx)) == ' ') || (c == '\t'))
		;
	unget_char(ctx, c);
	return c;
}

static int NextItem(CompileContext *ctx, int type)
{
	unsigned char c = 0;
	unsigned char c2;
	int ix;
	const keywtab_t *pk;

	ctx->item_type = -1;

	ctx->f_in_displ = ftell(ctx->f_in);
	ctx->f_in_linenum = ctx->linenum;

	while (!feof(ctx->f_in)) {
		c = get_char(ctx);
		if (c == '/') {
			if ((c2 = get_char(ctx)) == '/') {
				// comment, ignore to end of line
				while (!feof(ctx->f_in) && ((c = get_char(ctx)) != '\n'))
					;
			} else
				unget_char(ctx, c2);
		}
		if (!isspace(c))
			break;
	}
	if (feof(ctx->f_in))
		return -2;

	if (c == '(') {
		if (type == tOPENBRACKET)
			return 1;
		return -1;
	}

	ix = 0;
	while (!feof(ctx->f_in) && !isspace(c) && (c != '(') && (c != ')') && (c != ',')) {
		if (c == '\\')
			c = get_char(ctx);
		ctx->item_string[ix++] = c;
		c = get_char(ctx);
		if (feof(ctx->f_in))
			break;
		if (ctx->item_string[ix-1] == '=')
			break;
	}
	ctx->item_string[ix] = 0;

	while (isspace(c))
		c = get_char(ctx);

	ctx->item_terminator = ' ';
	if ((c == ')') || (c == '(') || (c == ','))
		ctx->item_terminator = c;

	if (!feof(ctx->f_in) && !(c == ')' || c == ','))
		unget_char(ctx, c);

	if (type == tSTRING)
		return 0;

	if ((type == tNUMBER) || (type == tSIGNEDNUMBER)) {
		int acc = 0;
		int sign = 1;
		char *p;
		 p = ctx->item_string;

		if ((*p == '-') && (type == tSIGNEDNUMBER)) {
			sign = -1;
			p++;
		}
		if (!isdigit(*p)) {
			if ((type == tNUMBER) && (*p == '-'))
				error(ctx, "Expected an unsigned number");
			else
				error(ctx, "Expected a number");
		}
		while (isdigit(*p)) {
			acc *= 10;
			acc += (*p - '0');
			p++;
		}
		return acc * sign;
	}

	if ((type >= tKEYWORD) && (type <= tINTONATION)) {
		pk = keyword_tabs[type-tKEYWORD];
		while (pk->mnem != NULL) {
			if (strcmp(ctx->item_string, pk->mnem) == 0) {
				ctx->item_type = pk->type;
				return pk->data;
			}
			pk++;
		}
		ctx->item_type = -1;
		return -1; // keyword not found
	}
	if (type == tPHONEMEMNEM)
		return LookupPhoneme(ctx, ctx->item_string, 2);
	return -1;
}

static void UngetItem(CompileContext *ctx)
{
	fseek(ctx->f_in, ctx->f_in_displ, SEEK_SET);
	ctx->linenum = ctx->f_in_linenum;
}

