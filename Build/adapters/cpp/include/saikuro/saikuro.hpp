#ifndef SAIKURO_CPP_SAIKURO_HPP
#define SAIKURO_CPP_SAIKURO_HPP

#include <stdexcept>
#include <string>
#include <memory>
#include <utility>

extern "C" {
#include "saikuro.h"
}

namespace saikuro {

class Error : public std::runtime_error {
public:
    explicit Error(const std::string& message) : std::runtime_error(message) {}
};

inline std::string take_owned_c_string(char* ptr) {
    if (ptr == nullptr) {
        return {};
    }
    std::unique_ptr<char, decltype(&saikuro_string_free)> guard(ptr, &saikuro_string_free);
    std::string out(guard.get());
    return out;
}

inline std::string last_error() {
    return take_owned_c_string(saikuro_last_error_message());
}

// CRTP base: move-only RAII handle with custom destroy.
template<typename Derived, typename Handle>
class MoveOnlyHandle {
public:
    MoveOnlyHandle(const MoveOnlyHandle&) = delete;
    MoveOnlyHandle& operator=(const MoveOnlyHandle&) = delete;

    MoveOnlyHandle(MoveOnlyHandle&& other) noexcept
        : handle_(std::exchange(other.handle_, nullptr)) {}

    MoveOnlyHandle& operator=(MoveOnlyHandle&& other) noexcept {
        if (this != &other) {
            destroy();
            handle_ = std::exchange(other.handle_, nullptr);
        }
        return *this;
    }

    ~MoveOnlyHandle() { destroy(); }

protected:
    MoveOnlyHandle() = default;
    explicit MoveOnlyHandle(Handle handle) : handle_(handle) {}

    Handle handle_ = nullptr;

    void destroy() {
        if (handle_ != nullptr) {
            static_cast<Derived*>(this)->destroy_impl();
            handle_ = nullptr;
        }
    }

    template<typename Fn>
    bool next_json_from(std::string& out_item_json, Fn c_next_fn) {
        char* raw = nullptr;
        int done = 0;
        if (c_next_fn(handle_, &raw, &done) != 0) {
            throw Error(last_error());
        }
        if (done != 0) {
            out_item_json.clear();
            return false;
        }
        out_item_json = take_owned_c_string(raw);
        return true;
    }
};

class Client : public MoveOnlyHandle<Client, saikuro_client_t> {
public:
    class Stream : public MoveOnlyHandle<Stream, saikuro_stream_t> {
    public:
        explicit Stream(saikuro_stream_t handle) : MoveOnlyHandle(handle) {
            if (handle_ == nullptr) {
                throw Error(last_error());
            }
        }

        bool next_json(std::string& out_item_json) {
            return next_json_from(out_item_json, saikuro_stream_next_json);
        }

    private:
        friend class MoveOnlyHandle<Stream, saikuro_stream_t>;
        void destroy_impl() { saikuro_stream_free(handle_); }
    };

    class Channel : public MoveOnlyHandle<Channel, saikuro_channel_t> {
    public:
        explicit Channel(saikuro_channel_t handle) : MoveOnlyHandle(handle) {
            if (handle_ == nullptr) {
                throw Error(last_error());
            }
        }

        void send_json(const std::string& item_json) {
            if (!open_) {
                throw Error("channel is closed");
            }
            if (saikuro_channel_send_json(handle_, item_json.c_str()) != 0) {
                throw Error(last_error());
            }
        }

        void close() {
            if (saikuro_channel_close(handle_) != 0) {
                throw Error(last_error());
            }
            open_ = false;
        }

        void abort() {
            if (saikuro_channel_abort(handle_) != 0) {
                throw Error(last_error());
            }
            open_ = false;
        }

        bool next_json(std::string& out_item_json) {
            if (!next_json_from(out_item_json, saikuro_channel_next_json)) {
                open_ = false;
                return false;
            }
            return true;
        }

