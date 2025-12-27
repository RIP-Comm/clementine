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
    @cargo clippy --workspace

# clean build directory
clean:
    @cargo clean

# check formatting, return non-zero if not formatted
check-fmt:
    @cargo fmt --all --check

# format all code
fmt:
    @cargo fmt

set positional-arguments

# run <rom> in debug mode
run rom:
    @cargo run -- $1

# run <rom> in debug mode with logging to file
run-log rom:
    @cargo run -- $1 --log-to-file

# run <rom> in release mode, better for performance
run-release rom:
    @cargo run --release -- $1

# run <rom> in release mode with logging to file
run-release-log rom:
    @cargo run --release -- $1 --log-to-file

# generate and open documentation in browser (no dependencies, includes private items)
doc:
    @rm -rf target/doc
    @cargo doc --workspace --no-deps --document-private-items --open
