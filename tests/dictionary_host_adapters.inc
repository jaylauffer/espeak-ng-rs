/* Frozen a9184c95 native dictionary adapters. Shared Rust controllers: this
 * proves host wiring only; separate full-C fixtures prove compiler algorithms.
 * SPDX-License-Identifier: GPL-3.0-or-later */
/* Begin native dictionary-list compiler. */
typedef struct { CompileContext *ctx; void *output; } DictListIo;
static int list_encode(void *opaque, const char *input, char *output, size_t capacity, int *bad)
{
	(void)opaque;
	return EncodePhonemesBounded(input, output, capacity, bad) != NULL;
}
static void list_translate(void *opaque, char *input, char *output)
{
	DictListIo *io = opaque;
	TranslateWord(translator, input, NULL, NULL);
	strncpy0(output, io->ctx->word_phonemes, N_WORD_BYTES-4);
}
static int list_case(void *opaque, uint32_t code, uint32_t *lower)
{
	(void)opaque;
	if (!iswupper(code)) return 0;
	*lower = towlower2(code, translator);
	return 1;
}
static int list_transpose(void *opaque, char *word)
{
	(void)opaque;
	return translator->transpose_min > 0 ? TransposeAlphabet(translator, word) : (int)strlen(word);
}
/* Begin native dictionary list diagnostic projection. */
static void list_report(void *opaque, int line, int kind, int value, const char *first, const char *second)
{
 CompileContext *ctx=((DictListIo *)opaque)->ctx;
 RustDictionaryDiagnostic d={ctx->f_log,stderr,NULL,&ctx->error_count,NULL,0};
 espeak_rs_dictionary_diagnostic(&d,RUST_DIAGNOSTIC_LIST,line,kind,value,first,second,NULL);
}
/* End native dictionary list diagnostic projection. */
static int list_write(void *opaque, const uint8_t *bytes, size_t length)
{
	return espeak_rs_dictionary_write(((DictListIo *)opaque)->output, bytes, length);
}
static RustDictList list_descriptor(DictListIo *io, uint8_t alnums[256])
{
	for (unsigned i = 0; i < 256; i++) alnums[i] = isalnum(i) != 0;
	RustDictList host = { io, alnums, translator->langopts.textmode, translator->langopts.listx,
#ifdef _WIN32
		1,
#else
		0,
#endif
		list_encode, list_translate, list_case, list_transpose, list_report, list_write };
	return host;
}
#ifdef ESPEAK_DICT_LIST_FIXTURE
static int compile_line_native(CompileContext *ctx, const char *input, char *output, int capacity, int *hash)
{
	uint8_t alnums[256]; DictListIo io = { ctx, NULL };
	RustDictList host = list_descriptor(&io, alnums);
	RustDictListState state = { ctx->linenum, ctx->text_mode, ctx->error_need_dictionary };
	int length = espeak_rs_compile_dict_line(&host, input, (uint8_t *)output, capacity, &state, hash);
	ctx->text_mode = state.text_mode; ctx->error_need_dictionary = state.need_dictionary;
	return length;
}
#endif
static espeak_ng_STATUS compile_dictlists_native(CompileContext *ctx, const char *prefix, void *output)
{
	uint8_t alnums[256]; DictListIo io = { ctx, output };
	RustDictList host = list_descriptor(&io, alnums);
	RustDictListState state = { ctx->linenum, ctx->text_mode, ctx->error_need_dictionary };
	espeak_ng_STATUS status = espeak_rs_compile_dict_lists(&host, prefix, &state);
	ctx->linenum = state.line; ctx->text_mode = state.text_mode; ctx->error_need_dictionary = state.need_dictionary;
	return status;
}
/* End native dictionary-list compiler. */

/* Begin native individual rule compiler. */
static int rule_encode(void *opaque, const char *input, char *output, size_t capacity, int *bad)
{
	(void)opaque;
	return EncodePhonemesBounded(input, output, capacity, bad) != NULL;
}

static int rule_digit(uint32_t code)
{
	return IsDigit(code);
}

#ifdef ESPEAK_RULE_COMPILER_FIXTURE
/* Begin native dictionary rule diagnostic projection. */
static void rule_report(void *opaque, int kind, int value, const char *text)
{
 CompileContext *ctx=opaque;
 RustDictionaryDiagnostic d={ctx->f_log,stderr,&ctx->linenum,&ctx->error_count,(unsigned char *)ctx->group_name,sizeof(ctx->group_name)};
 espeak_rs_dictionary_diagnostic(&d,RUST_DIAGNOSTIC_RULE,0,kind,value,text,NULL,NULL);
}
/* End native dictionary rule diagnostic projection. */


