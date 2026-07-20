# 编写自己的 Bot

从 `src/bots/my_bot.rs` 开始。每架无人机都有一个 `MyBot` 实例；引擎每回合调用一次 `decide(&Observation)`。

你只能使用观察到的局部信息和队友共享记忆，不能读取隐藏地图、对手状态或直接移动其他无人机。

## 三步流程

1. 修改 `MyBot::decide`，从 `Observation` 读取位置、货物、已知晶体和墙。
2. 返回一个 `Decision`：`Move`、`Harvest`、`Deposit` 或 `Wait`。
3. 运行 `cargo run`，在游戏中按 `M`，再按 `R` 或 `Enter`，让蓝队使用你的 Bot。

先观察 `MyBot` 的日志，再逐步加入路径规划、角色分工和不可达资源放弃机制。

## 规则边界

引擎会验证每个动作，并处理同时移动、冲突和采集规则。非法移动不会越过墙，也不会绕过交通裁判。

完整接口说明见 [`docs/bot-api.md`](../docs/bot-api.md)。每局结束后的逐回合记录在 `replays/`。
