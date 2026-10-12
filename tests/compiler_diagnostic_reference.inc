/* Frozen previous complete native diagnostic fragments from 82abb706.
 * Original bodies unchanged; prefix/body/count and structured callbacks.
 * SPDX-License-Identifier: GPL-3.0-or-later */
static void error(CompileContext *ctx, const char *format, ...)
{
	va_list args;
	va_start(args, format);

	fprintf(ctx->f_errors, "%s(%d): ", ctx->current_fname, ctx->linenum-1);
	vfprintf(ctx->f_errors, format, args);
	fprintf(ctx->f_errors, "\n");
	ctx->error_count++;

	va_end(args);
}

static void error_from_status(CompileContext *ctx, espeak_ng_STATUS status, const char *context)
{
	char message[512];
	espeak_ng_GetStatusCodeMessage(status, message, sizeof(message));
	if (context)
		error(ctx, "%s: '%s'.", message, context);
	else
		error(ctx, "%s.", message);
}

static void TableError(void *opaque, int32_t code, int32_t value, const unsigned char *bytes, size_t length)
{
	CompileContext *ctx = opaque; char name[256];
	if (length >= sizeof(name)) return;
	memcpy(name, bytes, length); name[length] = 0;
	switch (code) {
	case 1: error(ctx, "Bad phoneme name '%s'", name); break;
	case 2: error(ctx, "compile: unknown phoneme table: '%s'", name); break;
	case 3: error(ctx, "Phoneme reference not found: '%s'", name); break;
	case 4: error(ctx, "Phoneme [%s] not declared, referenced at line %d", name, value); ctx->error_count++; break;
	case 5: error(ctx, "Too many phonemetables"); break;
	case 6: error(ctx, "Out of memory"); break;
	case 7: error(ctx, "Can't find base phonemetable '%s'", name); break;
	case 8: error(ctx, "Cannot find phoneme '%s' to import.", name); break;
	case 9: error(ctx, "Phoneme import will override set properties."); break;
	}
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
static void ItemError(void *opaque,int32_t line,const unsigned char *bytes,size_t length)
{CompileContext *ctx=opaque;ctx->linenum=line;error(ctx,"%.*s",(int)length,bytes);}
static void ProgramError(void *opaque, const unsigned char *bytes, size_t length)
{ error(opaque, "%.*s", (int)length, (const char *)bytes); }
