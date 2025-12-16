use alloy_sol_types::SolType;
use clap::Parser;
use sp1_sdk::{include_elf, ProverClient, SP1Stdin};
use rand::RngCore;

use maenad_lib::merkle;
use rand::rng;
use blake3;

pub const VDD_ELF: &[u8] = include_elf!("vdd-program");

/// The arguments for the command.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    execute: bool,

    #[arg(long)]
    prove: bool,

    // kept for compatibility though not used by this guest
    #[arg(long, default_value = "20")]
    n: u32,
}

fn bytes_to_hex_prefix(b: &[u8], prefix_len: usize) -> String {
    let mut out = String::new();
    let mut first = true;
    for &byte in b.iter().take(prefix_len) {
        if !first {
            out.push_str("");
        }
        first = false;
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

fn main() {
    // Setup the logger.
    sp1_sdk::utils::setup_logger();
    dotenv::dotenv().ok();

    // Parse the command line arguments.
    let args = Args::parse();

    if args.execute == args.prove {
        eprintln!("Error: You must specify either --execute or --prove");
        std::process::exit(1);
    }

    // Setup the prover client.
    let client = ProverClient::from_env();

    const CHUNK_SIZE: usize = 1024 * 1024;

    // === Prepare inputs on host ===
    const ORIGIN_SIZE: u32 = 300 * 1024 * 1024;

    let mut origin = vec![0u8; ORIGIN_SIZE.try_into().unwrap()];
    rand::thread_rng().fill_bytes(&mut origin);

    // generate random 32-byte key
    let mut key_arr = [0u8; 32];
    rng().fill_bytes(&mut key_arr);
    let key_vec: Vec<u8> = key_arr.to_vec();

    // compute commitments
    let origin_mkt = merkle::build_merkle_tree(origin.as_slice(), CHUNK_SIZE);
    let origin_mkt_root = origin_mkt.root();
    let c_origin: Vec<u8> = origin_mkt_root.to_vec();

    let c_key_hash = blake3::hash(&key_vec);
    let c_key: Vec<u8> = c_key_hash.as_bytes().to_vec();

    // compute cipher
    let cipher_mkt = merkle::encrypt_merkle_tree(&origin_mkt, &key_arr).expect("data encryption failed");

    // compute cipher commitment
    let cipher_mkt_root = cipher_mkt.root();
    let c_cipher: Vec<u8> = cipher_mkt_root.to_vec();

    // combined expected public output (what guest commit_slice will commit)
    let mut combined_expected: Vec<u8> = Vec::with_capacity(128);
    combined_expected.extend_from_slice(&c_origin);
    combined_expected.extend_from_slice(&c_key);
    combined_expected.extend_from_slice(&c_cipher);
    combined_expected.extend_from_slice(&ORIGIN_SIZE.to_be_bytes());

    // print some info
    println!("Prepared inputs:");
    println!("  origin bytes: {}", origin.len());
    println!("  key: 32 bytes");
    println!("  c_origin (merkle root): {}", bytes_to_hex_prefix(&c_origin, 32));
    println!("  c_key    (blake3): {}", bytes_to_hex_prefix(&c_key, 32));
    println!("  c_cipher (merkle root): {}", bytes_to_hex_prefix(&c_cipher, 32));

    // === Build SP1 stdin ===
    let mut stdin = SP1Stdin::new();
    // order matters and must match guest read_vec order:
    // c_origin, c_key, c_cipher, origin, key
    stdin.write(&c_origin);
    stdin.write(&c_key);
    stdin.write(&c_cipher);
    stdin.write(&origin);
    stdin.write(&key_vec);

    if args.execute {
        // Execute the program
        let (output, report) = client.execute(VDD_ELF, &stdin).run().unwrap();
        println!("Program executed successfully (execute mode).");

        // output should be the combined bytes committed by guest
        println!("Returned public output length: {}", output.as_slice().len());
        if output.as_slice() == combined_expected.as_slice() {
            println!("Output matches expected combined commitments.");
        } else {
            println!("Output DOES NOT match expected combined commitments!");
            // print some diagnostics
            let show = 64.min(output.as_slice().len());
            println!("  returned first {} bytes hex: {}", show, bytes_to_hex_prefix(&output.as_slice(), show));
            println!("  expected first {} bytes hex: {}", show, bytes_to_hex_prefix(&combined_expected.as_slice(), show));
        }

        // print a short readable summary
        println!("Merkle root (host calc): {}", bytes_to_hex_prefix(&c_cipher, 32));
        println!("Output sample (first 64 bytes hex): {}", bytes_to_hex_prefix(&output.as_slice(), 64));

        println!("Number of cycles executed: {}", report.total_instruction_count());

        // 打印内存使用情况
        println!("Unique Memory Touched:    {} addresses", report.touched_memory_addresses);

        // 打印 Gas 消耗（如果可用）
        if let Some(gas) = report.gas {
            println!("Gas Used (Estimated):     {}", gas);
        } else {
            println!("Gas Used:                 (Not Calculated)");
        }
        println!("---------------------------------");
    } else {
        // prove path: setup, prove, verify
        let (pk, vk) = client.setup(VDD_ELF);

        let proof = client
            .prove(&pk, &stdin)
            .run()
            .expect("failed to generate proof");

        println!("Successfully generated proof!");

        client.verify(&proof, &vk).expect("failed to verify proof");
        println!("Successfully verified proof!");

        // additionally run execute locally to obtain and print the public output
        let (output, _report) = client.execute(VDD_ELF, &stdin).run().unwrap();
        println!("Also executed to obtain public output (prove mode).");
        println!("Returned public output length: {}", output.as_slice().len());
        println!("Output sample (first 64 bytes hex): {}", bytes_to_hex_prefix(&output.as_slice(), 64));
        if output.as_slice() == combined_expected.as_slice() {
            println!("Output matches expected combined commitments.");
        } else {
            println!("Output DOES NOT match expected combined commitments!");
        }
    }
}
