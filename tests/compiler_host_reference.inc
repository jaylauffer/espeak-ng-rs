/* ec00d27e previous native callback host. The generic Rust controllers
 * are shared; this fixture compares host wiring/publication, not independent
 * complete-C compiler algorithms. Original callback bodies retained.
 * Fragment SHA256: 4c8e8dd51f2b2bffb3bf9cfffb8b0b5276bc88a11a422345842ddf32a06a663a
 * SPDX-License-Identifier: GPL-3.0-or-later */
/* Begin native compiler diagnostic projections. */
static void CompilerStatusMessage(int status, char *bytes, size_t length)
{ espeak_ng_GetStatusCodeMessage(status, bytes, length); }
static void error_from_status(CompileContext *ctx, espeak_ng_STATUS status, const char *context)
{
 RustCompilerDiagnosticStatus d = {ctx, status, context, 4096, CompilerStatusMessage};
 espeak_rs_compiler_diagnostic_status(&d);
}
/* End native compiler diagnostic projections. */

static int NextItem(CompileContext *, int);
static int TableBase(void *opaque, unsigned char *bytes, size_t width)
{
	CompileContext *ctx = opaque;
	NextItem(ctx, tSTRING);
	size_t length = strnlen(ctx->item_string, sizeof(ctx->item_string));
	if (length >= width) return EINVAL;
	memcpy(bytes, ctx->item_string, length + 1); return ENS_OK;
}
/* Begin native table diagnostic projection. */
static void TableError(void *opaque, int32_t code, int32_t value, const unsigned char *bytes, size_t length)
{ espeak_rs_compiler_diagnostic_table(opaque, code, value, bytes, length); }
/* End native table diagnostic projection. */
static int TableWrite(void *opaque, const unsigned char *bytes, size_t length)
{
	CompileContext *ctx = opaque;
	return espeak_rs_compiler_output_write(ctx->output_owner, RUST_OUTPUT_TABLE, bytes, length);
}
static RustCompilerTableResult TableCall(CompileContext *ctx, uint32_t operation, const char *name, int control)
{
	RustCompilerTables request = {ctx->table_owner, ctx, name, operation == RUST_TABLE_IMPORT ? ctx->phoneme_out : NULL, ctx->linenum, control, TableBase, TableError, TableWrite};
	RustCompilerTableResult result = {0};
	int status = espeak_rs_compiler_tables_call(&request, operation, &result);
	if (status) { result.status = status; if (!ctx->table_status) ctx->table_status = status; error_from_status(ctx, status, "phoneme tables"); }
	else { ctx->phoneme_tab2 = result.current; ctx->n_phoneme_tabs = result.tables; ctx->n_phcodes = result.count; }
	return result;
}
static int LookupPhoneme(CompileContext *ctx, const char *string, int control)
{
	RustCompilerTableResult result = TableCall(ctx, RUST_TABLE_LOOKUP, string, control);
	return result.status ? -1 : result.value;
}

static void SourceClasses(unsigned char *spaces,unsigned char *digits)
{
 for (int i=0;i<256;i++) {spaces[i]=isspace(i)!=0;digits[i]=isdigit(i)!=0;}
}
static CompilerInput SourceOpen(CompileContext *ctx, const char *path, size_t depth, int text)
{
 int file_error=0;
 RustCompilerSourceOpen request={ctx,path,N_PATH_BUF,depth,text,SourceClasses};
 int input=espeak_rs_compiler_context_source(&request,&file_error);
	if (!input) errno = file_error;
	return input;
}
#define SourceEof(ctx) espeak_rs_compiler_source_eof((ctx)->source_pool, (ctx)->f_in)
/* Content reads are complete before open returns; the session retains bytes. */
#define SourceClose(ctx) ((void)(ctx))
#define SourceRewind(ctx) espeak_rs_compiler_source_rewind((ctx)->source_pool, (ctx)->f_in)
#define SourceLine(ctx, buffer, width) espeak_rs_compiler_source_line((ctx)->source_pool, (ctx)->f_in, (unsigned char *)(buffer), (width))


static int ItemLookup(void *opaque,int32_t line,const unsigned char *bytes,size_t length)
{CompileContext *ctx=opaque;char name[256];if(length>=sizeof(name))return -1;memcpy(name,bytes,length);name[length]=0;ctx->linenum=line;return LookupPhoneme(ctx,name,2);}
/* Begin native item diagnostic projection. */
static void ItemError(void *opaque,int32_t line,const unsigned char *bytes,size_t length)
{ espeak_rs_compiler_diagnostic_item(opaque,line,bytes,length); }
/* End native item diagnostic projection. */
static int NextItem(CompileContext *ctx,int type)
{
 RustCompilerItem d={ctx->source_pool,ctx->f_in,type,ctx->linenum,ctx->item_terminator,(const unsigned char *)ctx->item_string,ctx,ItemLookup,ItemError};RustItemResult result;
 int status=espeak_rs_compiler_item(&d,&result);
 if(status){if(!ctx->program_status)ctx->program_status=status;ctx->item_type=-1;return -2;}
 ctx->linenum=result.line;ctx->item_type=result.token.kind;ctx->item_terminator=result.token.terminator;memcpy(ctx->item_string,result.token.text,256);
 return result.token.value;
}

