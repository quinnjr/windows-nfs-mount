# windows-nfs-mount

A general-purpose NFSv4.1 client for Windows, written in Rust. Mounts NFS
shares as native drive letters via [WinFsp](https://winfsp.dev/). Primary
target is [AWS S3 Files](https://docs.aws.amazon.com/AmazonS3/latest/userguide/s3-mountpoint-nfs-mount.html)
(NFS access to S3 directory buckets), but it works against any NFSv4.1
compliant server.

> **Status:** Early development. The protocol stack (XDR, RPC, NFS types,
> session management) is complete and tested. The WinFsp filesystem bridge,
> TLS transport, CLI, and Windows service are not yet implemented.
> See [PROGRESS.md](PROGRESS.md) for details.

## Why

AWS S3 Files exposes S3 buckets as NFSv4.1 endpoints, but the tooling
(`amazon-efs-utils`, `efs-proxy`) is Linux-only. Windows' built-in NFS client
lacks NFSv4.1 session support, has no concept of IAM authentication, and
cannot perform the mandatory TLS upgrade that AWS requires. No existing
Windows NFS client fills this gap.

## Features

- Full NFSv4.1 protocol implementation (RFC 8881)
- Drive letter mounts via WinFsp (`N:\`, full Win32 API compatibility)
- AWS IAM credential resolution and automatic refresh
- RPC-over-TLS (RFC 9289) for AWS S3 Files and security-conscious deployments
- Attribute caching, directory caching, read-ahead, and write coalescing
- CLI for interactive use, Windows service for persistent auto-mounts
- TUI configuration interface (`ratatui`)

## Architecture

```
cli / service
    |
winfsp-bridge          (WinFsp filesystem callbacks)
    |
nfs-mount              (caching, buffering, path translation)
    |
nfs4-client            (session management, compound builder)
    |
nfs4-types             (NFSv4.1 wire types)
    |
onc-rpc                (RPC framing, TCP/TLS transport)
    |
xdr-codec + xdr-derive (XDR serialization, derive macros)
```

Each layer is a separate crate with no circular dependencies. `aws-creds` is
isolated so non-AWS users don't pull in the AWS SDK.

## Building

Requires Rust 2024 edition (1.85+).

```sh
cargo build --workspace
cargo test --workspace
```

The protocol stack crates (`xdr-codec`, `onc-rpc`, `nfs4-types`, `nfs4-client`,
`aws-creds`, `nfs-mount`) build and test on any platform. The `winfsp-bridge`,
`cli`, and `service` crates require Windows with WinFsp installed.

## Usage (Planned)

```sh
# Mount an NFS share
windows-nfs-mount mount nas.internal:/shared S

# Mount AWS S3 Files
windows-nfs-mount mount fs-01234567.s3-nfs.us-east-1.amazonaws.com:/ N -o auth=aws

# List active mounts
windows-nfs-mount list

# Interactive configuration
windows-nfs-mount config

# Install as a Windows service for persistent mounts
windows-nfs-mount service install
windows-nfs-mount service start
```

## Configuration

`%PROGRAMDATA%\windows-nfs-mount\config.toml`:

```toml
[defaults]
tls = "auto"
rsize = 1048576

[[mount]]
source = "fs-01234567.s3-nfs.us-east-1.amazonaws.com:/"
drive = "N"
auth = "aws"
aws-profile = "production"
auto = true

[[mount]]
source = "nas.internal:/shared"
drive = "S"
auth = "sys"
uid = 1000
gid = 1000
auto = true
```

## Mount Options

| Option | Default | Description |
|---|---|---|
| `tls` | `auto` | TLS mode: `auto`, `require`, `disable` |
| `auth` | `auto` | Auth: `auto`, `sys`, `aws` |
| `aws-profile` | `default` | AWS credential profile |
| `rsize` | `1048576` | Read buffer size (bytes) |
| `wsize` | `1048576` | Write buffer size (bytes) |
| `acregmin` | `1` | File attribute cache min TTL (seconds) |
| `acdirmin` | `3` | Directory attribute cache min TTL (seconds) |
| `uid` | `65534` | Default UID for AUTH_SYS |
| `gid` | `65534` | Default GID for AUTH_SYS |

Option naming mirrors Linux `mount.nfs` conventions.

## License

MIT
