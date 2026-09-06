# swarm-runner

`swarm-runner` 是无头比赛层，不依赖任何渲染框架。它负责创建比赛、逐回合推进、批量运行和收集回放。

```rust
use swarm_core::Scenario;
use swarm_runner::{run_baseline, MatchRunner};

let result = run_baseline(42, Scenario::default());
println!("{:?} {:?}", result.scores, result.winner);
```

Viewer 只接收 `RunnerUpdate::Snapshot(RenderSnapshot)`；它不持有或访问
`MatchRunner` 和权威 `Simulation`。CLI、服务器和 WASM 适配器复用同一层。

`WorldEvent` 还可被投影为跨项目教学用的 `RuleMissionEvent`：
`tick / actor / action / facts / consequences / visible_to`。投影保留原生事件的
队伍可见性（`visible_to` 为该无人机所属队伍），让未来的回放、教学或代理 UI
能够解释规则，同时不把未探索信息泄露给对手。
