#include <saikuro/schema_extractor.hpp>

#include <cstdlib>
#include <iostream>
#include <string>
#include <vector>

namespace {

void print_usage() {
    std::cerr << "Usage: saikuro-cpp-schema [--namespace <name>] [--pretty] <header>\n";
}

}  // namespace

int main(int argc, char** argv) {
    std::string namespace_name = "default";
    bool pretty = false;
    std::string header_path;

    for (int i = 1; i < argc; ++i) {
        const std::string arg(argv[i]);
        if (arg == "--pretty") {
            pretty = true;
            continue;
        }
        if (arg == "--namespace") {
            if (i + 1 >= argc) {
                std::cerr << "--namespace requires a value\n";
                print_usage();
                return EXIT_FAILURE;
            }
            namespace_name = argv[i + 1];
            i += 1;
            continue;
        }
        if (!arg.empty() && arg[0] == '-') {
            std::cerr << "unknown argument: " << arg << "\n";
            print_usage();
            return EXIT_FAILURE;
        }
        if (!header_path.empty()) {
            std::cerr << "unexpected extra positional argument: " << arg << "\n";
            print_usage();
            return EXIT_FAILURE;
        }
        header_path = arg;
    }

    if (header_path.empty()) {
        print_usage();
        return EXIT_FAILURE;
    }

    try {
        const std::string schema = saikuro::extract_schema_from_file(header_path, namespace_name, pretty);
        std::cout << schema << std::endl;
    } catch (const std::exception& ex) {
        std::cerr << ex.what() << std::endl;
        return EXIT_FAILURE;
    }

    return EXIT_SUCCESS;
}
