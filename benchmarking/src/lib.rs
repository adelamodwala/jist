use rand::Rng;
use rand::rngs::ThreadRng;
use serde_json::{json, Value};

fn random_ip(rng: &mut ThreadRng) -> String {
    format!("{}.{}.{}.{}",
            rng.gen_range(1..255),
            rng.gen_range(1..255),
            rng.gen_range(1..255),
            rng.gen_range(1..255)
    )
}

fn random_hostname(rng: &mut ThreadRng) -> String {
    let domains = ["example", "test", "sample", "demo"];
    let tlds = ["com", "net", "org", "io"];
    format!("{}.{}.{}",
            random_string(rng, 8),
            domains[rng.gen_range(0..domains.len())],
            tlds[rng.gen_range(0..tlds.len())]
    )
}

fn random_string(rng: &mut ThreadRng, len: usize) -> String {
    (0..len)
        .map(|_| rng.gen_range(b'a'..=b'z') as char)
        .collect()
}

fn generate_location_data(rng: &mut ThreadRng) -> Value {
    json!({
        "autonomous_system_number": rng.gen_range(1000..50000),
        "autonomous_system_organization": random_string(rng, 15),
        "city_name": random_string(rng, 10),
        "continent_code": random_string(rng, 2).to_uppercase(),
        "continent_name": random_string(rng, 10),
        "country_code": random_string(rng, 2).to_uppercase(),
        "country_name": random_string(rng, 10),
        "ip": random_ip(rng),
        "latitude": rng.gen_range(-90.0..90.0),
        "longitude": rng.gen_range(-180.0..180.0),
        "port": rng.gen_range(1..65535),
        "timezone": format!("UTC{:+}", rng.gen_range(-12..12))
    })
}

pub fn generate_connection_info() -> String {
    let rng = &mut rand::thread_rng();
    json!({
        "connection": {
            "app_protocol": random_string(rng, 5),
            "id": random_string(rng, 10),
            "ip_protocol": random_string(rng, 3),
            "local_ip": random_ip(rng),
            "remote_hostname": random_hostname(rng)
        },
        "data_stream": {
            "namespace": random_string(rng, 15)
        },
        "download": {
            "filename": random_string(rng, 10) + ".txt",
            "md5": random_string(rng, 32)
        },
        "dst": generate_location_data(rng),
        "job": random_string(rng, 8),
        "name": random_string(rng, 10),
        "origin": random_string(rng, 10),
        "sensor_uuid": random_string(rng, 36),
        "source_type": random_string(rng, 10),
        "src": generate_location_data(rng),
        "timestamp": std::time::SystemTime::now().duration_since(std::time::SystemTime::UNIX_EPOCH).unwrap().as_secs(),
    }).to_string()
}