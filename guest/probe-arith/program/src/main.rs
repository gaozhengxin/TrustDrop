#![no_main]
sp1_zkvm::entrypoint!(main);

pub fn main() {
    use sp1_zkvm::io::{commit, read};

    let a = read::<u32>();
    let b = read::<u32>();

    let sum = a.wrapping_add(b);
    let product = a.wrapping_mul(b);
    let mixed = sum.rotate_left(7) ^ product.rotate_right(3);

    commit(&a);
    commit(&b);
    commit(&sum);
    commit(&product);
    commit(&mixed);
}