static void UngetItem(CompileContext *ctx)
{
	ctx->linenum = espeak_rs_compiler_source_unget(ctx->source_pool, ctx->f_in);
}

/* Begin native resource diagnostic projection. */
static void ResourceError(void *opaque, int32_t code, int32_t a, int32_t b, const unsigned char *bytes, size_t length)
{ espeak_rs_compiler_diagnostic_resource(opaque, code, a, b, bytes, length); }
/* End native resource diagnostic projection. */

static espeak_ng_STATUS LoadDataFile(CompileContext *ctx, const char *path, int control, int *addr)
{
/* Begin native resource admission projection. */
	RustNativeCompilerResource resource = {
		ctx, path, RustWavegenState(), stderr, N_PATH_BUF, samplerate, control, ResourceError
	};
	return espeak_rs_compiler_resource_native(&resource,addr);
/* End native resource admission projection. */
}

/* Begin native compiler token bridge. */
static int ProgramNext(void *opaque, int32_t type, RustProgramToken *token)
{
 CompileContext *ctx = opaque;
 if (ctx->program_status) return ctx->program_status;
 token->value = NextItem(ctx, type); token->kind = ctx->item_type;
 token->terminator = ctx->item_terminator;
 memcpy(token->text, ctx->item_string, sizeof(token->text)); return ctx->program_status;
}
/* End native compiler token bridge. */
static void ProgramUnget(void *opaque) { UngetItem(opaque); }
/* Begin native program diagnostic projection. */
static void ProgramError(void *opaque, const unsigned char *bytes, size_t length)
{ espeak_rs_compiler_diagnostic_text(opaque,bytes,length); }
/* End native program diagnostic projection. */
static int ProgramLookup(void *opaque, const unsigned char *bytes, size_t length)
{ char name[256]; if (length >= sizeof(name)) return -1; memcpy(name, bytes, length); name[length] = 0; return LookupPhoneme(opaque, name, 2); }
static int ProgramLoad(void *opaque, const unsigned char *bytes, size_t length, int32_t control)
{ char name[256]; int addr = 0; if (length >= sizeof(name)) return 0; memcpy(name, bytes, length); name[length] = 0; LoadDataFile(opaque, name, control, &addr); return addr; }
static int64_t ProgramPosition(void *opaque)
{ CompileContext *ctx = opaque; espeak_rs_compiler_output_flush(ctx->output_owner, RUST_OUTPUT_INDEX); return espeak_rs_compiler_output_position(ctx->output_owner, RUST_OUTPUT_INDEX); }
static int ProgramWrite(void *opaque, int32_t log, const unsigned char *bytes, size_t length)
{ CompileContext *ctx = opaque; return espeak_rs_compiler_output_write(ctx->output_owner, log ? RUST_OUTPUT_LOG : RUST_OUTPUT_INDEX, bytes, length); }

static PHONEME_TAB *FindPhoneme(CompileContext *ctx, const char *string)
{
	return TableCall(ctx, RUST_TABLE_FIND, string, 0).found;
}


