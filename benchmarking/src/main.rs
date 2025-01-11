use rand::Rng;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::sync::mpsc;
use std::thread::{self, available_parallelism};
use std::time::Instant;
use clap::Parser;
use humansize::{format_size, DECIMAL};

const JSON_TEMPLATE: &str = r#"    {
        "bar": {
            "baz": "{baz}",
            "bizbizbiz": "{bizbizbiz}",
            "bouou": [
                {bouou1},
                {bouou2}
            ],
            "poo": "{poo}"
        },
        "foo": {foo}
    }"#;

const BATCH_SIZE: usize = 1_000_000;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    n: Option<usize>,

    #[arg(short, long)]
    out: Option<String>,
}

fn main() {
    let args = Args::parse();
    let start_time = Instant::now();
    let num_threads = available_parallelism().unwrap().get();
    let num_objects = args.n.unwrap_or(num_threads);
    let objects_per_thread = num_objects / num_threads;

    println!("Generating {} objects using {} threads", num_objects, num_threads);
    println!("Each thread will generate {} objects in batches of {}", objects_per_thread, BATCH_SIZE);

    // Create a channel to receive completion signals
    let (tx, rx) = mpsc::channel();

    // Spawn threads
    for thread_id in 0..num_threads {
        let tx = tx.clone();
        thread::spawn(move || {
            let shard_filename = format!("shard_{}.json", thread_id);
            let file = File::create(&shard_filename).unwrap();
            let mut writer = BufWriter::new(file);
            let mut rng = rand::thread_rng();

            // Write opening bracket
            writer.write_all(b"[\n").unwrap();

            // Calculate number of full batches and remaining objects
            let num_batches = objects_per_thread / BATCH_SIZE;
            let remainder = objects_per_thread % BATCH_SIZE;

            // Pre-allocate string buffer for batch
            let mut batch = String::with_capacity(BATCH_SIZE * (JSON_TEMPLATE.len() + 100));

            // Process full batches
            for batch_num in 0..num_batches {
                batch.clear(); // Clear the string but keep capacity

                for i in 0..BATCH_SIZE {
                    let json = JSON_TEMPLATE
                        .replace("{baz}", &random_string(&mut rng, 50))
                        .replace("{bizbizbiz}", &random_string(&mut rng, 25))
                        .replace("{bouou1}", &rng.gen_range(1..100).to_string())
                        .replace("{bouou2}", &rng.gen_range(1..100).to_string())
                        .replace("{poo}", &rng.gen_bool(0.5).to_string())
                        .replace("{foo}", &rng.gen_range(1..100).to_string());

                    batch.push_str(&json);

                    // Add comma if not last object in thread
                    if !(batch_num == num_batches - 1 && i == BATCH_SIZE - 1 && remainder == 0) {
                        batch.push_str(",\n");
                    }
                }

                // Write entire batch at once
                writer.write_all(batch.as_bytes()).unwrap();
            }

            // Process remaining objects
            if remainder > 0 {
                batch.clear();
                for i in 0..remainder {
                    let json = JSON_TEMPLATE
                        .replace("{baz}", &random_string(&mut rng, 50))
                        .replace("{bizbizbiz}", &random_string(&mut rng, 25))
                        .replace("{bouou1}", &rng.gen_range(1..100).to_string())
                        .replace("{bouou2}", &rng.gen_range(1..100).to_string())
                        .replace("{poo}", &rng.gen_bool(0.5).to_string())
                        .replace("{foo}", &rng.gen_range(1..100).to_string());

                    batch.push_str(&json);
                    if i < remainder - 1 {
                        batch.push_str(",\n");
                    }
                }
                writer.write_all(batch.as_bytes()).unwrap();
            }

            // Write closing bracket
            writer.write_all(b"\n]").unwrap();
            writer.flush().unwrap();

            tx.send(shard_filename).unwrap();
            println!("Thread {} completed", thread_id);
        });
    }
    drop(tx);

    // Collect all shard filenames
    let shard_files: Vec<String> = rx.iter().collect();
    println!("All threads completed, combining shards...");

    // Combine shards into final output
    let output_file = File::create(args.out.unwrap_or("../output.json".to_string())).unwrap();
    let mut writer = BufWriter::new(&output_file);
    writer.write_all(b"[\n").unwrap();

    // Use larger buffer for combining files
    let mut buffer: Vec<String> = Vec::with_capacity(BATCH_SIZE * (JSON_TEMPLATE.len() + 100));

    for (i, shard_file) in shard_files.iter().enumerate() {
        // Read shard content (skipping first [ and last ])
        let content = fs::read_to_string(shard_file).unwrap();
        let content = &content[2..content.len()-2]; // Skip [ and ] from shard
        writer.write_all(content.as_bytes()).unwrap();

        // Add comma if not last shard
        if i < shard_files.len() - 1 {
            writer.write_all(b",\n").unwrap();
        }

        // Clean up shard file
        fs::remove_file(shard_file).unwrap();
    }

    writer.write_all(b"\n]").unwrap();
    writer.flush().unwrap();

    let duration = start_time.elapsed();
    let file_size = (&output_file).metadata().unwrap().len();
    println!("Completed in {:.2?}, file size: {}", duration, format_size(file_size, DECIMAL));
}

fn random_string(rng: &mut rand::rngs::ThreadRng, length: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut result = String::with_capacity(length);
    for _ in 0..length {
        result.push(CHARSET[rng.gen_range(0..CHARSET.len())] as char);
    }
    result
}
