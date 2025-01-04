#include "wrapper.h"
#include <iostream>
#include <fstream>
#include <filesystem>

std::string get_result(simdjson::simdjson_result<simdjson::fallback::ondemand::document> &doc, std::string_view path)
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
        simdjson::ondemand::parser parser;
        std::string_view json_path_v(json_path.data(), json_path.size());

        if (input_str.empty())
        {
            std::string_view file_name_v(file_name.data(), file_name.size());
            std::filesystem::path abs_path = std::filesystem::absolute(file_name_v);
            auto json = simdjson::padded_string::load(abs_path.string());
            auto doc = parser.iterate(json);
            return rust::String(get_result(doc, json_path_v));
        }
        else
        {
            auto json = simdjson::padded_string(std::string_view(input_str.data(), input_str.size()));
            auto doc = parser.iterate(json);
            return rust::String(get_result(doc, json_path_v));
        }
    }
    catch (const simdjson::simdjson_error &e)
    {
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