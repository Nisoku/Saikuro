#include <cassert>
#include <cstring>
#include <cstdlib>
#include <string>
#include <vector>

extern "C" {
#include "saikuro.h"
}

struct saikuro_client {
    int id;
};

struct saikuro_provider {
    int id;
};

struct saikuro_stream {
    int index;
};

struct saikuro_channel {
    int index;
};

static std::string g_last_error;
static std::string g_last_connect_address;
static std::string g_last_call_target;
static std::string g_last_call_args;
static std::string g_last_batch_calls;
static std::string g_last_resource_target;
static std::string g_last_resource_args;
static saikuro_client g_client{1};
static saikuro_provider g_provider{1};
static saikuro_stream g_stream{0};
static saikuro_channel g_channel{0};
static std::vector<std::string> g_stream_items{"1", "2", "3"};
static std::vector<std::string> g_channel_items{"\"server-1\"", "\"server-2\""};
static std::vector<std::string> g_channel_sent;
static bool g_channel_closed = false;
static bool g_channel_aborted = false;
static bool g_announce_sent = false;
static saikuro_provider_handler_fn g_registered_callback = nullptr;
static void* g_registered_user_data = nullptr;
static std::string g_registered_name;

static char* dup_string(const std::string& value) {
    char* out = static_cast<char*>(std::malloc(value.size() + 1));
    if (out == nullptr) {
        throw std::bad_alloc();
    }
    std::memcpy(out, value.c_str(), value.size() + 1);
    return out;
}

