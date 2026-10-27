# agents-ui as a read-only swarm dashboard: `agents-tui --headless`.
#
#   docker build -t agents-ui .
#   docker run -p 7878:7878 agents-ui --temporal agents --temporal-url http://temporal:7243
#
# The image watches anything it can reach over the network (Temporal's HTTP
# API). tmux/herdr swarms and Claude Code sub-agents are read from the local
# machine, so a container only sees those if the agents run beside it.

FROM rust:1.90-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim
# curl reads Temporal's HTTP API; git and gh read a repo's issues and
# pipeline refs when one is mounted in.
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl git gh \
 && rm -rf /var/lib/apt/lists/*
RUN useradd --uid 10001 --create-home agents
COPY --from=build /src/target/release/agents-tui /usr/local/bin/agents-tui
USER agents
EXPOSE 7878
ENTRYPOINT ["agents-tui", "--headless", "--web-bind", "0.0.0.0", "--read-only"]
