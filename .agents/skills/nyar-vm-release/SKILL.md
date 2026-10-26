---
name: nyar-vm-release
description: >-
  nyar-vm.rs 的 @nyar/* npm 包发布流程：版本 bump、版本同步检查、git tag、
  GitHub Actions 可信发布（OIDC）、发布失败排障。用户要求发版、bump 版本、
  打 tag、重推或修复 npm 发布时加载。
---

# nyar-vm npm 发布

真实版本只经 GitHub Actions Trusted Publisher（OIDC）发布，不在本地 `npm publish`。

## 固定合同

- 仓库：`nyar-vm/nyar-vm`
- workflow：`.github/workflows/publish-npm.yml`
- environment：`NPM_PUBLISH`
- 发布列表：`nifty.config.ts` 的 `publish.packages`
- 平台包命名：`@nyar/<包名>-<平台>`，不要加 `cli`、`gnu`、`musl` 等后缀

不要修改上述合同来绕发布失败。

## 前置条件

1. 目标提交的 Rust 与 TypeScript CI 全绿。
2. 工作区没有未提交的发布相关改动。
3. 版本保持连续：所有 `@nyar/*` 包统一 bump，不能跳号。
4. `1.0.0` 之前固定使用 `0.0.x`，不要提前升到 v1。

## 步骤

```powershell
# 1. 统一 bump（示例版本号）
node scripts/bump-nyar-workspace-versions.mjs 0.0.15

# 2. 校验同步
node scripts/check-nyar-workspace-versions.mjs

# 3. 提交并推送，等待目标提交 CI 全绿
git add <changed package.json files>
git commit
git push origin dev
```

CI 全绿后再打 tag 并推送：

```powershell
git tag v0.0.15
git push origin v0.0.15
```

推送 tag 后由 `publish-npm.yml` 完成：

1. `bind-publish-tag.mjs` 绑定 version ↔ tag ↔ commit。
2. `require-ci-success.mjs` 校验 tag 提交的 CI 全绿。
3. 各平台构建原生产物与 wasm 产物。
4. `scripts/ci/publish-npm.mjs --version=<version>` 发布。

## 发布后验证

- 确认所有 `publish.packages` 中的包都出现在 npm registry，版本一致。
- 确认 `@nyar/valkyrie` 读取的版本号来自其 `package.json`，与发布版本一致。
- 若只发布了部分包，不要直接 bump 下一版；先按下方排障处理。

## 排障

| 现象 | 处理 |
|------|------|
| 包已 staged 但未上线，重复发布会报版本已存在 | 先 `npm unpublish <pkg>@<version>` 清除 staged 状态，再重跑发布 |
| 新包 `nifty trust` 认证失败 | 先发布 `0.0.0` 占位版本，再配置 Trusted Publisher |
| `nifty trust` 仓库名不匹配 | 必须与 `nyar-vm/nyar-vm` 完全一致 |
| tag 与提交不匹配 | 重新在正确的全绿提交上打 tag 推送，不要用 `workflow_dispatch` 发布分支 HEAD |
| CI 未绿就打了 tag | 删除该 tag，修复 CI，重新在全绿提交上打 tag |
| 发布失败需要重试 | 重新在对应提交上打同一版本 tag 重推，保持版本连续 |

`workflow_dispatch` 只用于重试已存在 tag，version 必须与现有 tag 一致；不要用它发布新版本。

## 禁止

- 本地 `npm publish` 真实版本。
- 在 CI 未通过时提前打 tag。
- 用 `--no-verify` 或修改发布合同来跳过门槛。
- 为单个平台包添加额外后缀或另起命名规则。
