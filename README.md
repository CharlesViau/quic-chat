# QUIC Chat

A command-line chat project in Rust using Quinn and Tokio. Type a message and press Enter; the server prints it. Broadcasting to other clients is not implemented yet.

## Run

Requires Rust and a development certificate for `localhost`, for example from [mkcert](https://github.com/FiloSottile/mkcert). These PowerShell examples use certificates stored in `%LOCALAPPDATA%\quic-chat\certs`.

Run the server from the project folder:

```powershell
$env:QUIC_CHAT_CERT_PATH = "$env:LOCALAPPDATA\quic-chat\certs\server-cert.pem"
$env:QUIC_CHAT_KEY_PATH = "$env:LOCALAPPDATA\quic-chat\certs\server-key.pem"
$env:QUIC_CHAT_LISTEN_ADDR = "127.0.0.1:5000"
cargo run --bin server
```

Run the client in a second terminal, also in the project folder:

```powershell
$env:QUIC_CHAT_SERVER_ADDR = "127.0.0.1:5000"
$env:QUIC_CHAT_SERVER_NAME = "localhost"
$env:QUIC_CHAT_CA_CERT_PATH = "$env:LOCALAPPDATA\mkcert\rootCA.pem"
cargo run --bin client
```

Send messages from the client terminal. Press Ctrl+C to stop either program. Keep private keys out of Git.

## Tests

```powershell
cargo test --locked --lib --bins
cargo fmt --check
```

Docker files are included, but certificate mounts and environment settings still need updating before using Compose.
