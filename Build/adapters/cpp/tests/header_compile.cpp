#include <type_traits>

#include <saikuro/saikuro.hpp>

#define CHECK_TYPE_TRAITS(Type)                                                 \
  static_assert(!std::is_copy_constructible_v<Type>,                            \
                #Type " should not be copy constructible");                     \
  static_assert(std::is_move_constructible_v<Type>,                             \
                #Type " should be move constructible");                         \
  static_assert(!std::is_copy_assignable_v<Type>,                               \
                #Type " should not be copy assignable");                        \
  static_assert(std::is_move_assignable_v<Type>,                                \
                #Type " should be move assignable")

CHECK_TYPE_TRAITS(saikuro::Client);
CHECK_TYPE_TRAITS(saikuro::Client::Stream);
CHECK_TYPE_TRAITS(saikuro::Client::Channel);
CHECK_TYPE_TRAITS(saikuro::Provider);

int main() { return 0; }
