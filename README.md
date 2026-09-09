<p align="center">
  <img src="assets/logo.png" alt="Camux logo" width="180">
</p>

<h1 align="center">Camux</h1>

<p align="center">
  A lightweight, cross-platform webcam splitter, built with Rust and Bun-powered TypeScript.
</p>

## Tech stack

| Tool                                              | Role                                |
| ------------------------------------------------- | ----------------------------------- |
| [Rust](https://www.rust-lang.org/) (edition 2024) | Native application code             |
| [Bun](https://bun.com/) + TypeScript              | Frontend tooling and application UI |

## Prerequisites

Install the following tools before getting started:

- A current stable [Rust toolchain](https://rustup.rs/)
- [Bun](https://bun.com/docs/installation)
- [Just](https://just.systems/man/en/packages.html)

## Quick start

```sh
git clone https://github.com/danfq/Camux.git
cd Camux
just run
```

Run `just` to see every available recipe.

## Development commands

| Command      | Description                                |
| ------------ | ------------------------------------------ |
| `just run`   | Run the Rust application                   |
| `just build` | Build the Rust application                 |
| `just check` | Type-check the Rust application            |
| `just test`  | Run Rust tests                             |
| `just fmt`   | Format Rust sources                        |
| `just lint`  | Run Clippy with warnings treated as errors |
| `just clean` | Remove Rust build artifacts                |

Arguments passed to `just run` are forwarded to the Rust binary:

```sh
just run --example-argument
```

## License

Camux is licensed under the [GNU Affero General Public License v3.0](LICENSE).
