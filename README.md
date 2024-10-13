# Overview

`jist` attempts to find the complete JSON value (string, number, bool, or JSON object) for a given search key:

[![Watch the video](https://i.redd.it/l1luphdlsvl41.png)](https://github.com/user-attachments/assets/eb0c93e5-2380-434d-acc1-5ea6b470e50b)

Or

```
$ jist --data '{"a":"b", "c": {"d": ["e", "f", "g"]}}' --path "c.d"
["e", "f", "g"]
```

Or

```
$ jist -d '[{"a": "b"}, {"c": {"d": "e"}}]' -p "[1].c"
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

## Goals

- [x] It should find the full JSON value of a given search key. If the JSON data supplied provides an incomplete JSON value, the program should return an error.
- [ ] JSON object size should not impact number of operations to get result for a given search key, though it will affect speed and memory usage.
- [ ] As long as the search key is appropriate and a complete JSON value can be found, the input JSON object does not need to be complete or correctly formed.
- [ ] Parsing the entire input JSON object is not necessary, simply finding the search key path using JSON format is sufficient
- [ ] Streaming the JSON input should be possible, though will not be part of the starting design
