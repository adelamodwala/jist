use clap::Parser;
use jist::{buf_parser, schema_parser, schema_stream_parser, simd_parser, utils};
use log::debug;
use std::fs::File;
use std::{fs, io};
use std::io::{BufReader, Cursor, Read};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    data: Option<String>,

    #[arg(short, long)]
    file: Option<String>,

    #[arg(short, long)]
    path: Option<String>,

    #[arg(short, long)]
    streaming: bool,

    #[arg(short, long)]
    unionize: bool,
}

fn main() {
    let args = Args::parse();
    if args.file.is_some() {
        if args.path.is_none() {
            if args.streaming {
                match schema_stream_parser::parse(None, Some(args.file.unwrap().as_str())) {
                    Ok(result) => println!("{}", result),
                    Err(error) => panic!("{}", error),
                }
            } else {
                match schema_parser::summarize(fs::read_to_string(args.file.unwrap()).unwrap().as_str(), args.unionize) {
                    Ok(result) => println!("{}", result),
                    Err(error) => panic!("{}", error),
                }
            }

        } else {
            assert!(args.path.is_some());
            match search(
                None,
                Some(args.file.unwrap().as_str()),
                args.path.unwrap().as_str(),
                args.streaming,
            ) {
                Ok(result) => println!("{}", result),
                Err(error) => panic!("{}", error),
            }
        }
    } else {
        let haystack = if let Some(text) = args.data {
            text
        } else {
            let mut buffer = String::new();
            io::stdin()
                .read_to_string(&mut buffer)
                .expect("data not provided");
            buffer
        };
        if !haystack.is_empty() {
            if args.path.is_none() {
                if args.streaming {
                    match schema_stream_parser::parse(Some(&haystack), None) {
                        Ok(result) => println!("{}", result),
                        Err(error) => panic!("{}", error),
                    }
                } else {
                    match schema_parser::summarize(&haystack, args.unionize) {
                        Ok(result) => println!("{}", result),
                        Err(error) => panic!("{}", error),
                    }
                }
            } else {
                assert!(args.path.is_some());
                match search(
                    Some(haystack.as_str()),
                    None,
                    args.path.unwrap().as_str(),
                    args.streaming,
                ) {
                    Ok(result) => println!("{}", result),
                    Err(error) => panic!("{}", error),
                }
            }
        } else {
            panic!("No data provided");
        }
    }
}

pub fn search(
    haystack: Option<&str>,
    file: Option<&str>,
    search_key: &str,
    streaming: bool,
) -> Result<String, &'static str> {
    if (haystack.is_none() && file.is_none()) || search_key.is_empty() {
        return Err("Invalid input - no object found");
    }
    if file.is_none() && haystack.unwrap().is_empty() {
        return Err("Invalid input - empty data");
    }
    if haystack.is_none() && file.unwrap().is_empty() {
        return Err("Invalid input - empty file path");
    }

    // If input file size is greater than 4.2GB, fallback to buffered search
    let mut stream_only = streaming;
    if file.is_some() {
        let f = File::open(file.unwrap()).unwrap();
        if f.metadata().unwrap().len() >= u32::MAX as u64 {
            debug!("file too large - fallback to char lexer");
            stream_only = true;
        }
    }
    if stream_only {
        debug!("stream only");
        return buf_parser::search(haystack, file, search_key);
    }

    match simd_parser::search(haystack, file, search_key) {
        Ok(result) => Ok(result),
        Err(code) => {
            if code.eq("JIST_ERROR_FILE_TOO_LARGE") {
                debug!("fallback to char lexer");
                return buf_parser::search(haystack, file, search_key);
            }
            Err(code)
        }
    }
}
