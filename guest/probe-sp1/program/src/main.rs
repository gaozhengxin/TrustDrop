#![no_main]
sp1_zkvm::entrypoint!(main);

pub fn main() {
    let n = sp1_zkvm::io::read::<u32>();

    let mut a = 0u32;
    let mut b = 1u32;
    for _ in 0..n {
        let next = a.wrapping_add(b);
        a = b;
        b = next;
    }

    sp1_zkvm::io::commit(&n);
    sp1_zkvm::io::commit(&a);
}
