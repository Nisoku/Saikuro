#include <type_traits>

#include <saikuro/saikuro.hpp>

static_assert(!std::is_copy_constructible_v<saikuro::Client>);
static_assert(std::is_move_constructible_v<saikuro::Client>);
static_assert(!std::is_copy_constructible_v<saikuro::Client::Stream>);
static_assert(std::is_move_constructible_v<saikuro::Client::Stream>);
static_assert(!std::is_copy_constructible_v<saikuro::Client::Channel>);
static_assert(std::is_move_constructible_v<saikuro::Client::Channel>);
static_assert(!std::is_copy_constructible_v<saikuro::Provider>);
static_assert(std::is_move_constructible_v<saikuro::Provider>);

static_assert(!std::is_copy_assignable_v<saikuro::Client>);
static_assert(std::is_move_assignable_v<saikuro::Client>);
static_assert(!std::is_copy_assignable_v<saikuro::Client::Stream>);
static_assert(std::is_move_assignable_v<saikuro::Client::Stream>);
static_assert(!std::is_copy_assignable_v<saikuro::Client::Channel>);
static_assert(std::is_move_assignable_v<saikuro::Client::Channel>);
static_assert(!std::is_copy_assignable_v<saikuro::Provider>);
static_assert(std::is_move_assignable_v<saikuro::Provider>);

int main() {
    return 0;
}
