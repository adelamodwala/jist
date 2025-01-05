#include "wrapper.h"
#include <iostream>
#include <fstream>
#include <filesystem>

std::string get_result(simdjson::simdjson_result<simdjson::ondemand::document> &doc, std::string path)
{
    auto result = doc.at_path(path);
    std::ostringstream oss;
    oss << result;
    return std::string(oss.str());
}

rust::String value_at_path(rust::Str input_str, rust::Str file_name, rust::Str json_path)
{
    try
    {
        if (file_name.empty() && input_str.empty()) {
            return rust::String(std::string("Error: Input data not provided"));
        }

        simdjson::ondemand::parser parser;
        std::string json_path_s(json_path.data(), json_path.size());

        if (input_str.empty())
        {
            std::filesystem::path abs_path = std::filesystem::absolute(std::string_view(file_name.data(), file_name.size()));
            auto json = simdjson::padded_string::load(abs_path.string());
            auto doc = parser.iterate(json);
            return rust::String(get_result(doc, json_path_s));
        }
        else
        {
            auto json = simdjson::padded_string(std::string_view(input_str.data(), input_str.size()));
            auto doc = parser.iterate(json);
            return rust::String(get_result(doc, json_path_s));
        }
    }
    catch (const simdjson::simdjson_error &e)
    {
        if (e.error() == simdjson::error_code::MEMALLOC || e.error() == simdjson::error_code::CAPACITY) {
            return rust::String(std::string("JIST_ERROR_FILE_TOO_LARGE"));
        }
        return rust::String(std::string("JSON error: ") + e.what());
    }
    catch (const std::exception &e)
    {
        return rust::String(std::string("Error: ") + e.what());
    }
    catch (...)
    {
        return rust::String("Unknown error occurred");
    }
}