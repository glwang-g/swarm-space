# 部署到 swarm.freexlib.com

浏览器版本编译为 WASM，由同一台 Ubuntu 服务器上的 Nginx 直接提供静态文件，不占用新的常驻应用端口，也不影响 living-world 或 xshow。

## 首次配置

1. 为 `swarm.freexlib.com` 添加指向 `82.156.83.121` 的 DNS `A` 记录。
2. 在 swarm-space GitHub 仓库的 **Settings → Secrets and variables → Actions** 中添加 `SWARM_SPACE_DEPLOY_KEY`。它可以使用 living-world 当前部署密钥的同一份私钥。
3. 推送到 `master`，或手动运行 **Deploy swarm.freexlib.com** 工作流。

工作流会测试模拟核心、构建 WASM、同步到 `/var/www/swarm-space`，并安装独立的 Nginx 虚拟主机。远端 `ubuntu` 用户需要能够免密码执行 `sudo rsync`、`install`、`chown`、`nginx` 和 `systemctl reload nginx`。

首次 HTTP 发布成功且 DNS 生效后，在服务器执行：

```bash
sudo certbot --nginx -d swarm.freexlib.com
```

之后再次部署不会覆盖 Certbot 修改过的站点配置；仓库只会刷新独立的静态站点 snippet。
