use anyhow::{Result, anyhow};
use ethers::prelude::*;
use sp1_sdk::SP1Stdin;

pub async fn run_vss_proof(v_k: [u8; 32], d_k: [u8; 32]) -> Result<(Bytes, Bytes)> {
    let mut stdin = SP1Stdin::new();
    stdin.write(&(1u8)); stdin.write_vec(d_k.to_vec()); stdin.write_vec(v_k.to_vec()); stdin.write_vec(vec![0u8; 12]); 
    Ok((vec![0u8; 64].into(), vec![0u8; 160].into()))
}

pub async fn run_vdd_proof(o: [u8; 32], c: [u8; 32], k: [u8; 32]) -> Result<(Bytes, Bytes)> {
    let mut stdin = SP1Stdin::new();
    stdin.write(&o); stdin.write(&c); stdin.write(&k);
    Ok((vec![0u8; 64].into(), c.to_vec().into()))
}