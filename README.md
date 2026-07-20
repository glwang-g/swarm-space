# Swarm Space

**群智空间**的第一个实验：**Floating Isles Logistics Duel（漂浮群岛物流战）**。

两支 3 架无人机的自治群体，在一张公平对称的浮空岛地图上探索能量晶体、采集资源并运回基地。每架无人机都只能看到附近区域，Bot 会根据自己的共享记忆规划下一步动作。300 回合后，交付能量更多的一方获胜。

这是一个 Battlecode 风格的短局“群智对决”原型，同时预留了未来扩展为 Screeps 风格持久世界的模拟边界。

## Bot arena boundary

规则引擎与策略已分离：每架无人机都由一个 `Bot` 实例根据受限的
`Observation` 返回单步 `Intent`；引擎才是唯一能裁决移动、采集、交付和
交通冲突的一方。内置 AUTO / SCOUT / HYBRID 也通过同一接口运行，不享有
读取全局地图的特权。自定义 Rust bot 的最小接口与示例见
[docs/bot-api.md](docs/bot-api.md)。

## 写自己的 Bot

从 [`bots/README.md`](bots/README.md) 和 [`src/bots/my_bot.rs`](src/bots/my_bot.rs) 开始。游戏中按 `M` 选择蓝队使用 `MyBot`，再按 `R` 或 `Enter` 重开比赛；比赛结束后，逐回合行为记录会写入 `replays/seed-<seed>.log`。

## Run

```bash
cargo run
```

## Controls

- `Space`：暂停/继续
- `N`：暂停时单步推进一回合
- `T`：教学模式（自动暂停，逐回合显示每架无人机的决策）
- `M`：切换蓝队为 `MyBot`（修改 `src/bots/my_bot.rs` 后重开）
- `1` / `2` / `3`：1× / 4× / 16× 速度
- `R`：用当前地图重新开始
- `G`：生成新的对称地图并开始
- `F11`：窗口/全屏切换

## Current rules

- 24×16 网格、旋转对称地图
- Azure：Greedy Bot，优先最近的已知资源
- Amber：Explorer Bot，一架侦察机与两架分工运输机
- 每架无人机最多携带 3 点能量
- 可见范围为曼哈顿距离 5 格，发现的信息会在队内共享
- 无人机每回合可移动、采集、交付或等待
- 同一目的地发生冲突时，移动会被取消
- 回合上限 300，资源耗尽且没有携带货物时也会提前结束

模拟规则由 `crates/swarm-core` 暴露，不依赖 Bevy 的渲染 API；当前实现通过过渡性 include 复用 `src/simulation.rs`，方便后续继续拆成纯核心库。Bevy 只负责观战界面。这使得未来可以把模拟核心编译为 WASM，并接入 xshow 的“群智空间”页面。

核心还包含移动协商、不可达目标放弃、目标稳定性和死锁保护。workspace 测试会在 32 个不同 seed 上验证确定性、终局和无长期停摆。

## Tests

```bash
cargo test
```

测试覆盖固定 seed 的确定性、地图对称性，以及比赛能够完成并产生有效交付结果。
