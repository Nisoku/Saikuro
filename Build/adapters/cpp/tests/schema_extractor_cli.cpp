#include <saikuro/schema_extractor.hpp>

#include <cassert>
#include <iostream>
#include <string>
#include <tuple>

#ifndef SAIKURO_SCHEMA_FIXTURE
#define SAIKURO_SCHEMA_FIXTURE ""
#endif

static void contains_or_fail(const std::string &text,
                             const std::string &needle) {
  if (text.find(needle) == std::string::npos) {
    std::cerr << "Expected to find substring: '" << needle << "'\n";
    std::cerr << "Actual text:\n" << text << "\n";
    assert(!"Expected to find substring");
  }
}

int main() {
  const std::string fixture = SAIKURO_SCHEMA_FIXTURE;
  assert(!fixture.empty());

  const std::string compact =
      saikuro::extract_schema_from_file(fixture, "parityns", false);
  contains_or_fail(compact, "\"version\":1");
  contains_or_fail(compact, "\"parityns\"");
  contains_or_fail(compact, "\"add\"");
  contains_or_fail(compact, "\"maybe\"");
  contains_or_fail(compact, "\"avg\"");
  contains_or_fail(compact, "\"fire_and_forget\"");

  const std::string pretty =
      saikuro::extract_schema_from_file(fixture, "parityns", true);
  contains_or_fail(pretty, "\n");
  contains_or_fail(pretty, "  \"namespaces\"");

  bool threw = false;
  try {
    std::ignore = saikuro::extract_schema_from_file(
        "/definitely/missing/header.h", "parityns", false);
  } catch (const std::exception &) {
    threw = true;
  }
  assert(threw);

  return 0;
}
