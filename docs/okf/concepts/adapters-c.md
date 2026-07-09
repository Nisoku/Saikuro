---
type: concept
title: "C Adapter"
description: "Saikuro adapter for C"
source: "https://nisoku.org/Saikuro/adapters/c/"
path: /adapters/c/
updated: 2026-07-09
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-07-09T20:42:46.325Z"
---
---
title: "C Adapter"
description: "Saikuro adapter for C"
---

The C adapter provides a minimal FFI-friendly header. All arguments and results are JSON strings.

## Installation

```c
#include "saikuro.h"
// link libsaikuro_c.a
```

## Client

```c
#include "saikuro.h"
#include <stdio.h>

int main() {
    saikuro_client_t client = saikuro_client_connect("unix:///tmp/saikuro.sock");

    char* result = saikuro_client_call_json(client, "math.add", "[10, 32]");
    printf("result: %s\n", result);
    saikuro_string_free(result);

    saikuro_client_close(client);
    saikuro_client_free(client);
    return 0;
}
```

## Provider

```c
#include "saikuro.h"

char* add_handler(void* user_data, const char* args_json) {
    return saikuro_string_dup("42");
}

int main() {
    saikuro_provider_t provider = saikuro_provider_new("math");
    saikuro_provider_register(provider, "add", add_handler, NULL);
    saikuro_provider_serve(provider, "unix:///tmp/saikuro.sock");
    saikuro_provider_free(provider);
    return 0;
}
```

## API Reference

See the [C API Reference](./api-reference) for the full function list.
