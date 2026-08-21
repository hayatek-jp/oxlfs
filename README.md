# OxLFS

A Git LFS server written in Rust. OxLFS implements the Git LFS Batch API with
the basic transfer adapter and stores objects on the local filesystem.

[日本語版README](README_ja.md)

> [!WARNING]
> OxLFS is beta software. Configuration and behavior may change between releases.

> [!WARNING]
> Put it behind HTTPS before using it with untrusted clients.

## Requirements

- Rust 1.88 or later
- Git and Git LFS
- A filesystem directory writable by the OxLFS process

## Build

```sh
cargo build --release
```

The server binary is `target/release/oxlfs`. The `oxlfs_pw` utility is built
alongside it and generates Argon2 password hashes.

## Configuration

OxLFS requires a TOML configuration file. Pass its path with `--config`:

```sh
oxlfs --config /etc/oxlfs/config.toml
```

When `--config` is omitted, the path is `./sysroot/etc/oxlfs/config.toml` for
debug builds, `C:\ProgramData\oxlfs\config.toml` for Windows release builds,
and `/etc/oxlfs/config.toml` for other release builds.

Example:

```toml
listen = "0.0.0.0:4443"
tls = true
tls_cert = "/etc/oxlfs/server.crt"
tls_key = "/etc/oxlfs/server.key"
# git_root = "repos"
storage_dir = "/var/lib/oxlfs/repos"
log_dir = "/var/log/oxlfs"
log_level = "info"
healthcheck_endpoint = true
jwt_secret = "replace-with-a-long-random-secret"
config_dir = "/etc/oxlfs"
```

`listen`, `tls`, `storage_dir`, `log_dir`, and `jwt_secret` are required.
`tls_cert` and `tls_key` are required when TLS is enabled. `log_level` accepts
`trace`, `debug`, `info`, `warn`, `error`, or `off`; release builds default to
`info`. `healthcheck_endpoint` defaults to `true` and exposes `GET /`.

Generate a secret, for example:

```sh
openssl rand -base64 32
```

`git_root` is the URL path prefix. Use `git_root = "repos"` for a repository
URL such as `https://example.com/repos/<user>/<repo>.git`.

## User Database

The server loads `users.toml` from `config_dir`. Example:

```toml
[public]
repos = ["alice/example"]

[[users]]
name = "alice"
password_hash = "$argon2id$..."
permissions = [
	{ repo = "alice/example", read = true, write = true },
	{ repo = "alice/*", read = true, write = false },
]
```

Repositories listed in `public.repos` are readable without credentials and are
never writable anonymously. Permissions target either one repository
(`owner/repository`) or all repositories for an owner (`owner/*`). The user
name `anonymous` is reserved.

Generate an Argon2 password hash with:

```sh
cargo run --bin oxlfs_pw -- 'correct horse battery staple'
```

Copy the printed hash into `password_hash`. The Batch API uses HTTP Basic
authentication. Upload and download URLs returned by the Batch API use
short-lived Bearer JWTs.

## Use With Git LFS

```sh
git lfs install
git lfs track "*.bin"
git add .gitattributes
git commit -m "Track binary files with Git LFS"
git config lfs.url https://lfs.example.com/alice/example/info/lfs
git add model.bin
git commit -m "Add model"
git push origin main
```

Without `git_root`, the endpoint pattern is:

```text
https://HOST/<user>/<repo>/info/lfs
```

With `git_root = "repos"`:

```text
https://HOST/repos/<user>/<repo>/info/lfs
```

Configure credentials using Git's normal credential helpers. OxLFS supports
SHA-256 object IDs and the basic transfer adapter. SSH authentication,
alternative transfer adapters, and the Batch API `verify` action are not
currently supported.

## Storage and Logs

Objects are stored below `storage_dir`, grouped by repository and the first two
characters of the SHA-256 object ID. Daily `oxlfs.log` files are written below
`log_dir` and logs are also emitted to standard output.

Ensure that the service account can read `config_dir/users.toml`, write to
`storage_dir`, and write to `log_dir`. Keep `jwt_secret` and password hashes
out of source control.

## License

OxLFS is licensed under the [GNU Affero General Public License v3.0 only](LICENSE).
