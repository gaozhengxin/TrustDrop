# RSLH/VE Principle Note

## Purpose

RSLH/VE is the VDD proof strategy used by TrustDrop to prove that an encrypted Walrus blob is derived from the original asset with the seller's asset encryption key, without proving byte-by-byte encryption for the whole file.

The current implementation lives mainly in:

- `drop-lib/src/rslh_ve.rs`
- `guest/vdd/program-vdd-walrus-rslhve/src/main.rs`
- `drop-script/src/main.rs`

## Core Idea

ChaCha8 is used as a stream cipher:

```text
cipher = plain XOR keystream
```

Red Stuff / RS2-style coding is linear over byte symbols. Because linear coding distributes over XOR:

```text
Encode(cipher)
  = Encode(plain XOR keystream)
  = Encode(plain) XOR Encode(keystream)
```

Therefore the verifier does not need to check every byte of the original file. It can sample encoded parity constraints and check that the sampled encoded plaintext shard, sampled encoded keystream shard, and sampled encoded ciphertext shard satisfy the same XOR relation.

In the current code, each sampled proof checks:

```text
origin_shard XOR encoded_keystream == cipher_shard
```

## Current Parameters

The grid constants are:

```text
ROW_WIDTH_PRIMARY = 334
COL_HEIGHT_SECONDARY = 667
DEFAULT_SAMPLE_COUNT = 15
```

The logical symbol size is dynamic and equals the Walrus RS2 symbol size for the committed blob:

```text
symbol_size = walrus_symbol_size(blob_len)
            = max(1, ceil(blob_len / (334 * 667))) rounded up to even bytes
```

Layout rule (transposed Walrus-matrix mapping so that one sampled column
exactly covers one Walrus message-matrix row):

- RSLH column `c` (0..334) = Walrus message-matrix row `c` (primary sliver `c`)
- RSLH row `r` (0..667) = Walrus message-matrix column `r`
- logical symbol `(row r, col c)` covers flat bytes `[(c*667+r)*s, +(s))`
- keystream cells are read at these same flat offsets (ChaCha8 byte-seek)

Each sample is a column-style linear constraint. One sampled column combines
`667` logical symbols (one Walrus message row), so one sample checks a relation
over many original symbols rather than one standalone file chunk.

The covered data window is exactly the Walrus message matrix area of the blob:
`334 * 667 * symbol_size` bytes, which equals `228124672` bytes when the
symbol size reaches the maximum 1024 multiple; smaller blobs use smaller
symbols and the window shrinks accordingly.

Assets whose committed blob exceeds the Walrus message matrix are rejected by
Walrus encoding itself. For the current prototype and File Mall use case this
is an acceptable product limit; larger assets should be rejected before
listing or split into multiple proof-covered segments.

The script derives sample indices as:

```text
seed = sha256(c_origin || c_cipher || c_key)
idx_i = sha256(seed || i) mod 1000
```

The guest then maps each index to:

```text
col_index = global_index mod 334
```

So the current proof should be understood as `15` sampled column constraints over a `334` column space, with possible repeated columns.

## Sampling Benefit

Without coding, `15` samples check only `15` individual symbols.

With the current column-coded sampling, `15` samples cover about:

```text
15 * 667 = 10005 logical symbol positions
```

Accounting for repeated columns, the expected number of unique sampled columns is about `14.69`, so the expected covered symbol positions are about:

```text
14.69 * 667 ~= 9799 symbols
```

This is about `653x` the direct coverage of uncoded sampling at the same sample count.

## Detection and Escape Probability

Use these definitions to avoid ambiguity:

```text
escape probability = probability that corruption exists but passes the sampled check
detection probability = 1 - escape probability
```

In some discussions this was called "miss rate", but that name is ambiguous. The preferred terms are:

- `escape probability`: lower is better.
- `detection probability`: higher is better.

For ordinary random or dispersed corruption, if the bad-symbol ratio is `f`, uncoded sampling with `k` samples has approximate miss probability:

```text
P_escape_uncoded ~= (1 - f)^k
P_detect_uncoded ~= 1 - (1 - f)^k
```

With column-coded sampling, each sampled constraint covers about `667` symbols. The approximate miss probability becomes:

```text
P_escape_coded ~= (1 - f)^(667k)
P_detect_coded ~= 1 - (1 - f)^(667k)
```

For the current `k = 15`, this is roughly equivalent to about `10005` uncoded samples under a random-error model.

Example estimates:

| Bad symbol ratio | Uncoded 15-sample escape probability | Current coded escape probability |
| ---: | ---: | ---: |
| 0.01% | 99.85% | 36.77% |
| 0.1% | 98.51% | 0.0045% |
| 0.5% | 92.76% | effectively 0 |
| 1% | 86.01% | effectively 0 |

Increasing the sample count has an exponential effect:

```text
P_escape ~= (1 - effective_bad_rate)^k
P_detect ~= 1 - P_escape
```

So a modest increase from `15` to `32`, `64`, or `128` samples can sharply reduce the miss rate, while the cost grows roughly linearly with the number of sampled constraints.

## Practical Value

The current design has practical value because it turns a small number of zkVM checks into wide linear constraints over the encoded data. This is especially useful for proving large Walrus assets where full byte-level encryption verification would be too expensive.

The current implementation binds the sampled column proofs to the committed ciphertext:

- `verify_walrus_blob_opening` verifies Merkle openings of the sampled shards up to
  the per-shard pair roots, the pair root tree, and the Walrus blob id (`c_cipher`).
- `verify_cipher_column_bound` opens, for each sampled column `c`, the data-region
  leaves of primary sliver `c` (Walrus row `c`) and recomputes the column GF
  aggregate from the real ciphertext symbols, then compares it byte-for-byte with
  the supplied `cipher_shard`.
- Because the opened symbols are Merkle-bound to `c_cipher`, tampering with the
  ciphertext changes the blob id (and therefore `c_cipher`) or is detected by the
  aggregate comparison; the proof therefore cannot be recomputed for a different
  ciphertext than the one that was committed.

It is still a probabilistic sampling proof, not a complete proof over every
byte. The column constraints are strong against random or broad corruption, but
out-of-blob bytes are covered by keystream cells only, and the origin side
(`origin_shard`) is bound through the homomorphism relation rather than through
independent Walrus openings.

The current implementation also has an effective file-size caveat: its sampled column construction only covers about `228.1 MB`. Larger files may still produce Walrus blob IDs, but the VDD proof does not currently constrain the tail beyond that covered window.

For stronger production security, consider:

- increasing `DEFAULT_SAMPLE_COUNT`;
- mixing row and column constraints instead of column-only checks;
- adding explicit sampled-shard opening proofs against the Walrus commitment model;
- splitting large assets into proof-covered segments, or extending the sampling construction to multiple windows;
- documenting the assumed adversary model for cancellation attacks inside one linear constraint.
