default:
    cargo build --release --locked

check:
    ./scripts/quality.sh

run:
    cargo run --locked

sample:
    cargo run --locked -- --sample

install:
    ./scripts/install.sh
