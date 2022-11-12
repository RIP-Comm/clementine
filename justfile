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
@test_mode_3 *args='':
    @cargo run --features test_bitmap_3 -- $1

# run clemente in DEBUG mode
@test_mode_5 *args='':
    @cargo run --features test_bitmap_5 -- $1

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

# check code formatting.
check-fmt:
    @cargo fmt --all --check

# it performs format of code.
fmt:
    @cargo fmt

# runs a rebase on main and `just test`.
check-after-rebase:
    #! /bin/sh
    BEFORE=$(git rev-parse HEAD)
    git config pull.ff only 
    echo "update remote..."
    git remote update || exit 1
    echo "fetch all..."
    git fetch --all || exit 1
    echo "pull main --rebase..."
    git pull origin main --rebase || exit 1
    AFTER=$(git rev-parse HEAD)
    echo "check before and after commits..."
    [[ $BEFORE != "$AFTER" ]] && echo "nothing to do, branch is already rebased" || just check-fmt test lint
