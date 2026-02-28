/* Minimal iconv stub for Windows MSVC builds.
 * readstat's writer path never calls iconv() with a non-NULL converter,
 * so these definitions are sufficient to compile without libiconv. */
#pragma once
#include <stddef.h>

typedef void *iconv_t;

static inline iconv_t iconv_open(const char *tocode, const char *fromcode) {
    (void)tocode; (void)fromcode;
    return (iconv_t)-1;
}

static inline size_t iconv(iconv_t cd, char **inbuf, size_t *inbytesleft,
                            char **outbuf, size_t *outbytesleft) {
    (void)cd; (void)inbuf; (void)inbytesleft; (void)outbuf; (void)outbytesleft;
    return (size_t)-1;
}

static inline int iconv_close(iconv_t cd) {
    (void)cd;
    return 0;
}
