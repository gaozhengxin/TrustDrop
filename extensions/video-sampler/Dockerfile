FROM trustdrop/elf-repro-runner:ubuntu-amd64

ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
    && apt-get install -y --no-install-recommends ffmpeg \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace/extensions/video-sampler
