# default recipe to display help information
default:
  @just --list

# build entire project
build:
    @cargo build

# run test on all workspace
test:
    @cargo test --workspace --all-features

# run clippy with heavy config
lint:
    @cargo clippy --workspace -- -D warnings    \
    -W clippy::complexity                       \
    -W clippy::correctness                      \
    -W clippy::nursery                          \
    -W clippy::perf                             \
    -W clippy::style                            \
    -W clippy::suspicious

# clean build directory
clean:
    @cargo clean

check-fmt:
    @cargo fmt --all --check

fmt:
    @cargo fmt

set positional-arguments

run rom:
    @cargo run $1

run-logger rom:
    @cargo run --features logger $1

run-disassembler rom:
    @cargo run --features disassembler $1
