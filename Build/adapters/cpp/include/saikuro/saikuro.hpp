#ifndef SAIKURO_CPP_SAIKURO_HPP
#define SAIKURO_CPP_SAIKURO_HPP

#include <stdexcept>
#include <string>
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
    std::string out(ptr);
    saikuro_string_free(ptr);
    return out;
}

inline std::string last_error() {
    return take_owned_c_string(saikuro_last_error_message());
}

class Client {
public:
    class Stream {
    public:
        explicit Stream(saikuro_stream_t handle) : handle_(handle) {
            if (handle_ == nullptr) {
                throw Error(last_error());
            }
        }

        Stream(const Stream&) = delete;
        Stream& operator=(const Stream&) = delete;

        Stream(Stream&& other) noexcept : handle_(other.handle_) {
            other.handle_ = nullptr;
        }

        Stream& operator=(Stream&& other) noexcept {
            if (this != &other) {
                destroy();
                handle_ = other.handle_;
                other.handle_ = nullptr;
            }
            return *this;
        }

        ~Stream() {
            destroy();
        }

        bool next_json(std::string& out_item_json) {
            char* raw = nullptr;
            int done = 0;
            if (saikuro_stream_next_json(handle_, &raw, &done) != 0) {
                throw Error(last_error());
            }
            if (done != 0) {
                out_item_json.clear();
                return false;
            }
            out_item_json = take_owned_c_string(raw);
            return true;
        }

    private:
        void destroy() {
            if (handle_ != nullptr) {
                saikuro_stream_free(handle_);
                handle_ = nullptr;
            }
        }

        saikuro_stream_t handle_ = nullptr;
    };

    class Channel {
    public:
        explicit Channel(saikuro_channel_t handle) : handle_(handle) {
            if (handle_ == nullptr) {
                throw Error(last_error());
            }
        }

        Channel(const Channel&) = delete;
        Channel& operator=(const Channel&) = delete;

        Channel(Channel&& other) noexcept : handle_(other.handle_) {
            other.handle_ = nullptr;
        }

        Channel& operator=(Channel&& other) noexcept {
            if (this != &other) {
                destroy();
                handle_ = other.handle_;
                other.handle_ = nullptr;
            }
            return *this;
        }

        ~Channel() {
            destroy();
        }

        void send_json(const std::string& item_json) {
            if (saikuro_channel_send_json(handle_, item_json.c_str()) != 0) {
                throw Error(last_error());
            }
        }

        void close() {
            if (saikuro_channel_close(handle_) != 0) {
                throw Error(last_error());
            }
        }

        void abort() {
            if (saikuro_channel_abort(handle_) != 0) {
                throw Error(last_error());
            }
        }

        bool next_json(std::string& out_item_json) {
            char* raw = nullptr;
            int done = 0;
            if (saikuro_channel_next_json(handle_, &raw, &done) != 0) {
                throw Error(last_error());
            }
            if (done != 0) {
                out_item_json.clear();
                return false;
            }
            out_item_json = take_owned_c_string(raw);
            return true;
        }

    private:
        void destroy() {
            if (handle_ != nullptr) {
                saikuro_channel_free(handle_);
                handle_ = nullptr;
            }
        }

        saikuro_channel_t handle_ = nullptr;
    };

    explicit Client(const std::string& address) {
        handle_ = saikuro_client_connect(address.c_str());
        if (handle_ == nullptr) {
            throw Error(last_error());
        }
    }

    Client(const Client&) = delete;
    Client& operator=(const Client&) = delete;

    Client(Client&& other) noexcept : handle_(other.handle_) {
        other.handle_ = nullptr;
    }

    Client& operator=(Client&& other) noexcept {
        if (this != &other) {
            destroy();
            handle_ = other.handle_;
            other.handle_ = nullptr;
        }
        return *this;
    }

    ~Client() {
        destroy();
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
    void destroy() {
        if (handle_ != nullptr) {
            saikuro_client_close(handle_);
            saikuro_client_free(handle_);
            handle_ = nullptr;
        }
    }

    saikuro_client_t handle_ = nullptr;
};

class Provider {
public:
    using RawHandler = saikuro_provider_handler_fn;

    explicit Provider(const std::string& namespace_name) {
        handle_ = saikuro_provider_new(namespace_name.c_str());
        if (handle_ == nullptr) {
            throw Error(last_error());
        }
    }

    Provider(const Provider&) = delete;
    Provider& operator=(const Provider&) = delete;

    Provider(Provider&& other) noexcept : handle_(other.handle_) {
        other.handle_ = nullptr;
    }

    Provider& operator=(Provider&& other) noexcept {
        if (this != &other) {
            destroy();
            handle_ = other.handle_;
            other.handle_ = nullptr;
        }
        return *this;
    }

    ~Provider() {
        destroy();
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
    void destroy() {
        if (handle_ != nullptr) {
            saikuro_provider_free(handle_);
            handle_ = nullptr;
        }
    }

    saikuro_provider_t handle_ = nullptr;
};

}  // namespace saikuro

#endif
