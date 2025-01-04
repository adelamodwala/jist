fn main() -> miette::Result<()> {
    let path = std::path::PathBuf::from("src");
    let mut b = autocxx_build::Builder::new("src/simd_parser.rs", &[&path]).build().unwrap();
    b.flag_if_supported("-std=c++17")       // clang
        .flag_if_supported("/std:c++17")    // msvc
        .files(&["src/simdjson/wrapper.cpp", "src/simdjson/simdjson.cpp"])
        .include("src")
        .cpp(true)
        .compile("autocxx-simdjson-bridge");
    println!("cargo:rerun-if-changed=src/main.rs");
    println!("cargo:rerun-if-changed=src/simdjson/wrapper.cpp");
    println!("cargo:rerun-if-changed=src/simdjson/wrapper.h");
    Ok(())
}