static int DefinitionEof(void *opaque)
{
	CompileContext *ctx = opaque;
	return SourceEof(ctx);
}
static int DefinitionLookup(void *opaque,const unsigned char *bytes,size_t length,int32_t control)
{ char name[256]; if(length>=sizeof(name))return -1;memcpy(name,bytes,length);name[length]=0;return LookupPhoneme(opaque,name,control); }
static int DefinitionRow(void *opaque,int32_t code,PHONEME_TAB *row)
{ CompileContext *ctx=opaque;if(code<0||code>255||!ctx->phoneme_tab2)return EINVAL;*row=ctx->phoneme_tab2[code];return 0; }
static int DefinitionProject(void *opaque,const PHONEME_TAB *row)
{ CompileContext *ctx=opaque;ctx->definition_row=*row;ctx->phoneme_out=&ctx->definition_row;if(ctx->definition_phoneme){ctx->phoneme_tab2[row->code]=*row;}return 0; }
static int DefinitionFind(void *opaque,const unsigned char *bytes,size_t length,PHONEME_TAB *row)
{ char name[256];if(length>=sizeof(name))return -EINVAL;memcpy(name,bytes,length);name[length]=0;PHONEME_TAB *found=FindPhoneme(opaque,name);if(!found)return 0;*row=*found;return 1; }
static int DefinitionImport(void *opaque,const unsigned char *bytes,size_t length,PHONEME_TAB *row)
{ CompileContext *ctx=opaque;char name[256];if(length>=sizeof(name))return EINVAL;memcpy(name,bytes,length);name[length]=0;RustCompilerTables d={ctx->table_owner,ctx,name,row,ctx->linenum,0,TableBase,TableError,TableWrite};RustCompilerTableResult result={0};return espeak_rs_compiler_tables_call(&d,RUST_TABLE_IMPORT,&result); }
static void DefinitionUnknown(void *opaque,const unsigned char *bytes,size_t length)
{ char name[256];if(length>=sizeof(name))return;memcpy(name,bytes,length);name[length]=0;error_from_status(opaque,ENS_UNKNOWN_PHONEME_FEATURE,name); }
static int CompilePhoneme(CompileContext *ctx,int compile_phoneme)
{
 unsigned char alnum[256];for(int i=0;i<256;i++)alnum[i]=isalnum((unsigned char)i)!=0;
 ctx->definition_phoneme=compile_phoneme;
 RustCompilerProgram program={ctx->program_owner,ctx,NULL,0,(ctx->output_mask&(1u<<RUST_OUTPUT_LOG))!=0,0,ProgramNext,ProgramUnget,ProgramError,ProgramLookup,ProgramLoad,ProgramPosition,ProgramWrite};
 RustCompilerDefinition d={program,alnum,DefinitionEof,DefinitionLookup,DefinitionRow,DefinitionProject,DefinitionFind,DefinitionImport,DefinitionUnknown};RustDefinitionResult result={0};
 int status=espeak_rs_compiler_definition(&d,compile_phoneme,&result);
 if(status&&!ctx->program_status)ctx->program_status=status;
 return 0;
}

static void WritePhonemeTables(CompileContext *ctx) { TableCall(ctx, RUST_TABLE_PACK, NULL, 0); }
static void EndPhonemeTable(CompileContext *ctx) { TableCall(ctx, RUST_TABLE_FINISH, NULL, 0); }
static void StartPhonemeTable(CompileContext *ctx, const char *name) { TableCall(ctx, RUST_TABLE_START, name, 0); }

static int FilesReset(void *opaque)
{CompileContext *ctx=opaque;ctx->linenum=1;ctx->count_references=ctx->duplicate_references=ctx->count_frames=0;return 0;}
static int FilesEof(void *opaque){CompileContext *ctx=opaque;return SourceEof(ctx);}
static int FilesFrame(void *opaque,RustCompilerFrame *frame)
{CompileContext *ctx=opaque;frame->file=ctx->f_in;frame->line=ctx->linenum;memcpy(frame->name,ctx->current_fname,80);return 0;}
static int FilesProject(void *opaque,const RustCompilerFrame *frame)
{CompileContext *ctx=opaque;ctx->f_in=frame->file;ctx->linenum=frame->line;memcpy(ctx->current_fname,frame->name,80);return 0;}
static int FilesOpen(void *opaque,const unsigned char *bytes,size_t length,size_t depth)
{char path[N_PATH_BUF];if(length>=sizeof(path))return -EINVAL;memcpy(path,bytes,length);path[length]=0;return SourceOpen(opaque,path,depth,0);}
static int FilesClose(void *opaque){CompileContext *ctx=opaque;SourceClose(ctx);return 0;}
static int FilesTables(void *opaque){return ((CompileContext *)opaque)->n_phoneme_tabs;}
static int FilesEnd(void *opaque){CompileContext *ctx=opaque;EndPhonemeTable(ctx);return ctx->table_status;}
static int FilesStart(void *opaque,const unsigned char *bytes,size_t length)
{CompileContext *ctx=opaque;char name[256];if(length>=sizeof(name))return EINVAL;memcpy(name,bytes,length);name[length]=0;StartPhonemeTable(ctx,name);return ctx->table_status;}
static int FilesCompile(void *opaque,int32_t phoneme)
{CompileContext *ctx=opaque;CompilePhoneme(ctx,phoneme);return ctx->program_status;}
static int FilesTerminate(void *opaque){CompileContext *ctx=opaque;return TableCall(ctx,RUST_TABLE_TERMINATE,NULL,0).status;}
static void CompilePhonemeFiles(CompileContext *ctx)
{
 RustCompilerFiles d={ctx,(const unsigned char *)ctx->phsrc,N_PATH_BUF,FilesReset,FilesEof,ProgramNext,FilesFrame,FilesProject,FilesOpen,FilesClose,FilesTables,FilesEnd,FilesStart,FilesCompile,FilesTerminate,ProgramError};int32_t completed;
 int status=espeak_rs_compiler_files(&d,&completed);if(status&&!ctx->program_status)ctx->program_status=status;
}
