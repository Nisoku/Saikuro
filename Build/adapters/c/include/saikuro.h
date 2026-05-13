#ifndef SAIKURO_H
#define SAIKURO_H

#include <stddef.h>

#if defined(_WIN32) || defined(__CYGWIN__)
#if defined(SAIKURO_BUILD_DLL)
#define SAIKURO_API __declspec(dllexport)
#elif defined(SAIKURO_USE_DLL)
#define SAIKURO_API __declspec(dllimport)
#else
#define SAIKURO_API
#endif
#define SAIKURO_CALL __cdecl
#else
#if defined(__GNUC__) || defined(__clang__)
#define SAIKURO_API __attribute__((visibility("default")))
#else
#define SAIKURO_API
#endif
#define SAIKURO_CALL
#endif

#ifdef __cplusplus
extern "C" {
#endif

/* Opaque handles */
struct saikuro_client;
struct saikuro_provider;
struct saikuro_stream;
struct saikuro_channel;
typedef struct saikuro_client* saikuro_client_t;
typedef struct saikuro_provider* saikuro_provider_t;
typedef struct saikuro_stream* saikuro_stream_t;
typedef struct saikuro_channel* saikuro_channel_t;

/* Provider callback: receives JSON args array and returns JSON result.
 * VERY important: The returned string must be allocated with saikuro_string_dup so the adapter
 * can reclaim it safely.
 */
typedef char* (SAIKURO_CALL *saikuro_provider_handler_fn)(void* user_data, const char* args_json);

/* Shared string helpers.
 * Ownership contract: functions returning char* transfer ownership to the caller.
 * Free returned pointers with saikuro_string_free (including saikuro_string_dup,
 * saikuro_last_error_message, and saikuro_client_call_json-style return values).
 */
SAIKURO_API char* SAIKURO_CALL saikuro_string_dup(const char* input);
SAIKURO_API void SAIKURO_CALL saikuro_string_free(char* ptr);
/* Returns a heap-allocated snapshot of the current thread's last error.
 * The last error state is thread-local (each thread has its own message).
 * The returned pointer must be freed with saikuro_string_free.
 */
SAIKURO_API char* SAIKURO_CALL saikuro_last_error_message(void);

/* Client API */
SAIKURO_API saikuro_client_t SAIKURO_CALL saikuro_client_connect(const char* address);
/** Close the given client connection.  Returns 0 on success, non-zero on error. */
SAIKURO_API int SAIKURO_CALL saikuro_client_close(saikuro_client_t handle);
SAIKURO_API void SAIKURO_CALL saikuro_client_free(saikuro_client_t handle);

SAIKURO_API char* SAIKURO_CALL saikuro_client_call_json(
    saikuro_client_t handle,
    const char* target,
    const char* args_json
);

SAIKURO_API char* SAIKURO_CALL saikuro_client_call_json_timeout(
    saikuro_client_t handle,
    const char* target,
    const char* args_json,
    int timeout_ms
);

SAIKURO_API int SAIKURO_CALL saikuro_client_cast_json(
    saikuro_client_t handle,
    const char* target,
    const char* args_json
);

SAIKURO_API char* SAIKURO_CALL saikuro_client_batch_json(
    saikuro_client_t handle,
    const char* calls_json
);

SAIKURO_API saikuro_stream_t SAIKURO_CALL saikuro_client_stream_json(
    saikuro_client_t handle,
    const char* target,
    const char* args_json
);

SAIKURO_API saikuro_channel_t SAIKURO_CALL saikuro_client_channel_json(
    saikuro_client_t handle,
    const char* target,
    const char* args_json
);

SAIKURO_API int SAIKURO_CALL saikuro_channel_send_json(
    saikuro_channel_t channel,
    const char* item_json
);

SAIKURO_API int SAIKURO_CALL saikuro_channel_close(saikuro_channel_t channel);
SAIKURO_API int SAIKURO_CALL saikuro_channel_abort(saikuro_channel_t channel);

/* Same contract as saikuro_channel_next_json: see above. */
SAIKURO_API int SAIKURO_CALL saikuro_channel_next_json(
    saikuro_channel_t channel,
    char** out_item_json,
    int* out_done
);

SAIKURO_API void SAIKURO_CALL saikuro_channel_free(saikuro_channel_t channel);

/* Same contract as saikuro_channel_next_json: see above. */
SAIKURO_API int SAIKURO_CALL saikuro_stream_next_json(
    saikuro_stream_t stream,
    char** out_item_json,
    int* out_done
);

SAIKURO_API void SAIKURO_CALL saikuro_stream_free(saikuro_stream_t stream);

SAIKURO_API char* SAIKURO_CALL saikuro_client_resource_json(
    saikuro_client_t handle,
    const char* target,
    const char* args_json
);

SAIKURO_API int SAIKURO_CALL saikuro_client_log(
    saikuro_client_t handle,
    const char* level,
    const char* name,
    const char* msg,
    const char* fields_json
);

/* Provider API */
SAIKURO_API saikuro_provider_t SAIKURO_CALL saikuro_provider_new(const char* namespace_name);

SAIKURO_API int SAIKURO_CALL saikuro_provider_register(
    saikuro_provider_t handle,
    const char* name,
    saikuro_provider_handler_fn callback,
    void* user_data
);

SAIKURO_API int SAIKURO_CALL saikuro_provider_serve(saikuro_provider_t handle, const char* address);
SAIKURO_API void SAIKURO_CALL saikuro_provider_free(saikuro_provider_t handle);

#ifdef __cplusplus
}
#endif

#endif
