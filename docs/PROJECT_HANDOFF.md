# Project handoff

## Canonical location

从现在开始，Swarm Space 的代码、测试、部署和产品讨论都以本仓库为准：

```text
/Users/mac/works/github.com/swarm-space
git@github.com:glwang-g/swarm-space.git
```

`living-world` 是早期的独立原型，保留在它自己的仓库中，不再作为
Swarm Space 的工作目录，也不应把两个项目的代码或部署脚本混在一起。

## Current product shape

当前浏览器客户端是 Rust/WASM + Canvas，权威状态位于 `swarm-core`，回合与
Bot 调度位于 `swarm-runner`。浏览器只推进 runner 并读取快照；它不能绕过
引擎直接修改世界状态。

旧 Bevy Web 客户端已从主仓库移除。需要回看旧实现时，在开发机使用：

```text
/Users/mac/works/github.com/swarm-space-bevy-archive
```

## Common commands

```bash
cargo test --workspace --all-targets
cargo check --workspace --target wasm32-unknown-unknown
trunk serve
```

推送 `master` 后，GitHub self-hosted runner 会在生产机本地构建并发布，线上
地址为 `https://swarm.freexlib.com`。部署细节见
[`deployment.md`](deployment.md)。
