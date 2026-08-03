#!/usr/bin/env bash
set -euo pipefail

staging_dir=/home/ubuntu/swarm-space/deploy-staging
site_dir=/var/www/swarm-space
site_config=/etc/nginx/sites-available/swarm-space

sudo install -d -m 0755 "$site_dir"
sudo rsync -a --delete "$staging_dir/dist/" "$site_dir/"
sudo chown -R www-data:www-data "$site_dir"

sudo install -m 0644 "$staging_dir/nginx/swarm-space-static.conf" /etc/nginx/snippets/swarm-space-static.conf
if [[ ! -f "$site_config" ]]; then
    sudo install -m 0644 "$staging_dir/nginx/swarm-space.conf" "$site_config"
fi
sudo ln -sfn "$site_config" /etc/nginx/sites-enabled/swarm-space

sudo nginx -t
sudo systemctl reload nginx
curl -fsS -H 'Host: swarm.freexlib.com' http://127.0.0.1/ -o /dev/null
