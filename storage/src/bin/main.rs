use storage::{ClientConfig, WalrusClient, StorageNetwork, BlobId};
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    // skip program name
    let mut args = std::env::args().skip(1);

    let cmd = args.next().unwrap_or_else(|| {
        eprintln!("Commands:");
        eprintln!("  upload   --input <path> [--epoch <num>]");
        eprintln!("  status   --blob <blob_id>");
        eprintln!("  download --blob <blob_id> --output <path>");
        std::process::exit(1);
    });

    // instantiate client from your default config helper
    let cfg = default_config();
    let client = WalrusClient::new(cfg);

    match cmd.as_str() {
        "upload" => {
            // parse flags: --input <path>  optional: --epoch <num>
            let mut input: Option<String> = None;
            let mut epoch_arg: Option<String> = None;

            while let Some(a) = args.next() {
                match a.as_str() {
                    "--input" => {
                        input = args.next();
                    }
                    "--epoch" => {
                        epoch_arg = args.next();
                    }
                    other => {
                        eprintln!("Unknown argument for upload: {}", other);
                        std::process::exit(1);
                    }
                }
            }

            let input = input.unwrap_or_else(|| {
                eprintln!("upload requires --input <path>");
                std::process::exit(1);
            });

            // Validate epoch if provided; keep string around so &str lives across await
            let epoch_string: Option<String> = match epoch_arg {
                Some(s) => {
                    if s.parse::<u32>().is_err() {
                        eprintln!("--epoch value must be an integer: '{}'", s);
                        std::process::exit(1);
                    }
                    Some(s)
                }
                None => None,
            };
            // produce Option<&str> that borrows from epoch_string (must live until await returns)
            let epoch_opt: Option<&str> = epoch_string.as_deref();

            // Call upload_file (uses your trait's default impl which reads the file etc.)
            match client.upload_file(input, epoch_opt).await {
                Ok(blob_id) => {
                    println!("Uploaded. blob id = {}", blob_id.0);
                }
                Err(e) => {
                    eprintln!("Upload failed: {:?}", e);
                    std::process::exit(1);
                }
            }
        }

        "status" => {
            // parse --blob <blob_id>
            let mut blob_arg: Option<String> = None;
            while let Some(a) = args.next() {
                match a.as_str() {
                    "--blob" => blob_arg = args.next(),
                    other => {
                        eprintln!("Unknown argument for status: {}", other);
                        std::process::exit(1);
                    }
                }
            }

            let blob_str = blob_arg.unwrap_or_else(|| {
                eprintln!("status requires --blob <blob_id>");
                std::process::exit(1);
            });

            let blob = BlobId(blob_str);

            match client.get_status(&blob).await {
                Ok(status) => {
                    println!("Status: {:#?}", status);
                }
                Err(e) => {
                    eprintln!("Status query failed: {:?}", e);
                    std::process::exit(1);
                }
            }
        }

        "download" => {
            // parse --blob <blob_id> --output <path>
            let mut blob_arg: Option<String> = None;
            let mut output_arg: Option<String> = None;

            while let Some(a) = args.next() {
                match a.as_str() {
                    "--blob" => blob_arg = args.next(),
                    "--output" => output_arg = args.next(),
                    other => {
                        eprintln!("Unknown argument for download: {}", other);
                        std::process::exit(1);
                    }
                }
            }

            let blob_str = blob_arg.unwrap_or_else(|| {
                eprintln!("download requires --blob <blob_id> --output <path>");
                std::process::exit(1);
            });

            let out = output_arg.unwrap_or_else(|| {
                eprintln!("download requires --blob <blob_id> --output <path>");
                std::process::exit(1);
            });

            let blob = BlobId(blob_str);
            // call trait convenience method download_file (which uses download_blob + write_file)
            match client.download_file(&blob, out).await {
                Ok(()) => {
                    println!("Downloaded successfully.");
                }
                Err(e) => {
                    eprintln!("Download failed: {:?}", e);
                    std::process::exit(1);
                }
            }
        }

        other => {
            eprintln!("Unknown command: {}", other);
            std::process::exit(1);
        }
    }
}

/// Default client config used by demo CLI
fn default_config() -> ClientConfig {
    ClientConfig {
        publisher_url: "http://127.0.0.1:31415".into(),
        aggregator_url: "http://127.0.0.1:31415".into(),
        //aggregator_url: "https://walrus.blockscope.net".into(),
        blockberry_base: "https://api.blockberry.one/walrus-mainnet".into(),
        api_key: "eNx0cS4PemfQtVaArXbRbHcyJTnP0l".into(),
        send_object_to: None,
    }
}
