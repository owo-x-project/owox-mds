FROM mcr.microsoft.com/devcontainers/base:ubuntu-24.04

ARG NODE_MAJOR=24
ARG RUST_VERSION=stable
ARG CODEX_VERSION=latest
ENV DEBIAN_FRONTEND=noninteractive

# ---------- System packages ----------
RUN apt-get update && apt-get install -y --no-install-recommends \
  ca-certificates \
  curl \
  git \
  git-lfs \
  gh \
  jq \
  less \
  unzip \
  zip \
  sudo \
  openssh-client \
  bash-completion \
  build-essential \
  pkg-config \
  make \
  procps \
  ripgrep \
  fd-find \
  tree \
  bubblewrap \
  && rm -rf /var/lib/apt/lists/*

# ---------- JavaScript/TypeScript Dev Environment ----------
RUN mkdir -p /etc/apt/keyrings \
  && curl -fsSL https://deb.nodesource.com/gpgkey/nodesource-repo.gpg.key \
  | gpg --dearmor -o /etc/apt/keyrings/nodesource.gpg \
  && echo "deb [signed-by=/etc/apt/keyrings/nodesource.gpg] https://deb.nodesource.com/node_${NODE_MAJOR}.x nodistro main" \
  > /etc/apt/sources.list.d/nodesource.list \
  && apt-get update \
  && apt-get install -y --no-install-recommends nodejs \
  && rm -rf /var/lib/apt/lists/*

# ---------- Rust Dev Environment ----------
ENV RUSTUP_HOME=/usr/local/rustup
ENV CARGO_HOME=/usr/local/cargo
ENV PATH=/usr/local/cargo/bin:/usr/local/bin:${PATH}

RUN curl -fsSL https://sh.rustup.rs \
  | sh -s -- -y --profile minimal --default-toolchain ${RUST_VERSION} \
  && rustup component add rustfmt clippy \
  && chmod -R a+w ${RUSTUP_HOME} ${CARGO_HOME}

# ---------- Codex ----------
RUN npm install -g @openai/codex@${CODEX_VERSION}

# ---------- RTK ----------
RUN curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/refs/heads/master/install.sh \
  | RTK_INSTALL_DIR=/usr/local/bin sh

RUN mkdir -p \
  /home/vscode/.local/bin \
  && chown -R vscode:vscode /home/vscode

WORKDIR /workspace
