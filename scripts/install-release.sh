#!/usr/bin/env bash
set -euo pipefail

release_id=${1:?release id is required}
archive=${2:?archive path is required}
unit_source=${3:?unit path is required}
root=/opt/worms-ssh
release_dir="$root/releases/$release_id"
backup_dir="$root/backups/$release_id"

if ! id worms-ssh >/dev/null 2>&1; then
    useradd --system --home-dir "$root" --shell /usr/sbin/nologin worms-ssh
fi

install -d -o worms-ssh -g worms-ssh -m 0750 "$root" "$root/releases" "$root/shared" "$root/backups"
install -d -o worms-ssh -g worms-ssh -m 0750 "$release_dir"
tar -xzf "$archive" -C "$release_dir"
chown -R worms-ssh:worms-ssh "$release_dir"
chmod 0755 "$release_dir/worms_ssh"

if [[ ! -f "$root/shared/host_key" ]]; then
    sudo -u worms-ssh ssh-keygen -q -t ed25519 -N '' -f "$root/shared/host_key"
fi

if [[ -e "$root/current" || -L "$root/current" || -f /etc/systemd/system/worms-ssh.service ]]; then
    install -d -o worms-ssh -g worms-ssh -m 0750 "$backup_dir"
    if [[ -e "$root/current" || -L "$root/current" ]]; then
        cp -a "$root/current" "$backup_dir/current"
    fi
    if [[ -f /etc/systemd/system/worms-ssh.service ]]; then
        cp -a /etc/systemd/system/worms-ssh.service "$backup_dir/worms-ssh.service"
    fi
fi

install -o root -g root -m 0644 "$unit_source" /etc/systemd/system/worms-ssh.service
ln -sfn "$release_dir" "$root/current.next"
mv -Tf "$root/current.next" "$root/current"
systemctl daemon-reload
systemctl enable worms-ssh.service >/dev/null
systemctl restart worms-ssh.service

if systemctl is-active --quiet ufw && command -v ufw >/dev/null 2>&1; then
    ufw allow 1025/tcp comment 'worms-ssh' >/dev/null
fi

systemctl --no-pager --full status worms-ssh.service
