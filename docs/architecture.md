# Swarm Space architecture

Swarm Space 把权威世界、Agent 调度、传输协议和表现层分成四条边界：

```text
swarm-core
  世界状态、Observation、Intent、Decision、规则裁决
      ↓
swarm-runner
  独立 Bot 实例、AgentMemory、TickBudget、回放、RenderSnapshot
      ↓
WebMatch adapter
  将 RenderSnapshot 转成稳定 JSON DTO
      ↓
Canvas client
  地图绘制、局部视角、播放控制、Agent 检查器
```

## Authority

`swarm-core::Simulation` 是唯一世界权威。Bot 不能持有 Simulation，也不能直接移动实体；它只读取受限 `Observation` 并提交一个 `Intent`。核心收齐同一回合的决定后，再验证相邻移动、墙体、货物和交通冲突。

## Scheduling

`swarm-runner` 为每架无人机构造独立 Bot 和持久 `AgentMemory`。每个 tick 使用确定性的 `TickBudget`，避免平台相关的墙钟时间影响比赛结果。runner 输出：

- `MatchEvent`：回合和终局事件；
- `WorldEvent`：移动、采集和交付等结构化事实；
- `RuleMissionEvent`：由 `WorldEvent` 投影出的 `tick / actor / action / facts / consequences / visible_to` 教学事件，且保持所属队伍的可见性；
- `RenderSnapshot`：不暴露内部容器的表现协议；
- replay：可回放的逐回合历史。

2026-09-04 已通过 `cargo test --workspace --all-targets` 与
`cargo check --workspace --target wasm32-unknown-unknown` 验证。

WASM 快照还携带本回合的 `rule_missions`，因此 Canvas/外部教学层可以消费
投影事件，而不需要访问 runner 或原生 `Simulation`。
Canvas 的检查器会按全局/队伍观察权限过滤并展示这些回执，同时保留原生世界事件列表。
适配器测试会在首回合断言至少一条回执及全部共享字段，防止该传输边界退化。
`web/app-ui.test.mjs` 还锁定检查器面板、可见性过滤和原生日志共存这一前端边界。

## Web boundary

根 crate 是一个很小的 `wasm-bindgen` 适配器。它只暴露：

- 创建或重开比赛；
- 推进一步或有限批量回合；
- 查询是否结束；
- 获取序列化快照。

Canvas 客户端不复制寻路、资源或冲突规则。它可以更换为 WebGL、远程 WebSocket 或其他 UI，而不改变核心。

## ECS decision

当前不依赖 `bevy_ecs`。代码只借鉴 ECS 的职责拆分：

- 组件数据使用普通 Rust struct；
- 系统使用输入输出明确的函数；
- 世界资源属于 Simulation；
- 决策和结算阶段显式排序；
- 实体使用稳定业务 ID。

只有当动态组件组合、插件调度或实体规模证明需要 ECS 存储时，才考虑单独引入 `bevy_ecs`，而不是完整 Bevy 引擎。

## Extension points

- 新世界规则：`swarm-core`
- 新 Bot、比赛调度、批量统计：`swarm-runner` 或独立 adapter crate
- 网络协议：从 `RenderSnapshot` / `WorldEvent` 派生版本化 DTO
- 画面与交互：`web/`
- 自定义 Bot 入门：`examples/my_bot.rs`
