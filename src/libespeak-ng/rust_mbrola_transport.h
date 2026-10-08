/* Serialized legacy adapter; native Rust callers own independent transports. */
#ifndef ESPEAK_RUST_MBROLA_TRANSPORT_H
#define ESPEAK_RUST_MBROLA_TRANSPORT_H
#include <stddef.h>
typedef struct espeak_rs_mbr_transport espeak_rs_mbr_transport;
espeak_rs_mbr_transport *espeak_rs_mbr_transport_create(void);
void espeak_rs_mbr_transport_destroy(espeak_rs_mbr_transport *);
int espeak_rs_mbr_queue(espeak_rs_mbr_transport *, const unsigned char *, size_t);
/* Borrow until consume/queue/clear/destroy; caller serializes access. */
size_t espeak_rs_mbr_front(const espeak_rs_mbr_transport *, const unsigned char **);
int espeak_rs_mbr_consume(espeak_rs_mbr_transport *, size_t);
void espeak_rs_mbr_clear(espeak_rs_mbr_transport *, int clear_stderr);
int espeak_rs_mbr_stderr(espeak_rs_mbr_transport *, const unsigned char *, size_t,
                       int eof, unsigned char *, size_t);
int espeak_rs_mbr_sample_rate(const unsigned char *, size_t);
#endif
