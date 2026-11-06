# Docker setup

> Historical setup notes: these instructions predate the current client and server. Compose needs updated certificate mounts and environment variables before use. See [README.md](README.md) for the current local run commands.

Docker builds and runs the current Rust programs. It does not implement the chat.

Current behavior:

- `setup` generates an identity in memory and prints the public certificate. It does **not save the certificate or private key yet**.
- `server` prints `Server starting!` and exits. It does **not listen for connections yet**.
- `client` prints its placeholder message and exits.

## Requirements

- Docker Desktop running with its Linux container engine.
- Docker Compose v2 (included with Docker Desktop).
- Internet access for the initial image and crate downloads.

## Build and run

Open PowerShell in the project directory:

```powershell
cd C:\Users\Charles\Documents\GitHub\quic-chat
docker compose config --quiet
docker compose build server
```

Run the certificate setup program on demand:

```powershell
docker compose run --rm setup
```

Run the server:

```powershell
docker compose up server
```

With the current Rust code, the server will print its message and exit successfully. That is expected. Running setup currently prints a public certificate; it does not populate the identity volume.

Rebuild after editing Rust code with `docker compose build server`, or use `docker compose up --build server`. To run a freshly built setup program, use `docker compose run --build --rm setup`.

The client can continue running in your Windows terminal with `cargo run --bin client`. Its binary is also included in the image for later use.

## What each file does

- `Dockerfile`: compiles all three binaries with Rust 1.99.1, using the checked-in Cargo.lock, then copies them into a smaller Debian runtime image. The runtime runs as user 10001, not root.
- `compose.yaml`: defines the server and an explicitly invoked setup tool. Both use the same image. The `x-runtime` block shares container settings between them.
- `.dockerignore`: allows only the Cargo manifests and Rust source into the build. No certificates or private keys are baked into the image.

## Certificate storage

Docker creates a named volume, `quic-chat_identity`, mounted at `/var/lib/quic-chat` inside the containers. Use this instead of the previously discussed Windows AppData folder when running with Docker.

The setup container has write access to this volume. The server mounts it read-only. The directory is initialized with owner 10001 and permissions 0700, so the application account can access it. Host administrators and anyone with Docker control can still access Docker-managed data; this is persistent storage, not a secrets vault.

The next Rust task is to read `QUIC_CHAT_IDENTITY_DIR` and save/load:

```text
/var/lib/quic-chat/server-cert.pem
/var/lib/quic-chat/server-key.pem
```

The intended setup behavior is to create the files once, give the private key owner-only permissions, report failures, and refuse to overwrite an existing identity. The server will load the saved pair. The client should receive only the public certificate and explicitly verify it and the expected server name. Never give a client the identity volume or the private key.

The environment variable is provided now, but the Rust programs do not read it yet. A later certificate export step will make the public certificate available to the Windows client.

## Networking

The intended server bind address inside the container is `0.0.0.0:5000`, provided as `QUIC_CHAT_LISTEN_ADDR`. The Rust server must be taught to read and use it. Binding to all interfaces inside the container lets Docker forward traffic to it.

Compose publishes UDP port 5000 on the host's `127.0.0.1` only. QUIC uses UDP, so publishing just a TCP port would not work. Once networking is implemented, a Windows client will connect to `127.0.0.1:5000`, using `localhost` as the certificate's server name. This configuration is for testing on this computer; other computers cannot connect through this loopback-only mapping.

## Lifecycle

```powershell
docker compose logs server
docker compose down
```

`down` removes the project's containers and network but preserves the named volume. Avoid `docker compose down --volumes` unless you intend to delete the saved identity. Keep a secure backup once the volume contains a key you need to retain.

## Troubleshooting

If Docker reports that `dockerDesktopLinuxEngine` cannot be found, open Docker Desktop and wait for its Linux engine to start. Verify with `docker version`: it should show both Client and Server details. A working `docker --version` alone only confirms that the CLI is installed.

`docker compose config --quiet` validates the Compose configuration without needing the engine. Building images and running containers require the engine.

## References

- [Docker multi-stage builds](https://docs.docker.com/build/building/multi-stage/)
- [Docker volumes](https://docs.docker.com/engine/storage/volumes/)
- [Docker port publishing](https://docs.docker.com/engine/network/port-publishing/)
