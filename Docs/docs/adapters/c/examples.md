---
title: "C Adapter Examples"
description: "C ABI usage patterns and ownership-safe examples"
---

## Call flow with explicit ownership

```c
#include "saikuro.h"

int main(void) {
    saikuro_client_t *client = saikuro_client_connect("tcp://127.0.0.1:7700");
    if (!client) {
        char *err = saikuro_last_error_message();
        fprintf(stderr, "connect failed: %s\n", err ? err : "unknown");
        saikuro_string_free(err);
        return 1;
    }

    const char *args_json = "[10, 32]";
    char *result_json = saikuro_client_call_json(client, "math.add", args_json);
    if (!result_json) {
        char *err = saikuro_last_error_message();
        fprintf(stderr, "call failed: %s\n", err ? err : "unknown");
        saikuro_string_free(err);
        saikuro_client_free(client);
        return 1;
    }

    printf("result: %s\n", result_json);
    saikuro_string_free(result_json); /* free API-returned strings */
    saikuro_client_free(client);
    return 0;
}
```

## Provider callback ownership contract

Provider callbacks return ownership of a heap string to the adapter.

```c
static char *add_cb(void *ctx, const char *args_json) {
    (void)ctx;
    (void)args_json;
    return saikuro_string_dup("42"); /* required ownership-safe return */
}
```

Never return stack memory or string literals from provider callbacks.

## Next Steps

- [C Adapter](./)
- [C++ examples](../cpp/examples)
- [Protocol Reference](../../api/)