use std::io;
use std::io::Read;
use clap::Parser;
use jist::search;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    data: Option<String>,

    #[arg(short, long)]
    path: String,
}

fn main() {
    let args = Args::parse();
    let haystack = if let Some(text) = args.data {
        text
    } else {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer).expect("data not provided");
        buffer
    };


    match search(haystack.as_str(), args.path.as_str()) {
        Ok(result) => println!("{}", result),
        Err(error) => panic!("{}", error),
    }
}
