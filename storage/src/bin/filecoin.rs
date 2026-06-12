use storage::{ FilecoinConfig, filecoin::FilecoinClient, BlobStatus, StorageNetwork, BlobId };
use std::env;
use dotenv::dotenv;

#[tokio::main]
async fn main() {
    dotenv().ok();

    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| {
        eprintln!("Usage: <cmd> [args...]");
        std::process::exit(1);
    });

    let api_key = env::var("LIGHTHOUSE_API_KEY").expect("LIGHTHOUSE_API_KEY missing");
    let cfg = FilecoinConfig::new(api_key);

    let client = FilecoinClient::new(cfg);

    match cmd.as_str() {
        "upload" => {
            let mut input = None;
            let mut epoch_arg = None;
            while let Some(a) = args.next() {
                match a.as_str() {
                    "--input" => {
                        input = args.next();
                    }
                    "--epoch" => {
                        epoch_arg = args.next();
                    }
                    _ => {}
                }
            }

            let path = input.expect("--input required");
            match client.upload_file(path, epoch_arg.as_deref()).await {
                Ok(blob_id) => println!("Uploaded. CID: {}", blob_id.0),
                Err(e) => eprintln!("Upload failed: {:?}", e),
            }
        }

        "status" => {
            let mut blob_arg = None;
            while let Some(a) = args.next() {
                if a == "--cid" {
                    blob_arg = args.next();
                }
            }

            let blob = BlobId(blob_arg.expect("--cid required"));
            match client.get_status(&blob).await {
                Ok(status) =>
                    match status {
                        BlobStatus::InfoFC { cid, deal_id, start_epoch, end_epoch, status } => {
                            println!("CID: {}", cid);
                            println!("Deal ID: {}", deal_id);
                            println!("Start: {}", start_epoch);
                            println!("End: {}", end_epoch);
                            println!("Status: {}", status);
                        }
                        BlobStatus::Error(msg) => {
                            eprintln!("Status Error: {}", msg);
                            std::process::exit(1);
                        }
                        _ => {
                            println!("Status: {:?}", status);
                        }
                    }
                Err(e) => {
                    eprintln!("Network or Storage Error: {:?}", e);
                    std::process::exit(1);
                }
            }
        }

        "download" => {
            let mut blob_arg = None;
            let mut output_arg = None;
            while let Some(a) = args.next() {
                match a.as_str() {
                    "--cid" => {
                        blob_arg = args.next();
                    }
                    "--output" => {
                        output_arg = args.next();
                    }
                    _ => {}
                }
            }

            let blob = BlobId(blob_arg.expect("--blob required"));
            let out = output_arg.expect("--output required");

            match client.download_file(&blob, out).await {
                Ok(()) => println!("Downloaded successfully."),
                Err(e) => eprintln!("Download failed: {:?}", e),
            }
        }

        "cid" => {
            use drop_lib::cid;
            let mut input = None;
            while let Some(a) = args.next() {
                if a == "--input" {
                    input = args.next();
                }
            }
            let path = input.expect("--input required");

            // 直接读取整个文件到内存 (支持 1GB+)
            let data = std::fs::read(&path).expect("Failed to read file");

            // 调用 lib 里的计算函数
            let cid_str = cid::compute_ipfs_cid(&data);

            println!("File: {}", path);
            println!("Size: {} bytes", data.len());
            println!("Local CID: {}", cid_str);
        }

        _ => eprintln!("Unknown command"),
    }
}