extern "C" {

char* saikuro_string_dup(const char* input) {
    if (input == nullptr) {
        g_last_error = "input must not be null";
        return nullptr;
    }
    return dup_string(input);
}

void saikuro_string_free(char* ptr) {
    std::free(ptr);
}

char* saikuro_last_error_message(void) {
    return dup_string(g_last_error);
}

saikuro_client_t saikuro_client_connect(const char* address) {
    g_last_connect_address = address == nullptr ? "" : address;
    if (address == nullptr || std::strcmp(address, "tcp://bad") == 0) {
        g_last_error = "connect failed";
        return nullptr;
    }
    return &g_client;
}

int saikuro_client_close(saikuro_client_t) {
    return 0;
}

void saikuro_client_free(saikuro_client_t) {}

char* saikuro_client_call_json(saikuro_client_t, const char* target, const char* args_json) {
    const char* safe_target = target == nullptr ? "" : target;
    const char* safe_args = args_json == nullptr ? "" : args_json;
    g_last_call_target = safe_target;
    g_last_call_args = safe_args;
    if (std::strcmp(safe_target, "math.fail") == 0) {
        g_last_error = "call failed";
        return nullptr;
    }
    if (std::strcmp(safe_target, "echo.roundtrip") == 0) {
        return dup_string(g_last_call_args);
    }
    return dup_string("42");
}

char* saikuro_client_call_json_timeout(saikuro_client_t, const char* target, const char*, int timeout_ms) {
    if (timeout_ms < 50 || std::strcmp(target, "math.timeout") == 0) {
        g_last_error = "call timed out";
        return nullptr;
    }
    return dup_string("42");
}

int saikuro_client_cast_json(saikuro_client_t, const char* target, const char*) {
    if (std::strcmp(target, "math.fail") == 0) {
        g_last_error = "cast failed";
        return 1;
    }
    return 0;
}

char* saikuro_client_batch_json(saikuro_client_t, const char* calls_json) {
    g_last_batch_calls = calls_json == nullptr ? "" : calls_json;
    if (std::strstr(calls_json, "fail") != nullptr) {
        g_last_error = "batch failed";
        return nullptr;
    }
    return dup_string("[3,7]");
}

saikuro_stream_t saikuro_client_stream_json(saikuro_client_t, const char* target, const char*) {
    if (std::strcmp(target, "stream.fail") == 0) {
        g_last_error = "stream open failed";
        return nullptr;
    }
    g_stream.index = 0;
    return &g_stream;
}

int saikuro_stream_next_json(saikuro_stream_t stream, char** out_item_json, int* out_done) {
    if (stream == nullptr || out_item_json == nullptr || out_done == nullptr) {
        g_last_error = "stream next failed";
        return 1;
    }
    if (stream->index >= static_cast<int>(g_stream_items.size())) {
        *out_done = 1;
        *out_item_json = nullptr;
        return 0;
    }
    *out_done = 0;
    *out_item_json = dup_string(g_stream_items[stream->index]);
    stream->index += 1;
    return 0;
}

void saikuro_stream_free(saikuro_stream_t) {}

saikuro_channel_t saikuro_client_channel_json(saikuro_client_t, const char* target, const char*) {
    if (std::strcmp(target, "channel.fail") == 0) {
        g_last_error = "channel open failed";
        return nullptr;
    }
    g_channel.index = 0;
    g_channel_sent.clear();
    return &g_channel;
}

int saikuro_channel_send_json(saikuro_channel_t, const char* item_json) {
    g_channel_sent.emplace_back(item_json == nullptr ? "" : item_json);
    return 0;
}

int saikuro_channel_close(saikuro_channel_t) {
    g_channel_closed = true;
    return 0;
}

int saikuro_channel_abort(saikuro_channel_t) {
    g_channel_aborted = true;
    return 0;
}

int saikuro_channel_next_json(saikuro_channel_t channel, char** out_item_json, int* out_done) {
    if (channel == nullptr || out_item_json == nullptr || out_done == nullptr) {
        g_last_error = "channel next failed";
        return 1;
    }
    if (channel->index >= static_cast<int>(g_channel_items.size())) {
        *out_done = 1;
        *out_item_json = nullptr;
        return 0;
    }
    *out_done = 0;
    *out_item_json = dup_string(g_channel_items[channel->index]);
    channel->index += 1;
    return 0;
}

void saikuro_channel_free(saikuro_channel_t) {}

char* saikuro_client_resource_json(saikuro_client_t, const char* target, const char* args_json) {
    g_last_resource_target = target == nullptr ? "" : target;
    g_last_resource_args = args_json == nullptr ? "" : args_json;
    if (std::strcmp(target, "resource.fail") == 0) {
        g_last_error = "resource failed";
        return nullptr;
    }
    return dup_string("{\"ok\":true}");
}

int saikuro_client_log(saikuro_client_t, const char* level, const char*, const char*, const char*) {
    if (std::strcmp(level, "bad") == 0) {
        g_last_error = "log failed";
        return 1;
    }
    return 0;
}

saikuro_provider_t saikuro_provider_new(const char*) {
    return &g_provider;
}

int saikuro_provider_register(
    saikuro_provider_t,
    const char* name,
    saikuro_provider_handler_fn callback,
    void* user_data
) {
    if (callback == nullptr) {
        g_last_error = "callback must not be null";
        return 1;
    }
    g_registered_name = name == nullptr ? "" : name;
    g_registered_callback = callback;
    g_registered_user_data = user_data;
    return 0;
}

int saikuro_provider_serve(saikuro_provider_t, const char*) {
    if (g_registered_callback == nullptr) {
        g_last_error = "no callback";
        return 1;
    }
    g_announce_sent = true;
    char* result = g_registered_callback(g_registered_user_data, "[10,32]");
    if (result == nullptr) {
        g_last_error = "callback failed";
        return 1;
    }
    saikuro_string_free(result);
    return 0;
}

void saikuro_provider_free(saikuro_provider_t) {}
}

#include <saikuro/saikuro.hpp>

static char* provider_callback(void* user_data, const char* args_json) {
    auto* called = static_cast<bool*>(user_data);
    *called = true;
    if (std::strcmp(args_json, "[10,32]") != 0) {
        return saikuro_string_dup("0");
    }
    return saikuro_string_dup("42");
}

