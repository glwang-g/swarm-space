# Swarm Space architecture

当前运行时已经分成三条边界：

```text
swarm-core       世界规则、Observation/Decision、Bot Contract
      ↓
swarm-runner     无头比赛、逐回合推进、批量运行、回放、RenderSnapshot
      ↓
RenderSnapshot   稳定的渲染数据协议，不暴露 Simulation/Drone 内部结构
      ↓
swarm-space      Bevy viewer、摄像机、雾效、UI 和输入
```

比赛生命周期通过消息协议交互，而不是共享运行器对象：

```text
Bevy / CLI ── MatchCommand ──> runner thread
Bevy / CLI <─ MatchEvent ───── runner thread
Bevy / CLI <─ WorldSnapshot ── runner thread
```

Bot 调度也位于 runner：runner 为每架无人机构造 `Observation`、调用 Bot
并收集 `Decision`，随后把完整决策集交给 `swarm-core::resolve_tick`。核心仍
负责观察数据的构造和所有规则裁决，但不再是比赛执行路径上的 Bot 调度器。
`Simulation::step` 和 `with_bot_factory` 只作为兼容性的便捷 API 保留。

每个 Bot 还拥有 runner 管理的持久 `AgentMemory`，每个 tick 接收确定性的
`TickBudget`。Bot 只能通过 `MemoryPatch` 更新自己的记忆，不能借此修改世界；
预算耗尽的决策会被裁判降级为 `Wait`。

`swarm-core` 没有 Bevy 依赖，可以单独测试。`swarm-runner` 也没有 Bevy
依赖，因此 CLI、服务器和未来 WASM 适配器可以复用它。Bevy 主程序只持有
`RunnerHandle`、命令和当前快照，不持有 `MatchRunner`，负责把快照映射为
ECS 实体。

`MatchEvent` 用于描述回合推进和终局事件；其中的 `WorldEvent` 使用结构化
移动、采集和交付记录。它是未来网络同步、回放播放器和网页观战端可以复用
的协议边界。回放文件由 runner 写出，Bevy 不负责持久化。

Bevy ECS 只管理表现实体，不是规则引擎的权威状态。规则引擎保持独立的
`Simulation`，每回合通过 `RenderSnapshot` 同步到表现层；这样可以继续支持
无头比赛、服务器和 WASM，而不把 Bevy 引入核心。

## 推荐扩展位置

- 新规则：`swarm-core`
- 批量比赛、统计、回放：`swarm-runner`
- Bot 适配器：独立 Bot crate 或 runner adapter
- 画面、交互、地图操作：`swarm-space`
