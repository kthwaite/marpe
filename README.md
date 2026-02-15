# marpe

Local markdown preview server with live reload.

## Install

```sh
cargo install --path .
```

## Usage

```sh
marpe [OPTIONS] [DIRECTORY]
```

- `DIRECTORY` defaults to the current directory.
- `--port` is the starting port; marpe will try up to 10 ports (`PORT..PORT+9`) if needed.
- `--cert` and `--key` must be provided together.

## Options

```text
  --tls          Enable HTTPS (uses mkcert certificates)
  --open         Open the browser automatically
  --cert <PATH>  TLS certificate file (PEM)
  --key <PATH>   TLS private key file (PEM)
  --port <PORT>  Starting port (default: 13181)
  --syntax-theme-light <THEME>  Syntax theme for light mode (default: InspiredGitHub)
  --syntax-theme-dark <THEME>   Syntax theme for dark mode (default: Monokai)
  -h, --help     Show this help
```

## Examples

```sh
marpe
marpe --open
marpe --port 8080 ./docs
marpe --tls
marpe --tls --cert ./localhost.pem --key ./localhost-key.pem
```