    public:
        Channel(Channel&&) = default;
        Channel& operator=(Channel&& other) noexcept {
            if (this != &other) {
                MoveOnlyHandle::operator=(std::move(other));
                open_ = std::exchange(other.open_, false);
            }
            return *this;
        }

    private:
        friend class MoveOnlyHandle<Channel, saikuro_channel_t>;
        void destroy_impl() {
            if (open_) { std::ignore = saikuro_channel_close(handle_); }
            saikuro_channel_free(handle_);
        }

        bool open_ = true;
    };

    explicit Client(const std::string& address) {
        handle_ = saikuro_client_connect(address.c_str());
        if (handle_ == nullptr) {
            throw Error(last_error());
        }
    }

    std::string call_json(const std::string& target, const std::string& args_json) const {
        char* result = saikuro_client_call_json(handle_, target.c_str(), args_json.c_str());
        if (result == nullptr) {
            throw Error(last_error());
        }
        return take_owned_c_string(result);
    }

    std::string call_json_timeout(
        const std::string& target,
        const std::string& args_json,
        int timeout_ms
    ) const {
        char* result = saikuro_client_call_json_timeout(
            handle_,
            target.c_str(),
            args_json.c_str(),
            timeout_ms
        );
        if (result == nullptr) {
            throw Error(last_error());
        }
        return take_owned_c_string(result);
    }

    void cast_json(const std::string& target, const std::string& args_json) const {
        if (saikuro_client_cast_json(handle_, target.c_str(), args_json.c_str()) != 0) {
            throw Error(last_error());
        }
    }

    std::string batch_json(const std::string& calls_json) const {
        char* result = saikuro_client_batch_json(handle_, calls_json.c_str());
        if (result == nullptr) {
            throw Error(last_error());
        }
        return take_owned_c_string(result);
    }

    Stream stream_json(const std::string& target, const std::string& args_json) const {
        saikuro_stream_t stream = saikuro_client_stream_json(handle_, target.c_str(), args_json.c_str());
        if (stream == nullptr) {
            throw Error(last_error());
        }
        return Stream(stream);
    }

    Channel channel_json(const std::string& target, const std::string& args_json) const {
        saikuro_channel_t channel = saikuro_client_channel_json(handle_, target.c_str(), args_json.c_str());
        if (channel == nullptr) {
            throw Error(last_error());
        }
        return Channel(channel);
    }

    std::string resource_json(const std::string& target, const std::string& args_json) const {
        char* result = saikuro_client_resource_json(handle_, target.c_str(), args_json.c_str());
        if (result == nullptr) {
            throw Error(last_error());
        }
        return take_owned_c_string(result);
    }

    void log(
        const std::string& level,
        const std::string& name,
        const std::string& msg,
        const std::string& fields_json = "{}"
    ) const {
        if (saikuro_client_log(handle_, level.c_str(), name.c_str(), msg.c_str(), fields_json.c_str()) != 0) {
            throw Error(last_error());
        }
    }

private:
    friend class MoveOnlyHandle<Client, saikuro_client_t>;
    void destroy_impl() {
        std::ignore = saikuro_client_close(handle_);
        saikuro_client_free(handle_);
    }
};

class Provider : public MoveOnlyHandle<Provider, saikuro_provider_t> {
public:
    using RawHandler = saikuro_provider_handler_fn;

    explicit Provider(const std::string& namespace_name) {
        handle_ = saikuro_provider_new(namespace_name.c_str());
        if (handle_ == nullptr) {
            throw Error(last_error());
        }
    }

    void register_handler(const std::string& name, RawHandler callback, void* user_data) {
        if (saikuro_provider_register(handle_, name.c_str(), callback, user_data) != 0) {
            throw Error(last_error());
        }
    }

    void serve(const std::string& address) {
        if (saikuro_provider_serve(handle_, address.c_str()) != 0) {
            throw Error(last_error());
        }
    }

private:
    friend class MoveOnlyHandle<Provider, saikuro_provider_t>;
    void destroy_impl() { saikuro_provider_free(handle_); }
};

}  // namespace saikuro

#endif
