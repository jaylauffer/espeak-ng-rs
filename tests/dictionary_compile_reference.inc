/* Unchanged 645abcb2 top-level native dictionary compiler. */
ESPEAK_NG_API espeak_ng_STATUS espeak_ng_CompileDictionary(const char *dsource, const char *dict_name, FILE *log, int flags, espeak_ng_ERROR_CONTEXT *context)
{
	if (!log) log = stderr;
	if (!dict_name) dict_name = dictionary_name;

	// fname:  space to write the filename in case of error
	// flags: bit 0:  include source line number information, for debug purposes.

	FILE *f_in;
	FILE *f_out;
	int offset_rules = 0;
	int value;
	char fname_in[N_PATH_BUF];
	char fname_out[N_PATH_BUF];
	char path[N_PATH_BUF];

	CompileContext *ctx = calloc(1, sizeof(CompileContext));

	ctx->error_count = 0;
	ctx->error_need_dictionary = 0;
	memset(ctx->letterGroupsDefined, 0, sizeof(ctx->letterGroupsDefined));

	ctx->debug_flag = flags & 1;

	if (dsource == NULL)
		dsource = "";

	ctx->f_log = log;
	if (ctx->f_log == NULL)
		ctx->f_log = stderr;

	// try with and without '.txt' extension
	snprintf(path, sizeof(path), "%s%s_", dsource, dict_name);
	snprintf(fname_in, sizeof(fname_in), "%srules.txt", path);
	if ((f_in = fopen(fname_in, "r")) == NULL) {
		snprintf(fname_in, sizeof(fname_in), "%srules", path);
		if ((f_in = fopen(fname_in, "r")) == NULL) {
			clean_context(ctx);
			return create_file_error_context(context, errno, fname_in);
		}
	}

	snprintf(fname_out, sizeof(fname_out), "%s%c%s_dict", path_home, PATHSEP, dict_name);
	if ((f_out = fopen(fname_out, "wb+")) == NULL) {
		int error = errno;
		fclose(f_in);
		clean_context(ctx);
		return create_file_error_context(context, error, fname_out);
	}

	value = N_HASH_DICT;
	Write4Bytes(f_out, value);
	Write4Bytes(f_out, offset_rules);

	fprintf(ctx->f_log, "Using phonemetable: '%s'\n", phoneme_tab_list[phoneme_tab_number].name);
#ifdef USE_RUST_CORE
	espeak_ng_STATUS list_status = compile_dictlists_native(ctx, path, f_out);
	if (list_status != ENS_OK) {
		fclose(f_in); fclose(f_out); clean_context(ctx);
		return list_status;
	}
#else
	compile_dictlist_start(ctx);
	compile_dictlist_file(ctx, path, "roots");
	if (translator->langopts.listx) {
		compile_dictlist_file(ctx, path, "list");
		compile_dictlist_file(ctx, path, "listx");
	} else {
		compile_dictlist_file(ctx, path, "listx");
		compile_dictlist_file(ctx, path, "list");
	}
	compile_dictlist_file(ctx, path, "emoji");
	compile_dictlist_file(ctx, path, "extra");

	compile_dictlist_end(ctx, f_out);
#endif
	offset_rules = ftell(f_out);

	fprintf(ctx->f_log, "Compiling: '%s'\n", fname_in);

#ifdef USE_RUST_CORE
	espeak_ng_STATUS status = compile_dictrules_native(ctx, fname_in, f_out);
#else
	espeak_ng_STATUS status = compile_dictrules(ctx, f_in, f_out);
#endif
	fclose(f_in);

	fseek(f_out, 4, SEEK_SET);
	Write4Bytes(f_out, offset_rules);
	fclose(f_out);
	fflush(ctx->f_log);

	if (status != ENS_OK) {
		clean_context(ctx);
		return status;
	}

	LoadDictionary(translator, dict_name, 0);

	status = ctx->error_count > 0 ? ENS_COMPILE_ERROR : ENS_OK;
	clean_context(ctx);
	return status;
}
/* End frozen dictionary compiler. */
