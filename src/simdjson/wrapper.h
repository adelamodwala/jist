#pragma once
#include "simdjson/simdjson.h"
#include "cxx.h"

rust::String value_at_path(rust::Str input_str, rust::Str file_name, rust::Str json_path);