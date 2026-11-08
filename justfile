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

# Build manual RPM, source archives and checksums; does not publish.
release:
    ./scripts/build-release.sh
