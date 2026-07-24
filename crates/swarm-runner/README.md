# swarm-runner

`swarm-runner` 是无头比赛层，不依赖 Bevy。它负责创建比赛、逐回合推进、批量运行和收集回放。

```rust
use swarm_core::Scenario;
use swarm_runner::{run_baseline, MatchRunner};

let result = run_baseline(42, Scenario::default());
println!("{:?} {:?}", result.scores, result.winner);
```

Bevy viewer 只接收 `RunnerUpdate::Snapshot(RenderSnapshot)`；它不持有或访问
`MatchRunner` 和权威 `Simulation`。未来 CLI、服务器和 WASM 适配器可以复用同一层。
