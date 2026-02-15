# marpe

Local markdown preview server with live reload.

## Usage

Run from source:

```sh
cargo run -- [OPTIONS] [DIRECTORY]
```

Run the built binary:

```sh
./target/release/marpe [OPTIONS] [DIRECTORY]
```

- `DIRECTORY` defaults to the current directory.
- `--port` is the starting port; marpe will try up to 10 ports (`PORT..PORT+9`) if needed.
- `--cert` and `--key` must be provided together.

## CLI help

```text
Usage: markdown-preview [OPTIONS] [DIRECTORY]

Options:
  --tls          Enable HTTPS (uses mkcert certificates)
  --open         Open the browser automatically
  --cert <PATH>  TLS certificate file (PEM)
  --key <PATH>   TLS private key file (PEM)
  --port <PORT>  Starting port (default: 13181)
  --syntax-theme-light <THEME>  Syntax theme for light mode (default: InspiredGitHub)
  --syntax-theme-dark <THEME>   Syntax theme for dark mode (default: base16-ocean.dark)
  -h, --help     Show this help
```

## Examples

```sh
cargo run
cargo run -- --open
cargo run -- --port 8080 ./docs
cargo run -- --tls
cargo run -- --tls --cert ./localhost.pem --key ./localhost-key.pem
```
