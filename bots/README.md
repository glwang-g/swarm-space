# 编写自己的 Bot

从 `examples/my_bot.rs` 开始。每架无人机都有一个独立 Bot 实例；runner 每回合调用一次 `decide(&Observation)`。

你只能使用观察到的局部信息和队友共享记忆，不能读取隐藏地图、对手状态或直接移动其他无人机。

## 三步流程

1. 修改 `MyBot::decide`，从 `Observation` 读取位置、货物、已知晶体和墙。
2. 返回一个 `Decision`：`Move`、`Harvest`、`Deposit` 或 `Wait`。
3. 运行 `cargo run --example my_bot`，让蓝队的 MyBot 与内置基准策略完成一局比赛。

先观察比赛结果，再逐步加入路径规划、角色分工和不可达资源放弃机制。浏览器只读取比赛快照，不会给予 Bot 额外权限。

## 规则边界

引擎会验证每个动作，并处理同时移动、冲突和采集规则。非法移动不会越过墙，也不会绕过交通裁判。

完整接口说明见 [`docs/bot-api.md`](../docs/bot-api.md)。每局结束后的逐回合记录在 `replays/`。
