#include "saikuro.h"

//
// Byte-level UTF-8 character statistics.
//

static int write_int(int v, char *buf)
{
    char tmp[16];
    char *p = tmp + 16;
    int neg = 0;
    unsigned int uv;
    if (v < 0)
    {
        neg = 1;
        uv = (unsigned int)(-(v + 1)) + 1;
    }
    else
    {
        uv = (unsigned int)v;
    }
    do
    {
        *--p = '0' + (uv % 10);
        uv /= 10;
    } while (uv > 0);
    if (neg)
        *--p = '-';
    int n = 0;
    while (p < tmp + 16)
        buf[n++] = *p++;
    buf[n] = '\0';
    return n;
}

static char *wc(char *p, char c)
{
    *p++ = c;
    return p;
}
static char *ws(char *p, const char *s)
{
    while (*s)
        *p++ = *s++;
    return p;
}

int compute_stats(const char *text, char *out_buf, int out_capacity)
{
    int bytes = 0, chars = 0, ascii = 0, non_ascii = 0;
    if (text)
    {
        while (text[bytes])
        {
            unsigned char c = (unsigned char)text[bytes];
            chars++;
            if (c < 0x80)
            {
                ascii++;
                bytes++;
            }
            else if (c < 0xE0)
            {
                non_ascii++;
                bytes += (text[bytes+1] && (text[bytes+1] & 0xC0) == 0x80) ? 2 : 1;
            }
            else if (c < 0xF0)
            {
                non_ascii++;
                int len = 1;
                while (len < 3 && text[bytes+len] && (text[bytes+len] & 0xC0) == 0x80) len++;
                bytes += len;
            }
            else if (c < 0xF8)
            {
                non_ascii++;
                int len = 1;
                while (len < 4 && text[bytes+len] && (text[bytes+len] & 0xC0) == 0x80) len++;
                bytes += len;
            }
            else
            {
                bytes++;
            }
        }
    }
    if (out_capacity < 128)
        return 0;
    char buf[16];
    char *p = out_buf;
    p = wc(p, '{');
    p = ws(p, "\"bytes\":");
    write_int(bytes, buf);
    p = ws(p, buf);
    p = wc(p, ',');
    p = ws(p, "\"chars\":");
    write_int(chars, buf);
    p = ws(p, buf);
    p = wc(p, ',');
    p = ws(p, "\"ascii\":");
    write_int(ascii, buf);
    p = ws(p, buf);
    p = wc(p, ',');
    p = ws(p, "\"non_ascii\":");
    write_int(non_ascii, buf);
    p = ws(p, buf);
    p = wc(p, '}');
    *p = '\0';
    return (int)(p - out_buf);
}

//
// Extract first quoted string from JSON array: ["hello world"] -> "hello world"
//

static const char *extract_first_string(const char *json, char *buf, int cap)
{
    const char *p = json;
    while (*p && *p != '"')
        p++;
    if (!*p)
        return NULL;
    p++;
    int i = 0;
    while (*p && *p != '"' && i < cap - 1)
    {
        if (*p == '\\' && *(p + 1))
        {
            p++;
            switch (*p)
            {
                case 'n': buf[i++] = '\n'; break;
                case 't': buf[i++] = '\t'; break;
                case 'r': buf[i++] = '\r'; break;
                case '"': buf[i++] = '"'; break;
                case '\\': buf[i++] = '\\'; break;
                default: buf[i++] = *p; break;
            }
            p++;
        }
        else
        {
            buf[i++] = *p++;
        }
    }
    buf[i] = '\0';
    return buf;
}

//
// Provider handler for "c.stats"
//   args_json: ["hello world"]
//   result:    {"bytes":11,"chars":11,"ascii":11,"non_ascii":0}
//

static char *handle_stats(void *user_data, const char *args_json)
{
    (void)user_data;
    char text[4096];
    if (!extract_first_string(args_json, text, sizeof(text)))
    {
        return saikuro_string_dup("{\"error\":\"missing text argument\"}");
    }
    char result[512];
    compute_stats(text, result, sizeof(result));
    return saikuro_string_dup(result);
}

//
// Exported entry point called from TypeScript as start_c_provider(channel)
//

__attribute__((used, visibility("default"))) void saikuro_c_start_provider(const char *channel)
{
    char address[256];
    {
        const char prefix[] = "wasm-host://";
        size_t i = 0;
        const char *p;
        for (p = prefix; *p && i < sizeof(address) - 1; p++)
            address[i++] = *p;
        for (p = channel; *p && i < sizeof(address) - 1; p++)
            address[i++] = *p;
        address[i] = '\0';
    }

    saikuro_provider_t provider = saikuro_provider_new("c");
    saikuro_provider_register_with_schema(provider, "stats", handle_stats, NULL, 1, "Any");
    saikuro_provider_serve(provider, address);
}
