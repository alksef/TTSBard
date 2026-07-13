#!/usr/bin/env bash

# Sync Pull Script: Download code from Windows machine to local
# Remote: al@win.lan (path: D:\RustProjects\app-tts-v2)

REMOTE_USER="ssh-sync"
REMOTE_HOST="win.lan"
REMOTE_PATH="D:/RustProjects/app-tts-v2/"

# Default to Git Bash rsync path if needed, or leave empty if rsync is in Windows PATH
# REMOTE_RSYNC_PATH="--rsync-path='C:/Program Files/Git/usr/bin/rsync.exe'"
REMOTE_RSYNC_PATH=""

# Directories to exclude from synchronization
EXCLUDE_FLAGS=(
    --exclude="target/"
    --exclude="src-tauri/target/"
    --exclude="node_modules/"
    --exclude=".git/"
    --exclude=".idea/"
    --exclude=".vscode/"
    --exclude=".claude/"
    --exclude="dist/"
    --exclude="*.log"
)

LOCAL_PATH="./"

echo "=== Syncing source code from Windows ($REMOTE_USER@$REMOTE_HOST) ==="
echo "Remote path: $REMOTE_USER@$REMOTE_HOST:$REMOTE_PATH"
echo "Local path:  $LOCAL_PATH"
echo ""

# Check connection
ssh -o ConnectTimeout=3 -o PreferredAuthentications=publickey -o PasswordAuthentication=no "$REMOTE_USER@$REMOTE_HOST" "echo 'SSH Connection OK'" > /dev/null 2>&1
if [ $? -ne 0 ]; then
    echo "ERROR: Cannot connect to $REMOTE_USER@$REMOTE_HOST without password."
    echo "Please run './scripts/setup-ssh.sh' first to set up passwordless authentication."
    exit 1
fi

# Run rsync
# We use -rltgoD (equivalent to -a without -p to avoid setting permissions that Windows NTFS doesn't support)
# -z for compression, -v for verbose output
# --delete to remove deleted files on local machine
echo "Running rsync..."
rsync -rltgoDzv --delete "${EXCLUDE_FLAGS[@]}" $REMOTE_RSYNC_PATH "$REMOTE_USER@$REMOTE_HOST:$REMOTE_PATH" "$LOCAL_PATH"

if [ $? -eq 0 ]; then
    echo ""
    echo "Pull sync completed successfully!"
else
    echo ""
    echo "ERROR: rsync failed."
    echo "Possible reasons:"
    echo "1. 'rsync' is not installed on Windows or not in the system PATH."
    echo "   Tip: If Git for Windows is installed, you can modify this script to set:"
    echo "   REMOTE_RSYNC_PATH=\"--rsync-path='C:/Program Files/Git/usr/bin/rsync.exe'\""
    echo "2. The remote folder '$REMOTE_PATH' does not exist."
fi