static char *compile_rule(CompileContext *ctx, char *input)
{
	uint8_t spaces[256], output[512];
	for (int ix = 0; ix < 256; ix++) spaces[ix] = isspace2((char)ix) != 0;
	RustRuleCompiler compiler = { ctx->group_name, (const uint8_t *)ctx->letterGroupsDefined,
		spaces, ctx->linenum, ctx->debug_flag, (char)0xff < 0, ctx, rule_encode, rule_digit, rule_report };
	size_t length = espeak_rs_compile_rule(&compiler, input, output, sizeof(output));
	char *rule = length ? malloc(length) : NULL;
	if (rule != NULL) memcpy(rule, output, length);
	return rule;
}
#endif
/* End native individual rule compiler. */

/* Begin native rule-file compiler. */
typedef struct { CompileContext *compiler; void *output; } RuleFileIo;
static int rule_file_write(void *opaque, const uint8_t *bytes, size_t length)
{
	RuleFileIo *io = opaque;
	return espeak_rs_dictionary_write(io->output, bytes, length);
}
static int rule_file_tell(void *opaque, uint64_t *position)
{
	RuleFileIo *io = opaque;
	return espeak_rs_dictionary_tell(io->output, position);
}
/* Begin native dictionary file diagnostic projection. */
static void rule_file_report(void *opaque, int line, const uint8_t *group, int kind, int value, const char *text)
{
 CompileContext *ctx=((RuleFileIo *)opaque)->compiler;
 RustDictionaryDiagnostic d={ctx->f_log,stderr,&ctx->linenum,&ctx->error_count,(unsigned char *)ctx->group_name,sizeof(ctx->group_name)};
 espeak_rs_dictionary_diagnostic(&d,RUST_DIAGNOSTIC_FILE,line,kind,value,text,NULL,group);
}
/* End native dictionary file diagnostic projection. */
static espeak_ng_STATUS compile_dictrules_native(CompileContext *ctx, const char *path, void *output)
{
	uint8_t spaces[256];
	for (int ix = 0; ix < 256; ix++) spaces[ix] = isspace2((char)ix) != 0;
	RuleFileIo io = { ctx, output };
	RustRuleFile compiler = { &io, (const uint8_t *)ctx->letterGroupsDefined, spaces,
		ctx->debug_flag, (char)0xff < 0, translator->letter_bits_offset,
#ifdef _WIN32
		1,
#else
		0,
#endif
		rule_encode, rule_digit, rule_file_report, rule_file_write, rule_file_tell };
	RustRuleFileState state;
	espeak_ng_STATUS status = espeak_rs_compile_rule_file(&compiler, path, &state);
	if (status == ENS_OK) {
		ctx->linenum = state.line;
		ctx->group3_ix = state.group3;
		memcpy(ctx->group_name, state.group, sizeof(ctx->group_name));
		memcpy(ctx->letterGroupsDefined, state.letters, sizeof(ctx->letterGroupsDefined));
	}
	return status;
}
/* End native rule-file compiler. */

/* Begin native dictionary compiler projections. */
typedef struct { CompileContext *compiler; espeak_ng_ERROR_CONTEXT *error; } DictionaryIo;
static int dictionary_lists(void *opaque, const char *path, void *writer)
{
	return compile_dictlists_native(((DictionaryIo *)opaque)->compiler, path, writer);
}
static int dictionary_rules(void *opaque, const char *path, void *writer)
{
	return compile_dictrules_native(((DictionaryIo *)opaque)->compiler, path, writer);
}
/* Begin native dictionary driver diagnostic projection. */
static void dictionary_log(void *opaque, int kind, const char *text)
{
 RustDictionaryDiagnostic d={((DictionaryIo *)opaque)->compiler->f_log,NULL,NULL,NULL,NULL,0};
 espeak_rs_dictionary_diagnostic(&d,RUST_DIAGNOSTIC_DRIVER,0,kind,0,text,NULL,NULL);
}
/* End native dictionary driver diagnostic projection. */
static void dictionary_reload(void *opaque, const char *name)
{
	(void)opaque;
	LoadDictionary(translator, name, 0);
}
static int dictionary_errors(void *opaque)
{
	return ((DictionaryIo *)opaque)->compiler->error_count;
}
static int dictionary_error(void *opaque, int status, const char *path)
{
	return create_file_error_context(((DictionaryIo *)opaque)->error, status, path);
}
ESPEAK_NG_API espeak_ng_STATUS espeak_ng_CompileDictionary(const char *dsource, const char *dict_name, FILE *log, int flags, espeak_ng_ERROR_CONTEXT *context)
{
	CompileContext ctx = {0}; ctx.f_log = log ? log : stderr; ctx.debug_flag = flags & 1;
	DictionaryIo io = { &ctx, context };
	RustDictionaryCompiler compiler = { &io, dsource, dict_name ? dict_name : dictionary_name,
		path_home, phoneme_tab_list[phoneme_tab_number].name, N_PATH_BUF, PATHSEP,
		dictionary_lists, dictionary_rules, dictionary_log, dictionary_reload, dictionary_errors, dictionary_error };
	return espeak_rs_compile_dictionary(&compiler);
}
/* End native dictionary compiler projections. */

/* End frozen dictionary adapters. */
