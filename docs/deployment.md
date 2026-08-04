# 部署到 swarm.freexlib.com

生产部署使用仓库专用的 GitHub Actions self-hosted runner。Runner 运行在目标 Ubuntu 服务器，因此构建完成后直接复制到 Nginx 目录，不再从 GitHub 云端跨境 rsync 大文件。

## Production flow

```text
push master
  → GitHub 通知 swarm-space-production runner
  → 服务器本机 checkout / cargo test / trunk build
  → rsync dist/ 到 /var/www/swarm-space
  → nginx -t / reload
  → HTTPS smoke test
```

Runner 标签为：

```text
self-hosted, linux, x64, swarm-space
```

systemd 服务：

```text
actions.runner.glwang-g-swarm-space.swarm-space-production.service
```

## Server prerequisites

- Ubuntu x86_64
- Rust stable 与 `wasm32-unknown-unknown`
- Trunk 0.21.14 位于 `/usr/local/bin/trunk`
- Nginx、rsync、Certbot
- runner 用户为 `ubuntu`
- 持久 Cargo target 位于 `/home/ubuntu/.cache/swarm-space/target`

workflow 仅由 `master` push 或手动 dispatch 触发，不在外部 pull request 上执行。`ubuntu` 目前需要能够执行发布所需的 `sudo install`、`rsync`、`chown`、`nginx -t` 和 `systemctl reload nginx`。

## Nginx and HTTPS

站点目录：

```text
/var/www/swarm-space
```

主虚拟主机由 Certbot 管理，部署只更新 `/etc/nginx/snippets/swarm-space-static.conf`，不会覆盖证书配置。证书由以下命令首次创建，并由 Certbot 自动续期：

```bash
sudo certbot --nginx -d swarm.freexlib.com --redirect
```

HTML 与未哈希的 `app.js` 使用 `no-cache`；哈希 WASM/CSS/绑定脚本使用长期 immutable 缓存。Nginx 对 WASM、CSS 和 JavaScript 启用 gzip。

## Operations

查看 Runner：

```bash
sudo systemctl status actions.runner.glwang-g-swarm-space.swarm-space-production.service
```

查看 Nginx：

```bash
sudo nginx -t
curl -I https://swarm.freexlib.com/
```

旧的 `SWARM_SPACE_DEPLOY_KEY` 不再被 workflow 使用，可以从仓库 Actions secrets 中删除。
