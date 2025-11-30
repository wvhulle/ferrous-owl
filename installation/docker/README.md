# Docker Installation

You can run `rustowl` using the pre-built Docker image from GitHub Container Registry (GHCR).

1. Pull the latest stable image

```sh
docker pull ghcr.io/wvhulle/rustowl:latest
```

Or pull a specific version:

```sh
docker pull ghcr.io/wvhulle/rustowl:v0.3.4
```

1. Run the image

```sh
docker run --rm -v /path/to/project:/app ghcr.io/wvhulle/rustowl:latest
```

You can also pass command-line arguments as needed:

```sh
docker run --rm /path/to/project:/app ghcr.io/wvhulle/rustowl:latest --help
```

1. (Optional) Use as a CLI

To use `rustowl` as if it were installed on your system, you can create a shell alias:

```sh
alias rustowl='docker run --rm -v $(pwd):/app ghcr.io/wvhulle/rustowl:latest'
```

Now you can run `rustowl` from your terminal like a regular command.
