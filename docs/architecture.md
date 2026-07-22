# Swarm Space architecture

当前运行时已经分成三条边界：

```text
swarm-core       世界规则、Observation/Decision、Bot Contract
      ↓
swarm-runner     无头比赛、逐回合推进、批量运行、回放
      ↓
WorldSnapshot    稳定的渲染数据协议
      ↓
swarm-space      Bevy viewer、摄像机、雾效、UI 和输入
```

比赛生命周期通过消息协议交互，而不是共享运行器对象：

```text
Bevy / CLI ── MatchCommand ──> runner thread
Bevy / CLI <─ MatchEvent ───── runner thread
Bevy / CLI <─ WorldSnapshot ── runner thread
```

`swarm-core` 没有 Bevy 依赖，可以单独测试。`swarm-runner` 也没有 Bevy
依赖，因此 CLI、服务器和未来 WASM 适配器可以复用它。Bevy 主程序只持有
`RunnerHandle`、命令和当前快照，不持有 `MatchRunner`，负责把快照映射为
ECS 实体。

`MatchEvent` 用于描述回合推进和终局事件；它是未来网络同步、回放播放器和
网页观战端可以复用的协议边界。

## 推荐扩展位置

- 新规则：`swarm-core`
- 批量比赛、统计、回放：`swarm-runner`
- Bot 适配器：独立 Bot crate 或 runner adapter
- 画面、交互、地图操作：`swarm-space`
