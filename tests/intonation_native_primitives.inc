/* Frozen test fixture: the C host the native compilers received before
 * their primitives moved to Rust (rust/compiler_engine_compat.rs), copied from
 * src/libespeak-ng/compiledata.c as the test generator extracted it.
 * SPDX-License-Identifier: GPL-3.0-or-later */
/* Begin native intonation projection. */
static void IntonationReload(void *opaque)
{
	(void)opaque;
	LoadPhData(NULL, NULL);
}
static int IntonationError(void *opaque, int status, const char *path)
{
	return create_file_error_context(opaque, status, path);
}
