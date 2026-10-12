/* Frozen native compiler source/context teardown from 282b8543.
 * Copyright (C) 2005-2014 Jonathan Duddington, 2013-2017 Reece H. Dunn.
 * SPDX-License-Identifier: GPL-3.0-or-later */
static void clean_context(CompileContext *ctx) {
#ifdef USE_RUST_CORE
	espeak_rs_compiler_program_end(ctx->program_owner);
	espeak_rs_compiler_tables_end(ctx->table_owner);
	espeak_rs_compiler_output_end(ctx->output_owner);
	espeak_rs_compiler_sources_end(ctx->source_pool);
	espeak_rs_compiler_resources_end(ctx->resource_owner);
#else
	for (int i = 0; i < 256; i++) {
		REF_HASH_TAB *p;
		while ((p = ctx->ref_hash_tab[i])) {
			ctx->ref_hash_tab[i] = (REF_HASH_TAB*)p->link;
			free(p);
		}
	}
#endif
	#ifndef USE_RUST_CORE
	for (int i = 0; i < ctx->n_manifest; i++) {
		free(ctx->manifest[i].name);
	}
	free(ctx->manifest);
	#endif
	free(ctx);
}

static CompilerInput SourceOpen(CompileContext *ctx, const char *path, size_t depth, int text)
{
	int file_error = 0;
	if (!ctx->source_pool) {
		unsigned char spaces[256], digits[256];
		for (int i = 0; i < 256; i++) {
			spaces[i] = isspace(i) != 0;
			digits[i] = isdigit(i) != 0;
		}
		ctx->source_pool = espeak_rs_compiler_sources_begin(spaces, digits, &file_error);
		if (!ctx->source_pool) {
			errno = file_error;
			return 0;
		}
	}
#ifndef _WIN32
	text = 0;
#endif
	int input = espeak_rs_compiler_source_open(ctx->source_pool, path, depth, text, &file_error);
	if (!input) errno = file_error;
	return input;
}
