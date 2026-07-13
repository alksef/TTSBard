#!/usr/bin/env bash

# Tar Pull Script: Download code from Windows using built-in tar
# Remote: ssh-sync@win.lan (path: D:\RustProjects\app-tts-v2)

REMOTE_USER="ssh-sync"
REMOTE_HOST="win.lan"
REMOTE_PATH="D:/RustProjects/app-tts-v2"

echo "=== Syncing (PULL) from Windows using TAR ==="
echo "Remote path: $REMOTE_USER@$REMOTE_HOST:$REMOTE_PATH"
echo "Local path:  ./"
echo ""

# Test SSH connection
ssh -o PreferredAuthentications=publickey -o PasswordAuthentication=no "$REMOTE_USER@$REMOTE_HOST" "echo 'SSH Connection OK'" > /dev/null 2>&1
if [ $? -ne 0 ]; then
    echo "ERROR: Connection failed."
    exit 1
fi

echo "Archiving on Windows and downloading..."
# Archive remote files (excluding build dirs) and extract them locally
# Note: Windows tar.exe uses forward slashes for exclude filters
ssh -o PreferredAuthentications=publickey -o PasswordAuthentication=no "$REMOTE_USER@$REMOTE_HOST" \
    "tar --exclude=target --exclude=src-tauri/target --exclude=node_modules --exclude=.git -czf - -C $REMOTE_PATH ." | tar -xzf -

if [ $? -eq 0 ]; then
    echo "Sync completed successfully!"
else
    echo "ERROR: Tar transfer failed."
fi
