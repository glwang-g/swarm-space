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

`swarm-core` 没有 Bevy 依赖，可以单独测试。`swarm-runner` 也没有 Bevy
依赖，因此 CLI、服务器和未来 WASM 适配器可以复用它。Bevy 主程序负责
把 `WorldSnapshot` 映射为 ECS 实体，不再直接遍历比赛运行器的权威状态。

`MatchEvent` 用于描述回合推进和终局事件；它是未来网络同步、回放播放器和
网页观战端可以复用的协议边界。

## 推荐扩展位置

- 新规则：`swarm-core`
- 批量比赛、统计、回放：`swarm-runner`
- Bot 适配器：独立 Bot crate 或 runner adapter
- 画面、交互、地图操作：`swarm-space`
