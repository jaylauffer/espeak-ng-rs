/* Frozen complete native resource host fragments from edc778d6. GPL-3.0-or-later. */
static void ResourceLog(void *opaque, const unsigned char *bytes, size_t length)
{
	(void)opaque;
	fwrite(bytes, 1, length, stderr);
}
static void ResourceError(void *opaque, int32_t code, int32_t a, int32_t b, const unsigned char *bytes, size_t length)
{
	CompileContext *ctx = opaque;
	char name[4096];
	if (length >= sizeof(name)) return;
	memcpy(name, bytes, length); name[length] = 0;
	switch (code) {
	case 1: error(ctx, "Can't read file: %s", name); break;
	case 2: error(ctx, "File not SPEC or RIFF: %s", name); break;
	case 3: error(ctx, "Bad vowel file: '%s'", name); break;
	case 4: error(ctx, "Can't resample (%d to %d): %s", a, b, name); break;
	case 5: error(ctx, "WAV file is not mono: %s", name); break;
	}
}
static int64_t ResourcePosition(void *opaque)
{
	CompileContext *ctx = opaque;
	return espeak_rs_compiler_output_position(ctx->output_owner, RUST_OUTPUT_DATA);
}
static int ResourceWrite(void *opaque, int32_t destination, const unsigned char *bytes, size_t length)
{
	CompileContext *ctx = opaque;
	return espeak_rs_compiler_output_write(ctx->output_owner,
	       destination == 0 ? RUST_OUTPUT_DATA : RUST_OUTPUT_MANIFEST, bytes, length);
}
static espeak_ng_STATUS LoadDataFile(CompileContext *ctx, const char *path, int control, int *addr)
{
#ifdef USE_RUST_CORE
	if (!ctx->resource_owner) ctx->resource_owner = espeak_rs_compiler_resources_begin();
	if (!ctx->resource_owner) return ENOMEM;
	int32_t lower[256]; unsigned char space[256];
	for (int i = 0; i < 256; i++) { lower[i] = tolower(i); space[i] = isspace(i) != 0; }
	RustCompilerResource resource = {
		ctx->resource_owner, ctx, ctx->phsrc, path, RustWavegenState(), lower, space, N_PATH_BUF,
		samplerate, control, ctx->phoneme_out ? ctx->phoneme_out->mnemonic : 0, ctx->n_phoneme_tabs - 1,
		(unsigned char)localeconv()->decimal_point[0], {*addr, ctx->count_references, ctx->duplicate_references, ctx->count_frames},
		ResourceLog, ResourceError, ResourcePosition, ResourceWrite
	};
	RustResourceProgress result = resource.initial;
	espeak_ng_STATUS status = espeak_rs_compiler_resource_load(&resource, &result);
	*addr = result.address; ctx->count_references = result.references;
	ctx->duplicate_references = result.duplicates; ctx->count_frames = result.frames;
	return status;
#endif
}
