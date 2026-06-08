#include "saikuro.h"

//
// N-gram frequency analysis.
//

extern "C"
{
    extern void *malloc(unsigned long n);
    extern void free(void *p);
    extern unsigned long strlen(const char *s);
    extern void *memcpy(void *dst, const void *src, unsigned long n);
    static inline int strncmp(const char *a, const char *b, unsigned long n) {
        for (unsigned long i = 0; i < n; i++) {
            if (a[i] != b[i]) return (unsigned char)a[i] - (unsigned char)b[i];
            if (a[i] == '\0') return 0;
        }
        return 0;
    }
}

static bool is_alnum(char c)
{
    return (c >= 'a' && c <= 'z') ||
           (c >= 'A' && c <= 'Z') ||
           (c >= '0' && c <= '9');
}

static char to_lower(char c)
{
    return (c >= 'A' && c <= 'Z') ? (c + 32) : c;
}

struct KV
{
    char key[64];
    int count;
};

static KV *find_slot(KV *table, int table_len, const char *key)
{
    for (int i = 0; i < table_len; i++)
    {
        if (table[i].count == 0)
            return &table[i];
        if (strncmp(table[i].key, key, 64) == 0)
            return &table[i];
    }
    return nullptr;
}

static int tokenize(const char *text, char tokens[][64], int max_tokens)
{
    int count = 0;
    int pos = 0;
    while (text[pos] && count < max_tokens)
    {
        while (text[pos] && !is_alnum(text[pos]))
            pos++;
        if (!text[pos])
            break;
        int wi = 0;
        while (text[pos] && is_alnum(text[pos]) && wi < 63)
        {
            tokens[count][wi++] = to_lower(text[pos++]);
        }
        tokens[count][wi] = '\0';
        if (wi > 0)
            count++;
    }
    return count;
}

static void join_words(const char *a, const char *b, char *buf, int cap)
{
    int i = 0;
    while (*a && i < cap - 1)
        buf[i++] = *a++;
    if (i < cap - 1)
        buf[i++] = ' ';
    while (*b && i < cap - 1)
        buf[i++] = *b++;
    buf[i] = '\0';
}

static int write_json_ngrams(KV *table, int table_len, int top_n,
                             char *out, int cap)
{
    int order[256];
    int n = 0;
    for (int i = 0; i < table_len && table[i].count > 0; i++)
    {
        if (n < top_n)
        {
            order[n++] = i;
            for (int j = n - 1; j > 0; j--)
            {
                if (table[order[j]].count <= table[order[j - 1]].count)
                    break;
                int t = order[j];
                order[j] = order[j - 1];
                order[j - 1] = t;
            }
        }
        else if (table[i].count > table[order[top_n - 1]].count)
        {
            order[top_n - 1] = i;
            for (int j = top_n - 1; j > 0; j--)
            {
                if (table[order[j]].count <= table[order[j - 1]].count)
                    break;
                int t = order[j];
                order[j] = order[j - 1];
                order[j - 1] = t;
            }
        }
    }

    int pos = 0;
    auto wc = [&](char c)
    { if (pos < cap) out[pos++] = c; };
    auto ws = [&](const char *s)
    { while (*s && pos < cap) out[pos++] = *s++; };

    wc('[');
    for (int i = 0; i < n; i++)
    {
        if (i > 0)
            wc(',');
        wc('[');
        wc('"');
        ws(table[order[i]].key);
        wc('"');
        wc(',');
        char tmp[16];
        int ti = 0;
        int v = table[order[i]].count;
        if (v < 0)
        {
            wc('-');
            v = -v;
        }
        char t2[16];
        int t2i = 0;
        do
        {
            t2[t2i++] = '0' + (v % 10);
            v /= 10;
        } while (v > 0);
        for (int k = t2i - 1; k >= 0; k--)
            wc(t2[k]);
        wc(']');
    }
    wc(']');
    if (pos < cap)
        out[pos] = '\0';
    return pos;
}

