#!/usr/bin/env bash

# Tar Push Script: Upload local code to Windows using built-in tar
# Remote: ssh-sync@win.lan (path: D:\RustProjects\app-tts-v2)

REMOTE_USER="ssh-sync"
REMOTE_HOST="win.lan"
REMOTE_PATH="D:/RustProjects/app-tts-v2"

echo "=== Syncing (PUSH) to Windows using TAR ==="
echo "Local path:  ./"
echo "Remote path: $REMOTE_USER@$REMOTE_HOST:$REMOTE_PATH"
echo ""

# Ensure remote directory exists
ssh -o PreferredAuthentications=publickey -o PasswordAuthentication=no "$REMOTE_USER@$REMOTE_HOST" \
    "powershell -Command \"if (-not (Test-Path '$REMOTE_PATH')) { New-Item -ItemType Directory -Force -Path '$REMOTE_PATH' }\""

if [ $? -ne 0 ]; then
    echo "ERROR: Connection failed."
    exit 1
fi

echo "Archiving and sending files..."
# Create local tarball (excluding build dirs) and extract it on the remote Windows host
tar --exclude="target" \
    --exclude="src-tauri/target" \
    --exclude="node_modules" \
    --exclude=".git" \
    --exclude=".claude" \
    --exclude="dist" \
    -czf - . | ssh -o PreferredAuthentications=publickey -o PasswordAuthentication=no "$REMOTE_USER@$REMOTE_HOST" "tar -xzf - -C $REMOTE_PATH"

if [ $? -eq 0 ]; then
    echo "Sync completed successfully!"
else
    echo "ERROR: Tar transfer failed."
fi
