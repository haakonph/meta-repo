# meta-repo

A small Rust CLI for managing multiple Git repositories as one workspace.

## Config

The repository contains `meta-repo.yaml`, the JSON Schema definition for the config.

Create `meta.yaml` in the directory where the repositories should be checked out:

```yaml
version: 1
repositories:
  - name: backend
    url: https://github.com/example/backend.git
    tags: [backend, kotlin]

  - name: frontend
    url: https://github.com/example/frontend.git
    tags: [frontend, react]
```

Repository directories are created next to `meta.yaml`.

## Install

```sh
cargo install --path .
```

The executable is named `meta`.

## Usage

Clone missing repositories and update existing repositories in parallel:

```sh
meta sync
```

Only repositories matching a tag:

```sh
meta sync --tag backend
```

Run a command in all repositories in parallel:

```sh
meta exec -- git status --short
meta exec --tag frontend -- pnpm test
```

List configured repositories:

```sh
meta list
meta list --tag backend
```

Use another config file:

```sh
meta --config path/to/meta.yaml sync
```

A failure in one repository does not stop work already running in the others. The CLI exits non-zero when one or more repositories fail.
