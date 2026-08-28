# Video sampler extension

This extension parses one MP4 video track, derives three deterministic sample windows, writes their evidence CIDs to `manifest.json`, and uses `ffmpeg` stream copy to create three playable MP4 previews.

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

`manifest.json` records the deterministic sampling seed, source binding placeholder, sample ranges, and evidence CID for each window. `preview.html` is only a local viewer for the three generated previews.
