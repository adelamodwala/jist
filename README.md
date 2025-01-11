# Overview

`jist` attempts to find the complete JSON value (string, number, bool, or JSON object) for a given search key as fast as possible.

## Benchmarks
`jist` uses the state-of-the art [simdjson](https://github.com/simdjson/simdjson) library for parsing JSON input / files smaller than 4.2GB in size.

| 3.3GB input (get last element) |    jist    |   jq    |
|:-------------------------------|:----------:|:-------:|
| Time                           |   2.05s    | 34.17s  |
| Memory                         |   4.5GB    | 18GB 😱 |
| Throughput                     | 1600MB/s ✅ | 96MB/s  |

For files larger than 4.2GB or that don't fit into memory or forced streaming mode (option `-s` or `--streaming`), `jist` falls back to an earlier implementation using a simple character based lexer [json-tools](https://github.com/Byron/json-tools/). While the fallback implementation is relatively slower than simdjson, it's still really fast at a throughput of ~300MB/s and uses almost no memory (around 10MB generally) for virtually any size of file (B / KB / MB / GB / TB / PB / etc).

| 28.9GB input (get last element) |   jist    | jq (not enough ram) |
|:--------------------------------|:---------:|:-------------------:|
| Time                            |  1:30s ✅  |          ❌          |
| Memory                          |  9.7MB ✅  |          ❌          |
| Throughput                      | 321MB/s ✅ |          ❌          |

_(Test machine: Intel i7-12700H, 64GB DDR5@4800MT RAM)_

### Method
The benchmark file has the following shape:
```json
[
   {
      "bar": {
         "baz": "65gBJtrk7B1YrQVqgo9jxw4TXvS2UQ5upIiXPwI6Vtx36eQvHS",
         "bizbizbiz": "SCGgrAumMpZkfD7BWgryfka5Q",
         "bouou": [
            91,
            55
         ],
         "poo": "true"
      },
      "foo": 45
   },
   ...
]
```
To create your own file of similar structure:
```shell
# cd to the project directory
cd benchmarking
cargo build --release # things will go way faster with release especially for really large files
./target/release/genearator -n 100000000 -o ../output.json 
# n = number of records with the shape above - e.g. 100M records will result in a 28.9GB file
cd ../ # go back to project root directory
cargo build --release # (optional - build from source or use binary) again things will be much faster with release
./target/release/jist -f output.json -p "[9999999].bar.baz"
```
You can of course modify the shape of the data as well by updating `jist/benchmarking/src/main.rs`.

## Examples
```
$ jist --data '{"a":"b", "c": {"d": ["e", "f", "g"]}}' --path "c.d"
["e", "f", "g"]
```

Or

```
$ jist -d '[{"a": "b"}, {"c": {"d": "e"}}]' -p "[1].c"
{"d": "e"}
```

Or
```
$ jist -f my.json -p "[1054041].c"
{"d": "e"}
```

One of the use cases I had in mind was being able to extract values from JSON objects like access tokens programmatically for setting up config files easily without having to perform `jq` gymnastics. You know the JSON data shape and key you're looking for, just declare what you want.

## Interface:

1. Find the value of an exact match

   `jist` can take any valid JSON as input including an array root type. It expects the search key to be valid given the requested key.

```
$ curl https://api.github.com/repos/adelamodwala/rustbook/commits?per_page=1 | jist -p "[0].commit.author"
{
    "name": "adelamodwala",
    "email": "adel.amodwala@gmail.com",
    "date": "2023-11-06T20:36:53Z"
}
```

2. You can find values for keys that are deeply nested

```
$ wget https://api.github.com/repos/adelamodwala/rustbook/commits?per_page=1 | jist -p "[0].commit.author.name"
adelamodwala
```

3. If the root object is an array, then it's named `root` by default. All arrays are used like Javascript arrays syntactically.

```
$ wget https://api.github.com/repos/adelamodwala/rustbook/commits?per_page=1 | jist -p "[0].parents"
[]
```

# Algorithm
`jist` uses [simdjson](https://github.com/simdjson/simdjson), a C++ library, over a rust-C++ bridge. While a pure rust implementation of simdjson exists, it performed twice as slow as the native C++ version in my testing.

When the JSON input file is too large or in streaming mode, `jist` falls back to a streaming approach that keeps memory usage low, and uses `json-tools` crate to get a lexer iterator. Put together, we can scan through a JSON string/file from the top and keep track of depths compared to our target depth without ever unmarshalling JSON into memory. Once we reach our target depth and match all the expected indices/keys, `jist` returns the result.  

## Goals

- [x] It should find the full JSON value of a given search key (streaming mode or when file is too large only). If the JSON data supplied provides an incomplete JSON value, the program should return an error.
- [x] JSON object size should not impact memory usage while fully utilizing a single CPU core (streaming mode or when file is too large only)
- [x] As long as the search key is appropriate and a complete JSON value can be found, the input JSON object does not need to be complete or correctly formed (streaming mode or when file is too large only)
- [x] Parsing the entire input JSON object is not necessary, simply finding the search key path using JSON format is sufficient (streaming mode or when file is too large only)
- [x] Streaming the JSON input should be possible, though will not be part of the starting design
- [x] SIMD: the final frontier
- [ ] Feature: generate JSON schema, like super fast
- [ ] Search over compressed files like `gzip` and `bgzip`

