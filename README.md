
![Alt text](img/clementine_logo_200px.png?raw=true "Clementine_logo")

[![Rust](https://github.com/RIP-Comm/clementine/actions/workflows/rust.yml/badge.svg)](https://github.com/RIP-Comm/clementine/actions/workflows/rust.yml)

# Clementine - A collaborative approach to GBA emulation

Welcome to the first ripsters' project. Our goal is to understand how GameBoy Advance works and to create a modern emulator written in Rust (if you want to collaborate but you can't code in Rust take a look [here](https://doc.rust-lang.org/book/)).

Everything is work in progress. We will update this document a lot of times in this stage.


## Collaborative Guidelines

We love collaborating with others, so feel free to interact with us however you want. First of all, we strongly suggest you to enter in our Discord channel where you can find all of us ([here](https://discord.com/channels/919139369774891088/1013367016666714112)). 

[Contributing doc](./CONTRIBUTING.md)

[Resources](https://github.com/RIP-Comm/clementine/wiki/Resources)

## Build and quick start

- clone the repository :)
- we are using `just` and not `make` then if you want take the benefit of this install it `cargo install just`

```bash
# quick check all is working on you machine
just build
just test

# run a .gba file
cargo run -- ~/Desktop/my_game.gba
```
