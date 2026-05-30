FROM rust:slim-bookworm

RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    git \
    zsh \
    curl \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - && \
    apt-get install -y nodejs

RUN chsh -s $(which zsh)

RUN ARCH=$(uname -m) && \
    if [ "$ARCH" = "x86_64" ]; then \
        EZA_URL="https://github.com/eza-community/eza/releases/latest/download/eza_x86_64-unknown-linux-gnu.tar.gz"; \
    else \
        EZA_URL="https://github.com/eza-community/eza/releases/latest/download/eza_aarch64-unknown-linux-gnu.tar.gz"; \
    fi && \
    curl -fsSL "$EZA_URL" | tar xz -C /usr/local/bin && \
    chmod +x /usr/local/bin/eza

RUN sh -c "$(curl -fsSL https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/tools/install.sh)" "" --unattended

RUN git clone --depth=1 https://github.com/romkatv/powerlevel10k.git /root/.oh-my-zsh/custom/themes/powerlevel10k && \
    git clone https://github.com/zsh-users/zsh-autosuggestions /root/.oh-my-zsh/custom/plugins/zsh-autosuggestions && \
    git clone https://github.com/zsh-users/zsh-syntax-highlighting.git /root/.oh-my-zsh/custom/plugins/zsh-syntax-highlighting

COPY ./config/.zshrc /root/.zshrc
COPY ./config/.p10k.zsh /root/.p10k.zsh

RUN curl -fsSL https://claude.ai/install.sh | bash

RUN rustup component add rustfmt clippy

WORKDIR /workspace

CMD ["zsh"]