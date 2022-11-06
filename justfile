# default recipe to display help information
default:
  @just --list

# build entire project
build:
    @cargo build

# run test on all workspace
test:
    @cargo test --workspace

set positional-arguments

# run clemente in DEBUG mode
@debug *args='':
    @cargo run --features debug -- $1

# run clemente in DEBUG mode
@test_bitmap *args='':
    @cargo run --features test_bitmap -- $1

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