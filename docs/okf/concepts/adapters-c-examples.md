---
type: concept
title: "C Examples"
description: "C adapter usage patterns"
source: "https://nisoku.org/Saikuro/docs/adapters/c/examples/"
path: /adapters/c/examples/
updated: 2026-07-21
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-07-21T10:55:59.269Z"
---
---
title: "C Examples"
description: "C adapter usage patterns"
---

## Math Provider

```c
#include "saikuro.h"
#include <stdio.h>

char* add_handler(void* user_data, const char* args_json) {
    // args_json is a JSON array, e.g. "[10, 32]"
    // Return a JSON result as a heap string
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

## Client

```c
#include "saikuro.h"
#include <stdio.h>

int main() {
    saikuro_client_t client = saikuro_client_connect("unix:///tmp/saikuro.sock");

    char* result = saikuro_client_call_json(client, "math.add", "[10, 32]");
    printf("10 + 32 = %s\n", result);
    saikuro_string_free(result);

    saikuro_client_close(client);
    saikuro_client_free(client);
    return 0;
}
```

## Stream

```c
#include "saikuro.h"
#include <stdio.h>

int main() {
    saikuro_client_t client = saikuro_client_connect("unix:///tmp/saikuro.sock");

    saikuro_stream_t stream = saikuro_client_stream_json(client, "events.tick", "[5]");
    char* item = NULL;
    int done = 0;
    while (saikuro_stream_next_json(stream, &item, &done) == 0 && !done) {
        printf("tick: %s\n", item);
        saikuro_string_free(item);
    }
    saikuro_stream_free(stream);

    saikuro_client_close(client);
    saikuro_client_free(client);
    return 0;
}
```

## Channel

```c
#include "saikuro.h"
#include <stdio.h>

int main() {
    saikuro_client_t client = saikuro_client_connect("unix:///tmp/saikuro.sock");

    saikuro_channel_t ch = saikuro_client_channel_json(client, "chat.room", "[\"lobby\"]");
    saikuro_channel_send_json(ch, "\"hello\"");

    char* reply = NULL;
    int done = 0;
    saikuro_channel_next_json(ch, &reply, &done);
    printf("reply: %s\n", reply);
    saikuro_string_free(reply);

    saikuro_channel_close(ch);
    saikuro_channel_free(ch);
    saikuro_client_close(client);
    saikuro_client_free(client);
    return 0;
}
```

## Batch

```c
#include "saikuro.h"
#include <stdio.h>

int main() {
    saikuro_client_t client = saikuro_client_connect("unix:///tmp/saikuro.sock");

    // calls_json is a JSON array of [target, args] tuples
    char* results = saikuro_client_batch_json(client,
        "[[\"math.add\", [1, 2]], [\"math.multiply\", [3, 4]]]");
    printf("batch results: %s\n", results);
    saikuro_string_free(results);

    saikuro_client_close(client);
    saikuro_client_free(client);
    return 0;
}
```
