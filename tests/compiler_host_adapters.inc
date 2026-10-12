/* Frozen ec00d27e native adapters, test-only after Rust host adoption.
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
/* Begin native table diagnostic projection. */
static void TableError(void *opaque, int32_t code, int32_t value, const unsigned char *bytes, size_t length)
{ espeak_rs_compiler_diagnostic_table(opaque, code, value, bytes, length); }
/* End native table diagnostic projection. */
/* Begin native item diagnostic projection. */
static void ItemError(void *opaque,int32_t line,const unsigned char *bytes,size_t length)
{ espeak_rs_compiler_diagnostic_item(opaque,line,bytes,length); }
/* End native item diagnostic projection. */
/* Begin native program diagnostic projection. */
static void ProgramError(void *opaque, const unsigned char *bytes, size_t length)
{ espeak_rs_compiler_diagnostic_text(opaque,bytes,length); }
/* End native program diagnostic projection. */
/* Begin native resource diagnostic projection. */
static void ResourceError(void *opaque, int32_t code, int32_t a, int32_t b, const unsigned char *bytes, size_t length)
{ espeak_rs_compiler_diagnostic_resource(opaque, code, a, b, bytes, length); }
/* End native resource diagnostic projection. */
/* Begin native resource admission projection. */
	RustNativeCompilerResource resource = {
		ctx, path, RustWavegenState(), stderr, N_PATH_BUF, samplerate, control, ResourceError
	};
	return espeak_rs_compiler_resource_native(&resource,addr);
/* End native resource admission projection. */
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
