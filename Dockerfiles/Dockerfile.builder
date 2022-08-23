# docker build -t builder:latest -f Dockerfiles/Dockerfile.builder .

FROM debian:10-slim

RUN apt update
RUN apt install -y build-essential curl pkg-config libssl-dev
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y