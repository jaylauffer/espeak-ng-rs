/* Frozen native lifecycle primitives from 04d4db59. GPL-3.0-or-later. */
/* Begin native compiler lifecycle primitives. */
typedef struct {
 CompileContext *ctx;
 long rate;
 FILE *log;
 espeak_ng_ERROR_CONTEXT *error;
} PhonemeEffects;
static int PhonemeEffect(void *opaque,int operation,int value,const char *source,const char *destination,unsigned char *error_path)
{
 PhonemeEffects *effects=opaque;
 CompileContext *ctx=effects->ctx;
 switch(operation) {
 case RUST_COMPILER_ALLOCATE:
  {int status=0;effects->ctx=espeak_rs_compiler_context_begin(source,N_PATH_BUF,&status);return effects->ctx?0:status;}
 case RUST_COMPILER_TABLES:
  ctx->table_owner=espeak_rs_compiler_tables_begin(); return ctx->table_owner?0:ENOMEM;
 case RUST_COMPILER_PROGRAM:
  ctx->program_owner=espeak_rs_compiler_program_begin(); return ctx->program_owner?0:ENOMEM;
 case RUST_COMPILER_RATE: samplerate=effects->rate; return 0;
 case RUST_COMPILER_RELOAD: LoadPhData(NULL,NULL); return 0;
 case RUST_COMPILER_VOICE: return LoadVoice("",8)?0:ENS_VOICE_NOT_FOUND;
 case RUST_COMPILER_WAVE_INIT: WavegenInit(effects->rate,0); WavegenSetVoice(voice); return 0;
 case RUST_COMPILER_INITIALIZE:
  ctx->error_count=0;ctx->f_errors=effects->log;
  strncpy0(ctx->current_fname,"phonemes",sizeof(ctx->current_fname));return 0;
 case RUST_COMPILER_SOURCE_OPEN:
  ctx->f_in=SourceOpen(ctx,source,0,0);return ctx->f_in?0:errno;
 case RUST_COMPILER_OUTPUT_OPEN: {
  RustCompilerOutput outputs={ctx->phsrc,destination,N_PATH_BUF,version_phdata,samplerate,ctx,OutputFallback};
  RustCompilerOutputResult opened={0};
  int status=espeak_rs_compiler_output_begin(&outputs,&opened);
  if (status) {memcpy(error_path,opened.filename,sizeof(opened.filename));return status;}
  ctx->output_owner=opened.owner;ctx->output_mask=opened.mask;return 0;
 }
 case RUST_COMPILER_BASE_TABLE: ctx->n_phoneme_tabs=0;StartPhonemeTable(ctx,"base");return 0;
 case RUST_COMPILER_COMPILE: CompilePhonemeFiles(ctx);return 0;
 case RUST_COMPILER_END_TABLE: EndPhonemeTable(ctx);return 0;
 case RUST_COMPILER_WRITE_TABLES: WritePhonemeTables(ctx);return 0;
 case RUST_COMPILER_SOURCE_CLOSE: SourceClose(ctx);return 0;
 case RUST_COMPILER_OUTPUT_CLOSE: {
  int status=espeak_rs_compiler_output_end(ctx->output_owner);ctx->output_owner=0;
  return status?status:(ctx->table_status?ctx->table_status:ctx->program_status);
 }
 case RUST_COMPILER_OUTPUT_ERROR: error_from_status(ctx,value,"compiler output");return 0;
 case RUST_COMPILER_CLOSE_LOG:
  if (ctx->f_errors!=stderr && ctx->f_errors!=stdout) fclose(ctx->f_errors);return 0;
 case RUST_COMPILER_WAVE_FINI: WavegenFini();return 0;
 case RUST_COMPILER_CLEANUP: clean_context(ctx);effects->ctx=NULL;return 0;
 default:return EINVAL;
 }
}
static void PhonemeCounts(void *opaque,RustCompilerCounts *counts)
{
 CompileContext *ctx=((PhonemeEffects *)opaque)->ctx;
 *counts=(RustCompilerCounts){ctx->count_references,ctx->duplicate_references,ctx->error_count};
}
static void PhonemeLog(void *opaque,const unsigned char *bytes,size_t length)
{
 fwrite(bytes,1,length,((PhonemeEffects *)opaque)->log);
}
static int PhonemeError(void *opaque,int status,const char *path)
{
 return create_file_error_context(((PhonemeEffects *)opaque)->error,status,path);
}
