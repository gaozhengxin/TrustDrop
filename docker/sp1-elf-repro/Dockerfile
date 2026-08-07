# TrustDrop SP1 guest ELF reproducibility runner.
#
# This image provides the Ubuntu/amd64 userspace and SP1 tools used to build
# TrustDrop guest ELFs. Bit-for-bit matching also depends on the run-time mount
# paths and Cargo cache paths described in .codex/runbooks/sp1-elf-repro-build.md.

FROM ubuntu:22.04

ARG DEBIAN_FRONTEND=noninteractive
ARG RUSTUP_INIT_VERSION=1.28.2
ARG CARGO_PROVE_VERSION=6.2.4

ENV PATH=/root/.cargo/bin:/root/.sp1/bin:${PATH}
ENV RUSTUP_TOOLCHAIN=stable-x86_64-unknown-linux-gnu
ENV LC_ALL=C
ENV LANG=C

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        build-essential \
        pkg-config \
        libssl-dev \
        git \
        clang \
        cmake \
        protobuf-compiler \
        file \
        xz-utils \
        bash \
    && rm -rf /var/lib/apt/lists/*

RUN curl -fsSL "https://static.rust-lang.org/rustup/archive/${RUSTUP_INIT_VERSION}/x86_64-unknown-linux-gnu/rustup-init" -o /tmp/rustup-init \
    && chmod +x /tmp/rustup-init \
    && /tmp/rustup-init -y --default-toolchain stable-x86_64-unknown-linux-gnu \
    && rm /tmp/rustup-init \
    && rustup --version \
    && cargo --version

RUN curl -fsSL https://sp1up.succinct.xyz -o /tmp/sp1up \
    && bash /tmp/sp1up \
    && rm /tmp/sp1up \
    && /root/.sp1/bin/sp1up -v "${CARGO_PROVE_VERSION}" \
    && cargo prove --version \
    && rustc +succinct --version

WORKDIR /home/justin/TrustDrop/TrustDrop
