Run the following:
```
sudo apt update
sudo apt install qemu-system-x86
rustup override set nightly
rustup component add rust-src --toolchain nightly-x86_64-unknown-linux-gnu
rustup component add llvm-tools-preview
cargo run
```
