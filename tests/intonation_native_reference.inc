/* Frozen complete native public intonation host fragments from 14621f7d. GPL-3.0-or-later. */
typedef struct {
	FILE *log;
	espeak_ng_ERROR_CONTEXT *error;
} IntonationEffects;
static void IntonationLog(void *opaque, const unsigned char *bytes, size_t length)
{
	IntonationEffects *effects = opaque;
	fwrite(bytes, 1, length, effects->log);
}
static void IntonationClose(void *opaque)
{
	IntonationEffects *effects = opaque;
	fclose(effects->log);
}
static void IntonationReload(void *opaque)
{
	(void)opaque;
	LoadPhData(NULL, NULL);
}
static int IntonationError(void *opaque, int status, const char *path)
{
	IntonationEffects *effects = opaque;
	return create_file_error_context(effects->error, status, path);
}
espeak_ng_STATUS
espeak_ng_CompileIntonationPath(const char *source_path,
                                const char *destination_path,
                                FILE *log,
                                espeak_ng_ERROR_CONTEXT *context
                                )
{
	if (!log) log = stderr;
	if (!source_path) source_path = path_home;
	if (!destination_path) destination_path = path_home;

#ifdef USE_RUST_CORE
	unsigned char spaces[256], digits[256];
	for (int i = 0; i < 256; i++) {
		spaces[i] = isspace(i) != 0;
		digits[i] = isdigit(i) != 0;
	}
	IntonationEffects effects = { log, context };
	int windows = 0;
#ifdef _WIN32
	windows = 1;
#endif
	RustIntonationCompiler compiler = {
		&effects, source_path, destination_path, spaces, digits, N_PATH_BUF, windows,
		IntonationLog, IntonationClose, IntonationReload, IntonationError
	};
	return espeak_rs_compile_intonation(&compiler);
#endif
}
