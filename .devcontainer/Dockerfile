# Pinned to an Ubuntu release so a new LTS never arrives unannounced;
# docker-compose.yml pulls on every build for the release's security updates.
FROM mcr.microsoft.com/devcontainers/base:ubuntu-24.04

# Install development tools
# hadolint ignore=DL3009
RUN apt-get update && apt-get install -y --no-install-recommends \
    less \
    procps \
    fzf \
    man-db \
    unzip \
    gnupg2 \
    dnsutils \
    jq \
    nano \
    vim \
    git \
    fish \
    sox \
    # Audio passthrough for docker-compose.audio.yml; inert without it
    pulseaudio-utils \
    alsa-utils \
    libasound2-plugins \
    # Fast linker for Rust builds (cargo/config.toml, copied in post-create.sh)
    mold \
    # protoc for the build scripts of LanceDB's lance crates (abyme-index),
    # and the well-known .proto files their protos import
    protobuf-compiler \
    libprotobuf-dev \
    # Tauri's Linux prerequisites for the canvas (abyme-canvas): WebKitGTK
    # and the libraries and tools its build uses
    libwebkit2gtk-4.1-dev \
    build-essential \
    curl \
    wget \
    file \
    libxdo-dev \
    libssl-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev

# Route ALSA through PulseAudio so applications that use ALSA directly
# don't fail when /dev/snd is unavailable (e.g., VM or remote host).
RUN printf 'pcm.default pulse\nctl.default pulse\n' > /etc/asound.conf

# The Tauri CLI 2.12.4 for the canvas (abyme-canvas), prebuilt by the Tauri
# project. The archive is checked against the digest of its GitHub release
# before anything reads it; cargo then finds the binary as `cargo tauri`.
RUN set -eu; \
    case "$(dpkg --print-architecture)" in \
      amd64) target=x86_64-unknown-linux-gnu; \
        digest=820ae3c7580ca637cad15f65638960ee2a96e5a491e224691c86d6f25e6c9e8a ;; \
      arm64) target=aarch64-unknown-linux-gnu; \
        digest=4f601b9f462c3d3d10ce67b6ae963f099f6a5a3253920dae95a253abd558d723 ;; \
      *) echo "no Tauri CLI build for $(dpkg --print-architecture)" >&2; exit 1 ;; \
    esac; \
    tmp=$(mktemp -d); \
    curl -fsSL --proto '=https' --tlsv1.2 -o "$tmp/cargo-tauri.tgz" \
      "https://github.com/tauri-apps/tauri/releases/download/tauri-cli-v2.12.4/cargo-tauri-$target.tgz"; \
    printf '%s  %s\n' "$digest" "$tmp/cargo-tauri.tgz" > "$tmp/sha256sum"; \
    sha256sum -c "$tmp/sha256sum"; \
    tar -xzf "$tmp/cargo-tauri.tgz" -C "$tmp" cargo-tauri; \
    install -o root -g root -m 0755 "$tmp/cargo-tauri" /usr/local/bin/cargo-tauri; \
    rm -rf "$tmp"

# Set `DEVCONTAINER` environment variable to help with orientation
ENV DEVCONTAINER=true

# Create workspace directory and set permissions
RUN mkdir -p /workspace && \
    chown -R vscode:vscode /workspace

WORKDIR /workspace

# Switch to non-root user
USER vscode

# The installer leaves a stub ~/.claude.json (no onboarding, no account); in
# the image it would win post-start.sh's one-time fill of the config on the
# volume over the host copy.
RUN curl -fsSL https://claude.ai/install.sh | bash \
    && rm -f /home/vscode/.claude.json

# Configure and set the default shell to fish
COPY --chown=vscode:vscode fish/config.fish /home/vscode/.config/fish/config.fish
ENV SHELL=/usr/bin/fish

# Set the default editor and visual
ENV EDITOR=nano
ENV VISUAL=nano

RUN curl -sS https://starship.rs/install.sh | sh -s -- --yes
