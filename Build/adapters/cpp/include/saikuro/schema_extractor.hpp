#ifndef SAIKURO_CPP_SCHEMA_EXTRACTOR_HPP
#define SAIKURO_CPP_SCHEMA_EXTRACTOR_HPP

#include <string>

namespace saikuro {

std::string extract_schema_from_header(
    const std::string& source,
    const std::string& namespace_name,
    bool pretty
);

std::string extract_schema_from_file(
    const std::string& path,
    const std::string& namespace_name,
    bool pretty
);

}  // namespace saikuro

#endif
