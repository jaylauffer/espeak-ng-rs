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

/* Frozen complete output creation/header/failure cleanup and successful close
 * blocks from a2adc349 compiledata.c. Test adapters only replace surrounding
 * source/context teardown, error publication, and borrowed-stderr close. */
static int ReferenceBegin(CompileContext *ctx,const char *phdst,uint32_t version_phdata,int samplerate,Error *context)
{
	char fname[N_PATH_BUF];
	snprintf(fname, sizeof(fname), "%s/%s", phdst, "phondata-manifest");
	if ((ctx->f_phcontents = fopen(fname, "w")) == NULL)
		ctx->f_phcontents = stderr;

	fprintf(ctx->f_phcontents,
	        "# This file lists the type of data that has been compiled into the\n"
	        "# phondata file\n"
	        "#\n"
	        "# The first character of a line indicates the type of data:\n"
	        "#   S - A SPECT_SEQ structure\n"
	        "#   W - A wavefile segment\n"
	        "#   E - An envelope\n"
	        "#\n"
	        "# Address is the displacement within phondata of this item\n"
	        "#\n"
	        "#  Address  Data file\n"
	        "#  -------  ---------\n");

	snprintf(fname, sizeof(fname), "%s/%s", phdst, "phondata");
	ctx->f_phdata = fopen(fname, "wb");
	if (ctx->f_phdata == NULL) {
		int error = errno;
		SourceClose(ctx);
		fclose(ctx->f_phcontents);
		clean_context(ctx);
		return create_file_error_context(context, error, fname);
	}

	snprintf(fname, sizeof(fname), "%s/%s", phdst, "phonindex");
	ctx->f_phindex = fopen(fname, "wb");
	if (ctx->f_phindex == NULL) {
		int error = errno;
		SourceClose(ctx);
		fclose(ctx->f_phcontents);
		fclose(ctx->f_phdata);
		clean_context(ctx);
		return create_file_error_context(context, error, fname);
	}

	snprintf(fname, sizeof(fname), "%s/%s", phdst, "phontab");
	ctx->f_phtab = fopen(fname, "wb");
	if (ctx->f_phtab == NULL) {
		int error = errno;
		SourceClose(ctx);
		fclose(ctx->f_phcontents);
		fclose(ctx->f_phdata);
		fclose(ctx->f_phindex);
		clean_context(ctx);
		return create_file_error_context(context, error, fname);
	}

	snprintf(fname, sizeof(fname), "%s/compile_prog_log", ctx->phsrc);
	ctx->f_prog_log = fopen(fname, "wb");

	// write a word so that further data doesn't start at displ=0
	Write4Bytes(ctx->f_phdata, version_phdata);
	Write4Bytes(ctx->f_phdata, samplerate);
	Write4Bytes(ctx->f_phindex, version_phdata);

	return 0;
}
static void ReferenceFinish(CompileContext *ctx)
{
	SourceClose(ctx);
	fclose(ctx->f_phcontents);
	fclose(ctx->f_phdata);
	fclose(ctx->f_phindex);
	fclose(ctx->f_phtab);
	if (ctx->f_prog_log != NULL)
		fclose(ctx->f_prog_log);
}
