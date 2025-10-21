# SPEAR on ARM64

This guide documents host requirements and build tips for running SPEAR on 64-bit ARM hardware such as AWS Graviton, Ampere Altra, and Apple Silicon devices using Linux.

## System dependencies

Most SPEAR runtime components are Go binaries that compile without changes on ARM64. The following native libraries must be available on the host before building:

- `flatbuffers-compiler` (`flatc`) version \>= 24
- PortAudio development headers: `portaudio19-dev` (or `libportaudio2`)
- X11 interaction libraries used by automation tooling: `libx11-dev`, `libxtst-dev`, `libxext-dev`
- Optional audio helpers: `libasound2-dev`
- Docker Engine 24+ with BuildKit / buildx enabled

On Debian/Ubuntu based distributions:

```bash
sudo apt-get update
sudo apt-get install -y \
  build-essential pkg-config cmake \
  libx11-dev libxext-dev libxtst-dev \
  libasound2-dev libportaudio2 portaudio19-dev \
  flatbuffers-compiler docker.io
python3 -m pip install --upgrade pip build websocket-client isort
```

> **Tip:** When running on an ARM Mac with Linux VMs (Multipass, UTM, etc.) ensure virtualization extensions are enabled to run Docker.

## Building SPEAR

1. Generate FlatBuffers bindings and the Go proto package:
   ```bash
   make pkg/spear
   ```
2. Build the native spearlet binary for your host architecture:
   ```bash
   make spearlet
   ```
3. To create a Linux ARM64 spearlet from an x86_64 development host, use the new helper target:
   ```bash
   make spearlet-linux-arm64
   ```

The cross-compiled binary is saved under `bin/linux-arm64/spearlet`.

## Workload containers

SPEAR ships Docker-based workloads (Python and Go demo agents). These now respect the `PLATFORM` flag via `DOCKER_DEFAULT_PLATFORM`, enabling reproducible images for both `linux/amd64` and `linux/arm64`.

- Build workloads for the host architecture (default):
  ```bash
  make workload
  ```
- Build Linux ARM64 workloads on any machine with buildx:
  ```bash
  make workload-linux-arm64
  ```
- Build Linux AMD64 workloads (useful when developing on ARM hardware but targeting x86):
  ```bash
  make workload-linux-amd64
  ```

Ensure your Docker installation has a builder capable of multi-architecture builds:

```bash
docker buildx ls | grep -q "linux/arm64" || docker buildx create --use
```

## Runtime notes

- Vector store helper containers (Qdrant) publish multi-arch images, so no changes are required; Docker will pull the appropriate architecture automatically.
- Go dependencies such as `portaudio`, `robotgo`, and `kbinani/screenshot` rely on system libraries. When running headless on ARM servers, disable features requiring a GUI by setting the appropriate SPEAR runtime flags.
- For audio capture on devices without microphone hardware, consider mocking those hostcalls or running in dry-run mode.

## Troubleshooting

- **Missing flatc**: reinstall `flatbuffers-compiler` or build from source following the upstream README.
- **`docker compose` uses wrong architecture**: confirm `DOCKER_DEFAULT_PLATFORM` is exported or use the make targets above.
- **CGO cross-compilation errors**: install ARM64 toolchains (`gcc-aarch64-linux-gnu`) when building `spearlet-linux-arm64` on x86_64 hosts.

With these steps, SPEAR should build and run natively on ARM64 hosts as well as produce ARM64 artifacts from other architectures.