extern "C" int compute_ngrams(const char *text, int top_n,
                              char *out_buf, int out_capacity)
{
    char tokens[512][64];
    int n_tokens = tokenize(text, tokens, 512);

    KV bigrams[256];
    KV trigrams[256];
    for (int i = 0; i < 256; i++)
    {
        bigrams[i].count = 0;
        trigrams[i].count = 0;
    }

    for (int i = 0; i < n_tokens - 1; i++)
    {
        char key[64];
        join_words(tokens[i], tokens[i + 1], key, 64);
        KV *slot = find_slot(bigrams, 256, key);
        if (slot) {
            if (slot->count == 0)
                memcpy(slot->key, key, 64);
            slot->count++;
        }
    }

    for (int i = 0; i < n_tokens - 2; i++)
    {
        char key[64];
        join_words(tokens[i], tokens[i + 1], key, 64);
        int klen = strlen(key);
        int pos = klen;
        if (pos < 63)
            key[pos++] = ' ';
        const char *w = tokens[i + 2];
        while (*w && pos < 63)
            key[pos++] = *w++;
        key[pos] = '\0';

        KV *slot = find_slot(trigrams, 256, key);
        if (slot) {
            if (slot->count == 0)
                memcpy(slot->key, key, 64);
            slot->count++;
        }
    }

    int pos = 0;
    auto wc = [&](char c)
    { if (pos < out_capacity) out_buf[pos++] = c; };
    auto ws = [&](const char *s)
    { while (*s && pos < out_capacity) out_buf[pos++] = *s++; };

    char tmp[4096];
    int tpos = 0;

    wc('{');
    ws("\"bigrams\":");
    tpos = write_json_ngrams(bigrams, 256, top_n, tmp, 4096);
    tmp[tpos] = '\0';
    ws(tmp);
    wc(',');
    ws("\"trigrams\":");
    tpos = write_json_ngrams(trigrams, 256, top_n, tmp, 4096);
    tmp[tpos] = '\0';
    ws(tmp);
    wc('}');
    if (pos < out_capacity)
        out_buf[pos] = '\0';

    return pos;
}

//
// Parse JSON array ["text", 6] -> extract text and top_n
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
            p++;
        buf[i++] = *p++;
    }
    buf[i] = '\0';
    return buf;
}

static int extract_int_after_comma(const char *json, int default_val)
{
    const char *p = json;
    // Find the last comma in the array
    const char *comma = NULL;
    while (*p)
    {
        if (*p == ',')
            comma = p;
        p++;
    }
    if (!comma)
        return default_val;
    comma++;
    while (*comma == ' ' || *comma == '\t')
        comma++;
    int val = 0;
    int neg = 0;
    if (*comma == '-')
    {
        neg = 1;
        comma++;
    }
    while (*comma >= '0' && *comma <= '9')
    {
        val = val * 10 + (*comma - '0');
        comma++;
    }
    return neg ? -val : val;
}

//
// Provider handler for "cpp.ngrams"
//   args_json: ["hello world", 6]
//   result:    {"bigrams":[[...],...],"trigrams":[[...],...]}
//

static char *handle_ngrams(void *user_data, const char *args_json)
{
    (void)user_data;
    char text[4096];
    if (!extract_first_string(args_json, text, sizeof(text)))
    {
        return saikuro_string_dup("{\"error\":\"missing text argument\"}");
    }
    int top_n = extract_int_after_comma(args_json, 6);

    char result[16384];
    compute_ngrams(text, top_n, result, sizeof(result));
    return saikuro_string_dup(result);
}

//
// Exported entry point called from TypeScript as start_cpp_provider(channel)
//

extern "C"
{
    __attribute__((used, visibility("default"))) void saikuro_cpp_start_provider(const char *channel)
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

        saikuro_provider_t provider = saikuro_provider_new("cpp");
        saikuro_provider_register_with_schema(provider, "ngrams", handle_ngrams, NULL, 2, "Any");
        saikuro_provider_serve(provider, address);
    }
}
