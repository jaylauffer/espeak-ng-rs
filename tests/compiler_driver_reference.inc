/* Frozen complete public driver and manifest from b8f52c7f.
 * Copyright (C) 2005-2014 Jonathan Duddington, 2013-2017 Reece H. Dunn.
 * SPDX-License-Identifier: GPL-3.0-or-later */
static espeak_ng_STATUS ReadPhondataManifest(CompileContext *ctx, espeak_ng_ERROR_CONTEXT *context)
{
	// Read the phondata-manifest file
	FILE *f;
	int n_lines = 0;
	char *p;
	unsigned int value;
	char buf[N_PATH_BUF];
	char name[120];

	snprintf(buf, sizeof(buf), "%s%c%s", path_home, PATHSEP, "phondata-manifest");
	if ((f = fopen(buf, "r")) == NULL)
		return create_file_error_context(context, errno, buf);

	while (fgets(buf, sizeof(buf), f) != NULL)
		n_lines++;

	rewind(f);

	if (ctx->manifest != NULL) {
		for (int ix = 0; ix < ctx->n_manifest; ix++)
			free(ctx->manifest[ix].name);
	}

	if (n_lines == 0) {
		fclose(f);
		return ENS_EMPTY_PHONEME_MANIFEST;
	}

	NAMETAB *new_manifest = (NAMETAB *)realloc(ctx->manifest, n_lines * sizeof(NAMETAB));
	if (new_manifest == NULL) {
		fclose(f);
		free(ctx->manifest);
		return ENOMEM;
	} else
		ctx->manifest = new_manifest;

	ctx->n_manifest = 0;
	while (fgets(buf, sizeof(buf), f) != NULL) {
		if (!isalpha(buf[0]))
			continue;

		if (sscanf(&buf[2], "%x %s", &value, name) == 2) {
			if ((p = (char *)malloc(strlen(name)+1)) != NULL) {
				strcpy(p, name);
				ctx->manifest[ctx->n_manifest].value = value;
				ctx->manifest[ctx->n_manifest].name = p;
				ctx->n_manifest++;
			}
		}
	}
	fclose(f);

	return ENS_OK;
}

espeak_ng_STATUS
espeak_ng_CompilePhonemeDataPath(long rate,
                                 const char *source_path,
                                 const char *destination_path,
                                 FILE *log,
                                 espeak_ng_ERROR_CONTEXT *context)
{
	if (!log) log = stderr;

	char fname[N_PATH_BUF];
	char phdst[N_PATH_BUF]; // Destination: path to the phondata/phontab/phonindex output files.

	CompileContext *ctx = calloc(1, sizeof(CompileContext));
	if (!ctx) return ENOMEM;
#ifdef USE_RUST_CORE
	ctx->table_owner = espeak_rs_compiler_tables_begin();
	if (!ctx->table_owner) { clean_context(ctx); return ENOMEM; }
	ctx->program_owner = espeak_rs_compiler_program_begin();
	if (!ctx->program_owner) { clean_context(ctx); return ENOMEM; }
#endif

	if (source_path) {
		sprintf(ctx->phsrc, "%s", source_path);
	} else {
		snprintf(ctx->phsrc, sizeof(ctx->phsrc), "%s/../phsource", path_home);
	}

	if (destination_path) {
		sprintf(phdst, "%s", destination_path);
	} else {
		sprintf(phdst, "%s", path_home);
	}

	samplerate = rate;
	LoadPhData(NULL, NULL);
	if (LoadVoice("", 8/*compiling phonemes*/) == NULL) {
		clean_context(ctx);
		return ENS_VOICE_NOT_FOUND;
	}

	WavegenInit(rate, 0);
	WavegenSetVoice(voice);

	ctx->error_count = 0;
	ctx->f_errors = log;

	strncpy0(ctx->current_fname, "phonemes", sizeof(ctx->current_fname));

	snprintf(fname, sizeof(fname), "%s/phonemes", ctx->phsrc);
	fprintf(log, "Compiling phoneme data: %s\n", fname);
	ctx->f_in = SourceOpen(ctx, fname, 0, 0);
	if (!ctx->f_in) {
		clean_context(ctx);
		return create_file_error_context(context, errno, fname);
	}

#ifdef USE_RUST_CORE
	RustCompilerOutput outputs = {ctx->phsrc, phdst, N_PATH_BUF, version_phdata, samplerate, ctx, OutputFallback};
	RustCompilerOutputResult opened = {0};
	int output_status = espeak_rs_compiler_output_begin(&outputs, &opened);
	if (output_status) {
		SourceClose(ctx); clean_context(ctx);
		return create_file_error_context(context, output_status, (const char *)opened.filename);
	}
	ctx->output_owner = opened.owner; ctx->output_mask = opened.mask;
#else
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

#endif

#ifndef USE_RUST_CORE
	memset(ctx->ref_hash_tab, 0, sizeof(ctx->ref_hash_tab));
#endif

	ctx->n_phoneme_tabs = 0;
#ifndef USE_RUST_CORE
	MAKE_MEM_UNDEFINED(ctx->n_phcodes_list, sizeof(ctx->n_phcodes_list));
	MAKE_MEM_UNDEFINED(ctx->phoneme_tab_list2, sizeof(ctx->phoneme_tab_list2));
#endif

	#ifndef USE_RUST_CORE
	ctx->stack_ix = 0;
	MAKE_MEM_UNDEFINED(ctx->stack, sizeof(ctx->stack));
	#endif

	StartPhonemeTable(ctx, "base");
	CompilePhonemeFiles(ctx);

	EndPhonemeTable(ctx);
	WritePhonemeTables(ctx);

	fprintf(ctx->f_errors, "\nRefs %d,  Reused %d\n", ctx->count_references, ctx->duplicate_references);

	SourceClose(ctx);
#ifdef USE_RUST_CORE
	output_status = espeak_rs_compiler_output_end(ctx->output_owner); ctx->output_owner = 0;
	if (!output_status) output_status = ctx->table_status;
	if (!output_status) output_status = ctx->program_status;
	if (output_status) {
		error_from_status(ctx, output_status, "compiler output");
		if (ctx->f_errors != stderr && ctx->f_errors != stdout) fclose(ctx->f_errors);
		clean_context(ctx);
		return output_status;
	}
#else
	fclose(ctx->f_phcontents);
	fclose(ctx->f_phdata);
	fclose(ctx->f_phindex);
	fclose(ctx->f_phtab);
	if (ctx->f_prog_log != NULL)
		fclose(ctx->f_prog_log);
#endif

	LoadPhData(NULL, NULL);

	WavegenFini();

	fprintf(log, "Compiled phonemes: %d errors.\n", ctx->error_count);

	if (ctx->f_errors != stderr && ctx->f_errors != stdout)
		fclose(ctx->f_errors);

	espeak_ng_STATUS status = ReadPhondataManifest(ctx, context);
	int res = ctx->error_count > 0 ? ENS_COMPILE_ERROR : ENS_OK;
	clean_context(ctx);
	return (status != ENS_OK) ? status : res;
}
