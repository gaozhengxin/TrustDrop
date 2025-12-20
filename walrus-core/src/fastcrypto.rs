use core::fmt::Debug;
use core::fmt;
use digest::OutputSizeUser;
use generic_array::GenericArray;
use serde::{ Deserialize, Serialize };
use serde_with::serde_as;
use alloc::vec::Vec;

use base64;

/// Represents a digest of `DIGEST_LEN` bytes.
#[serde_as]
#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize, Ord, PartialOrd, Copy)]
pub struct Digest<const DIGEST_LEN: usize> {
    #[serde_as(as = "[_; DIGEST_LEN]")]
    pub digest: [u8; DIGEST_LEN],
}

impl<const DIGEST_LEN: usize> Digest<DIGEST_LEN> {
    /// Create a new digest containing the given bytes
    pub fn new(digest: [u8; DIGEST_LEN]) -> Self {
        Digest { digest }
    }

    /// Copy the digest into a new vector.
    pub fn to_vec(&self) -> Vec<u8> {
        self.digest.to_vec()
    }

    /// The size of this digest in bytes.
    pub fn size(&self) -> usize {
        DIGEST_LEN
    }
}

impl<const DIGEST_LEN: usize> fmt::Debug for Digest<DIGEST_LEN> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}", base64::encode(self.digest))
    }
}

impl<const DIGEST_LEN: usize> fmt::Display for Digest<DIGEST_LEN> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}", base64::encode(self.digest))
    }
}

impl<const DIGEST_LEN: usize> AsRef<[u8]> for Digest<DIGEST_LEN> {
    fn as_ref(&self) -> &[u8] {
        self.digest.as_ref()
    }
}

impl<const DIGEST_LEN: usize> From<Digest<DIGEST_LEN>> for [u8; DIGEST_LEN] {
    fn from(digest: Digest<DIGEST_LEN>) -> Self {
        digest.digest
    }
}

pub trait HashFunction<const DIGEST_LENGTH: usize>: Default {
    /// The length of this hash functions digests in bytes.
    const OUTPUT_SIZE: usize = DIGEST_LENGTH;

    /// Create a new hash function of the given type
    fn new() -> Self {
        Self::default()
    }

    /// Process the given data, and update the internal of the hash function.
    fn update<Data: AsRef<[u8]>>(&mut self, data: Data);

    /// Retrieve result and consume hash function.
    fn finalize(self) -> Digest<DIGEST_LENGTH>;

    /// Compute the digest of the given data and consume the hash function.
    fn digest<Data: AsRef<[u8]>>(data: Data) -> Digest<DIGEST_LENGTH> {
        let mut h = Self::default();
        h.update(data);
        h.finalize()
    }

    /// Compute a single digest from all slices in the iterator in order and consume the hash function.
    fn digest_iterator<K: AsRef<[u8]>, I: Iterator<Item = K>>(iter: I) -> Digest<DIGEST_LENGTH> {
        let mut h = Self::default();
        iter.for_each(|item| h.update(item));
        h.finalize()
    }
}

/// This trait is implemented by all messages that can be hashed.
pub trait Hash<const DIGEST_LEN: usize> {
    /// The type of the digest when this is hashed.
    type TypedDigest: Into<Digest<DIGEST_LEN>> + Eq + Copy + Debug;

    fn digest(&self) -> Self::TypedDigest;
}

/// This wraps a [digest::Digest] as a [HashFunction].
#[derive(Default)]
pub struct HashFunctionWrapper<Variant, const DIGEST_LEN: usize>(Variant);

pub type Blake2b256 = HashFunctionWrapper<blake2::Blake2b<typenum::U32>, 32>;

impl<Variant: digest::Digest + Default, const DIGEST_LEN: usize> HashFunction<DIGEST_LEN>
for HashFunctionWrapper<Variant, DIGEST_LEN> {
    fn update<Data: AsRef<[u8]>>(&mut self, data: Data) {
        self.0.update(data);
    }

    fn finalize(self) -> Digest<DIGEST_LEN> {
        let result = self.0.finalize();

        let mut digest = [0u8; DIGEST_LEN];

        digest.copy_from_slice(result.as_slice());

        Digest { digest }
    }
}
