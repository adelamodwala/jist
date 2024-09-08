use clap::Parser;
use jist::search;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    data: String,

    #[arg(short, long)]
    search_key: String,
}

fn main() {
    let args = Args::parse();

    match search(args.data, args.search_key) {
        Ok(result) => println!("{}", result),
        Err(error) => panic!("{}", error),
    }
}
