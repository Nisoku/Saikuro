#include <ctype.h>
#include <emscripten/emscripten.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static char *make_json_result(int bytes, int chars, int ascii, int non_ascii) {
    char buffer[256];
    int len = snprintf(buffer, sizeof(buffer),
        "{\"bytes\":%d,\"chars\":%d,\"ascii\":%d,\"non_ascii\":%d}",
        bytes, chars, ascii, non_ascii);
    char *out = (char *)malloc((size_t)len + 1);
    if (!out) return NULL;
    memcpy(out, buffer, (size_t)len + 1);
    return out;
}

EMSCRIPTEN_KEEPALIVE
char *insight_c_stats(const char *input) {
    if (!input) {
        return make_json_result(0, 0, 0, 0);
    }
    int bytes = (int)strlen(input);
    int chars = 0;
    int ascii = 0;
    int non_ascii = 0;
    for (int i = 0; i < bytes; chars++) {
        unsigned char c = (unsigned char)input[i];
        if (c < 128) {
            ascii++;
            i++;
        } else if ((c & 0xE0) == 0xC0) {
            non_ascii++;
            i += 2;
        } else if ((c & 0xF0) == 0xE0) {
            non_ascii++;
            i += 3;
        } else if ((c & 0xF8) == 0xF0) {
            non_ascii++;
            i += 4;
        } else {
            non_ascii++;
            i++;
        }
    }
    return make_json_result(bytes, chars, ascii, non_ascii);
}

EMSCRIPTEN_KEEPALIVE
void insight_c_free(char *ptr) {
    if (ptr) free(ptr);
}
