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
    @cargo run $1

# run <rom> in debug mode with logger feature
run-logger rom:
    @cargo run --features logger $1

# run <rom> in debug mode with disassembler feature
run-disassembler rom:
    @cargo run --features disassembler $1

# run <rom> in release mode, better for performance and animations
run-release rom:
    @cargo run --release $1

# run <rom> in debug mode with logger and disassembler features
run-all-debug rom:
    @cargo run --features logger --features disassembler $1