static void test_client_happy_path() {
    saikuro::Client client("tcp://ok");
    assert(g_last_connect_address == "tcp://ok");

    assert(client.call_json("math.add", "[1,2]") == "42");
    assert(client.call_json_timeout("math.add", "[1,2]", 100) == "42");
    client.cast_json("math.add", "[1,2]");
    assert(client.batch_json("[{\"target\":\"math.add\",\"args\":[1,2]}]") == "[3,7]");
    assert(client.resource_json("resource.open", "[]") == "{\"ok\":true}");
    client.log("info", "tests", "hello", "{}");

    auto stream = client.stream_json("numbers.stream", "[]");
    std::string item;
    assert(stream.next_json(item));
    assert(item == "1");
    assert(stream.next_json(item));
    assert(item == "2");
    assert(stream.next_json(item));
    assert(item == "3");
    assert(!stream.next_json(item));
    assert(item.empty());

    auto channel = client.channel_json("chat.open", "[]");
    channel.send_json("\"client-1\"");
    channel.send_json("\"client-2\"");
    assert(g_channel_sent.size() == 2);
    assert(g_channel_sent[0] == "\"client-1\"");

    assert(channel.next_json(item));
    assert(item == "\"server-1\"");
    assert(channel.next_json(item));
    assert(item == "\"server-2\"");
    assert(!channel.next_json(item));

    channel.close();
    channel.abort();
    assert(g_channel_closed);
    assert(g_channel_aborted);
}

static void test_envelope_roundtrip_fidelity() {
    saikuro::Client client("tcp://ok");

    const std::string args = R"([{"k":"v","n":42},[1,2,3],true,null])";
    const std::string call_out = client.call_json("echo.roundtrip", args);
    assert(call_out == args);
    assert(g_last_call_target == "echo.roundtrip");
    assert(g_last_call_args == args);

    const std::string batch = R"([{"target":"math.add","args":[1,2]}])";
    (void)client.batch_json(batch);
    assert(g_last_batch_calls == batch);

    const std::string resource_args = R"(["/tmp/file",{"mode":"r"}])";
    (void)client.resource_json("resource.open", resource_args);
    assert(g_last_resource_target == "resource.open");
    assert(g_last_resource_args == resource_args);

    auto channel = client.channel_json("chat.open", "[]");
    channel.send_json(R"({"msg":"hello","tags":["a","b"]})");
    assert(!g_channel_sent.empty());
    assert(g_channel_sent.back() == R"({"msg":"hello","tags":["a","b"]})");
}

static void test_client_errors_throw() {
    bool threw = false;
    try {
        saikuro::Client bad("tcp://bad");
    } catch (const saikuro::Error&) {
        threw = true;
    }
    assert(threw);

    saikuro::Client client("tcp://ok");

    threw = false;
    try {
        (void)client.call_json("math.fail", "[]");
    } catch (const saikuro::Error&) {
        threw = true;
    }
    assert(threw);

    threw = false;
    try {
        (void)client.call_json_timeout("math.timeout", "[]", 10);
    } catch (const saikuro::Error&) {
        threw = true;
    }
    assert(threw);

    threw = false;
    try {
        client.cast_json("math.fail", "[]");
    } catch (const saikuro::Error&) {
        threw = true;
    }
    assert(threw);

    threw = false;
    try {
        (void)client.batch_json("fail");
    } catch (const saikuro::Error&) {
        threw = true;
    }
    assert(threw);

    threw = false;
    try {
        (void)client.stream_json("stream.fail", "[]");
    } catch (const saikuro::Error&) {
        threw = true;
    }
    assert(threw);

    threw = false;
    try {
        (void)client.channel_json("channel.fail", "[]");
    } catch (const saikuro::Error&) {
        threw = true;
    }
    assert(threw);

    threw = false;
    try {
        (void)client.resource_json("resource.fail", "[]");
    } catch (const saikuro::Error&) {
        threw = true;
    }
    assert(threw);

    threw = false;
    try {
        client.log("bad", "tests", "hello", "{}");
    } catch (const saikuro::Error&) {
        threw = true;
    }
    assert(threw);
}

static void test_provider_wrapper() {
    bool callback_called = false;

    saikuro::Provider provider("math");
    provider.register_handler("add", provider_callback, &callback_called);
    provider.serve("tcp://unused");

    assert(g_registered_name == "add");
    assert(callback_called);
    assert(g_announce_sent);
}

int main() {
    test_client_happy_path();
    test_envelope_roundtrip_fidelity();
    test_client_errors_throw();
    test_provider_wrapper();
    return 0;
}
