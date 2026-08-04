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
