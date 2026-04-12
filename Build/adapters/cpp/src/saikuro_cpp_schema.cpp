#include <saikuro/schema_extractor.hpp>

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

    std::vector<std::string> args;
    for (int i = 1; i < argc; ++i) {
        args.push_back(argv[i]);
    }

    for (size_t i = 0; i < args.size(); ++i) {
        if (args[i] == "--pretty") {
            pretty = true;
            continue;
        }
        if (args[i] == "--namespace") {
            if (i + 1 >= args.size()) {
                std::cerr << "--namespace requires a value\n";
                print_usage();
                return 2;
            }
            namespace_name = args[i + 1];
            i += 1;
            continue;
        }
        if (!args[i].empty() && args[i][0] == '-') {
            std::cerr << "unknown argument: " << args[i] << "\n";
            print_usage();
            return 2;
        }
        if (!header_path.empty()) {
            std::cerr << "unexpected extra positional argument: " << args[i] << "\n";
            print_usage();
            return 2;
        }
        header_path = args[i];
    }

    if (header_path.empty()) {
        print_usage();
        return 2;
    }

    try {
        const std::string schema = saikuro::extract_schema_from_file(header_path, namespace_name, pretty);
        std::cout << schema << std::endl;
    } catch (const std::exception& ex) {
        std::cerr << ex.what() << std::endl;
        return 1;
    }

    return 0;
}
