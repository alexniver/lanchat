# GitHub Actions 设计

> 状态：已实现
> 不是实际 workflow 文件；落地时写入 `.github/workflows/release.yml`。

## 1. 触发

```yaml
on:
  push:
    tags: ["v*"]
  workflow_dispatch:
```

- tag 是发布入口；`workflow_dispatch` 用于重跑。
- PR/push 用独立 `ci.yml`，不要每次 push 都构建全部平台。

## 2. 权限

```yaml
permissions:
  contents: write
```

- 只有 release job 需要 `contents: write`。
- checkout 默认凭据足够；不要额外 PAT。

## 3. 矩阵示例

```yaml
strategy:
  fail-fast: false
  matrix:
    include:
      - os: ubuntu-22.04
        target: x86_64-unknown-linux-gnu
        archive: tar.gz
      - os: windows-2022
        target: x86_64-pc-windows-msvc
        archive: zip
      - os: macos-15-intel    # Intel
        target: x86_64-apple-darwin
        archive: tar.gz
      - os: macos-15          # Apple Silicon
        target: aarch64-apple-darwin
        archive: tar.gz
```

注意：GitHub runner 标签会变化，落地前确认当前可用的 macOS Intel/ARM 标签。

## 4. 步骤

1. `actions/checkout`
2. 安装 Rust target（`dtolnay/rust-toolchain` 或 `actions-rust-lang/setup-rust-toolchain`）
3. `Swatinem/rust-cache`
4. 校验 tag 与 Cargo 版本
5. `cargo test --locked`
6. `cargo build --locked --release --target <target>`
7. 打包（tar.gz/zip）+ 复制 README/CHANGELOG/LICENSE
8. `sha256sum` / `shasum -a 256`
9. `actions/upload-artifact`（job 间传递）
10. release job 下载全部 artifact，创建 draft release 并上传。

## 5. Tag/版本校验

```bash
TAG="${GITHUB_REF_NAME#v}"
VERSION=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].version')
test "$TAG" = "$VERSION"
```

- 不匹配直接失败。
- 建议用 `cargo metadata`，不要从 `Cargo.toml` grep。

## 6. Release 创建

- `softprops/action-gh-release` 或 `gh release create --draft`。
- 设置 `draft: true`。
- `files: dist/*`。
- 发布说明：从 `CHANGELOG.md` 取对应版本，或用 `generate_release_notes`。
- 发布前人工试运行。

## 7. Artifact vs Release

| | Actions Artifact | GitHub Release |
| --- | --- | --- |
| 面向 | workflow 内部/job 间 | 最终用户 |
| 保留 | 默认 90 天 | 长期 |
| 下载 | 需登录 | 公开 |
| 用途 | 传递构建结果 | 发布 |

最终用户只应使用 Release。

## 8. 失败策略

- `fail-fast: false`：一个平台失败仍能看到其他结果。
- 但 release job 必须 `needs` 全部构建成功。
- 任何测试/打包失败 → 不创建 draft（或保留失败状态，人工处理）。
