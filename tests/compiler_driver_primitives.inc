/* Frozen test fixture: the C host the native compilers received before
 * their primitives moved to Rust (rust/compiler_engine_compat.rs), copied from
 * src/libespeak-ng/compiledata.c as the test generator extracted it.
 * SPDX-License-Identifier: GPL-3.0-or-later */
/* Begin native compiler lifecycle primitives. */
typedef struct {
 long rate;
 FILE *log;
 espeak_ng_ERROR_CONTEXT *error;
} PhonemeEffects;
static int PhonemePrimitive(void *opaque,CompileContext *ctx,int operation,int value)
{
 PhonemeEffects *effects=opaque;
 switch(operation) {
 case RUST_COMPILER_RATE: samplerate=effects->rate; return 0;
 case RUST_COMPILER_RELOAD: LoadPhData(NULL,NULL); return 0;
 case RUST_COMPILER_VOICE: return LoadVoice("",8)?0:ENS_VOICE_NOT_FOUND;
 case RUST_COMPILER_WAVE_INIT: WavegenInit(effects->rate,0); WavegenSetVoice(voice); return 0;
 case RUST_COMPILER_OUTPUT_OPEN: return samplerate;
 case RUST_COMPILER_BASE_TABLE: StartPhonemeTable(ctx,"base");return 0;
 case RUST_COMPILER_COMPILE: CompilePhonemeFiles(ctx);return 0;
 case RUST_COMPILER_END_TABLE: EndPhonemeTable(ctx);return 0;
 case RUST_COMPILER_WRITE_TABLES: WritePhonemeTables(ctx);return 0;
 case RUST_COMPILER_OUTPUT_ERROR: error_from_status(ctx,value,"compiler output");return 0;
 case RUST_COMPILER_CLOSE_LOG:
  if (ctx->f_errors!=stderr && ctx->f_errors!=stdout) fclose(ctx->f_errors);return 0;
 case RUST_COMPILER_WAVE_FINI: WavegenFini();return 0;
 default:return EINVAL;
 }
}
static void PhonemeLog(void *opaque,const unsigned char *bytes,size_t length)
{
 fwrite(bytes,1,length,((PhonemeEffects *)opaque)->log);
}
static int PhonemeError(void *opaque,int status,const char *path)
{
 return create_file_error_context(((PhonemeEffects *)opaque)->error,status,path);
}
