# Video sampler extension

This extension parses one MP4 video track, derives three deterministic sample windows, uses `ffmpeg` stream copy to create three playable MP4 previews, and uploads those previews to public IPFS through Pinata. It checks every CID returned by Pinata against the locally computed CID before writing `manifest.json`.

## Pinata credentials

Keep credentials outside the repository. Point `PINATA_CONFIG_FILE` at a mounted file containing either a JWT:

```text
PINATA_JWT=...
```

or a legacy key pair:

```text
PINATA_API_KEY=...
PINATA_SECRET_API_KEY=...
```

Uploading is enabled by default. Set `PINATA_UPLOAD=0` only for an explicit local-only run.

The commands below assume they are run from the repository root. The input fixture can be replaced with another supported MP4 file.

## macOS: run in a temporary Ubuntu container

Prerequisites:

- Docker Desktop or another Docker-compatible runtime is running.
- The base image `trustdrop/elf-repro-runner:ubuntu-amd64` is available locally.

Build the Ubuntu development image:

```bash
docker build \
  --platform linux/amd64 \
  --tag trustdrop/video-sampler-dev:ubuntu-amd64 \
  extensions/video-sampler
```

Compile and run the sampler in a disposable container:

```bash
mkdir -p /tmp/trustdrop-video-sampler-output

docker run --rm \
  --platform linux/amd64 \
  --volume "$PWD:/workspace:ro" \
  --volume /tmp/trustdrop-video-sampler-output:/output \
  --volume /absolute/path/pinata.env:/run/secrets/pinata.env:ro \
  --env PINATA_CONFIG_FILE=/run/secrets/pinata.env \
  --workdir /workspace/extensions/video-sampler \
  trustdrop/video-sampler-dev:ubuntu-amd64 \
  bash -lc 'cargo run --release -- \
    /workspace/drop-lib/tests/fixtures/how-a-mosquito-operates-1912.mp4 \
    /output/how-a-mosquito-operates-1912'
```

The container is removed after the command exits. Generated files remain on macOS under:

```text
/tmp/trustdrop-video-sampler-output/how-a-mosquito-operates-1912/
```

## Linux: run directly with Cargo

Prerequisites:

- A Rust toolchain compatible with the repository.
- `ffmpeg` is installed and available on `PATH`.

From the repository root:

```bash
cd extensions/video-sampler

cargo run --release -- \
  ../../drop-lib/tests/fixtures/how-a-mosquito-operates-1912.mp4 \
  /tmp/how-a-mosquito-operates-1912
```

For Linux, set `PINATA_CONFIG_FILE` to the credential file path before running the command.

## Expected output

Both commands create the same output layout:

```text
how-a-mosquito-operates-1912/
├── manifest.json
├── preview.html
├── preview-0.mp4
├── preview-1.mp4
└── preview-2.mp4
```

`manifest.json` records the deterministic sampling seed, source binding placeholder, sample ranges, preview CID, `ipfs://` URL, and evidence CID for each window. `preview.html` is only a local viewer for the three generated previews.
