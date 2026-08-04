# Swarm Space

**群智空间**的第一个实验：**Floating Isles Logistics Duel（漂浮群岛物流战）**。

两支 3 架无人机的自治群体，在公平对称的浮空岛地图上探索能量晶体、采集资源并运回基地。每架无人机只能看到附近区域，Bot 根据自己的观察与共享记忆提交下一步意图；世界引擎统一裁决移动、冲突、采集和交付。

这是一个 Battlecode 风格的短局群智对决原型，同时保留向持久世界、外部 Agent 和可编程规则扩展的边界。

## Architecture

```text
swarm-core        纯 Rust 世界规则、观察、意图和裁决
      ↓
swarm-runner      Bot 调度、回合推进、回放与 RenderSnapshot
      ↓
swarm-space       134KB Rust/WASM 适配器，只序列化展示快照
      ↓
Canvas Web UI     绘制、交互、回放控制和调试视角
```

模拟核心和 runner 不依赖渲染引擎或浏览器 API。Web 客户端不能直接修改权威世界，只能推进 runner 并读取稳定快照。详细说明见 [docs/architecture.md](docs/architecture.md)。

原 Bevy 观战器已经移出主仓库并单独归档；主产品不再下载 Bevy 渲染器和完整中文字体。

## Run the Web client

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk --locked --version 0.21.14
trunk serve
```

打开终端输出的本地地址。浏览器端使用系统字体，没有外部图片或字体依赖。

### Controls

- `Space`：暂停/继续
- `N`：单步推进
- `R`：重开当前地图
- `G`：生成新种子地图
- `1` / `2` / `3`：1× / 4× / 16×
- 点击无人机：查看角色、货物、目标和决策原因
- 全局/蓝队/橙队：切换历史观察权限

## Write a Bot

Bot 只能读取 `Observation` 并返回一个 `Intent`。引擎负责验证动作并同时结算所有无人机的决定。

```bash
cargo run --example my_bot
```

从 [`examples/my_bot.rs`](examples/my_bot.rs) 开始，接口说明见 [docs/bot-api.md](docs/bot-api.md)。

## Rules

- 默认 24×16 旋转对称地图
- Azure 使用 Autonomous 基准策略
- Amber 使用 Hybrid Scout 基准策略
- 每架无人机最多携带 3 点能量
- 曼哈顿可见范围为 5，发现的信息在队内共享
- 每回合可移动、采集、交付或等待
- 同一目的地冲突由世界引擎统一裁决
- 默认回合上限 300；资源耗尽且无人携货时提前结束

## Verify

```bash
cargo test --workspace
cargo check --workspace --target wasm32-unknown-unknown
trunk build --release
```

`dist/` 是构建产物，不提交 Git。当前 release 总体积约 176KB，其中 WASM 约 134KB。

## Deploy

推送 `master` 后，仓库专用的 GitHub self-hosted runner 会在服务器本机构建、测试并发布到 `https://swarm.freexlib.com`，不再跨境上传完整构建产物。配置见 [docs/deployment.md](docs/deployment.md)。
