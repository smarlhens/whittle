# @smarlhens/whittle

npm distribution of [`whittle`](https://github.com/smarlhens/whittle) — a Rust CLI that lints and auto-normalizes Conventional Commit subjects.

Powered by [NAPI-RS](https://napi.rs). The Rust core is compiled as a Node addon and shipped as platform-specific packages (`@smarlhens/whittle-*`). `npm`/`pnpm`/`yarn` install only the package matching your `os` + `cpu` + `libc`, with no postinstall download.

## Install

```sh
npm install --save-dev @smarlhens/whittle
# or
pnpm add -D @smarlhens/whittle
# or
yarn add -D @smarlhens/whittle
```

## Use with husky

```sh
npx husky add .husky/commit-msg 'npx whittle fix "$1"'
```

## Use with lefthook (`lefthook.yml`)

```yaml
commit-msg:
  commands:
    whittle:
      run: npx whittle fix {1}
```

## Direct CLI

```sh
npx whittle check .git/COMMIT_EDITMSG
npx whittle fix   .git/COMMIT_EDITMSG
npx whittle --config whittle.toml fix .git/COMMIT_EDITMSG
```

## Supported platforms

| OS      | Arch          | libc      |
|---------|---------------|-----------|
| macOS   | x64 (Intel)   | n/a       |
| macOS   | arm64 (M1+)   | n/a       |
| Linux   | x64           | glibc     |
| Linux   | x64           | musl      |
| Linux   | arm64         | glibc     |
| Linux   | arm64         | musl      |
| Windows | x64           | n/a       |

## License

[Blue Oak Model License 1.0.0](https://github.com/smarlhens/whittle/blob/main/LICENSE.md)
