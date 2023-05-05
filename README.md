# Cargo Musl Builder

Cargo Musl Builder is a CLI tool that builds your Rust code as a MUSL static binary inside a Docker container.

It decreases consequent build times dramatically by utilizing Dockers caching along with [cargo-chef](https://github.com/LukeMathWalker/cargo-chef), especially for cargo workspaces.

It works by using [Tera](https://github.com/Keats/tera) to input your CLI arguments into a Dockerfile template, which is then saved in a temporary file and built.

After the build finishes, the binary gets copied out of the container in a `.zip` file. Ready to upload to your server or cloud provider.

I built this tool because we heavily use Lambda functions for [Råd til Bolig](https://raadtilbolig.dk/), and the build times were getting crazy.

## Installation

You can install this tool directly from GitHub using Cargo:

```bash
cargo install --git https://github.com/impl-rs/cargo-musl-builder
```

## Usage

You can use this tool by running `cargo-musl-builder` in your project directory.

```bash
cargo-musl-builder -p ./rust --bin theapi build
```

It will build the binary in the `./rust` directory using the `theapi` binary target. In a folder with the following structure

```
├── rust
│   ├── theapi
│   │   ├── Cargo.toml
│   ├── Cargo.toml
```
