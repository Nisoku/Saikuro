#include <type_traits>

#include <saikuro/saikuro.hpp>

static_assert(!std::is_copy_constructible<saikuro::Client>::value,
              "Client should not be copy constructible");
static_assert(std::is_move_constructible<saikuro::Client>::value,
              "Client should be move constructible");
static_assert(!std::is_copy_constructible<saikuro::Client::Stream>::value,
              "Client::Stream should not be copy constructible");
static_assert(std::is_move_constructible<saikuro::Client::Stream>::value,
              "Client::Stream should be move constructible");
static_assert(!std::is_copy_constructible<saikuro::Client::Channel>::value,
              "Client::Channel should not be copy constructible");
static_assert(std::is_move_constructible<saikuro::Client::Channel>::value,
              "Client::Channel should be move constructible");
static_assert(!std::is_copy_constructible<saikuro::Provider>::value,
              "Provider should not be copy constructible");
static_assert(std::is_move_constructible<saikuro::Provider>::value,
              "Provider should be move constructible");

static_assert(!std::is_copy_assignable<saikuro::Client>::value,
              "Client should not be copy assignable");
static_assert(std::is_move_assignable<saikuro::Client>::value,
              "Client should be move assignable");
static_assert(!std::is_copy_assignable<saikuro::Client::Stream>::value,
              "Client::Stream should not be copy assignable");
static_assert(std::is_move_assignable<saikuro::Client::Stream>::value,
              "Client::Stream should be move assignable");
static_assert(!std::is_copy_assignable<saikuro::Client::Channel>::value,
              "Client::Channel should not be copy assignable");
static_assert(std::is_move_assignable<saikuro::Client::Channel>::value,
              "Client::Channel should be move assignable");
static_assert(!std::is_copy_assignable<saikuro::Provider>::value,
              "Provider should not be copy assignable");
static_assert(std::is_move_assignable<saikuro::Provider>::value,
              "Provider should be move assignable");

int main() { return 0; }
