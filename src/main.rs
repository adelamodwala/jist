use clap::Parser;
use jist::search;
use std::io;
use std::io::Read;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    data: Option<String>,

    #[arg(short, long)]
    file: Option<String>,

    #[arg(short, long)]
    buffsize: Option<usize>,

    #[arg(short, long)]
    path: String,
}

fn main() {
    let args = Args::parse();
    if args.file.is_some() {
        match search(None, Some(args.file.unwrap().as_str()), args.path.as_str(), args.buffsize) {
            Ok(result) => println!("{}", result),
            Err(error) => panic!("{}", error),
        }
    } else {
        let haystack = if let Some(text) = args.data {
            text
        } else {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer).expect("data not provided");
            buffer
        };
        if !haystack.is_empty() {
            match search(Some(haystack.as_str()), None, args.path.as_str(), None) {
                Ok(result) => println!("{}", result),
                Err(error) => panic!("{}", error),
            }
        } else {
            panic!("No data provided");
        }
    }
}
