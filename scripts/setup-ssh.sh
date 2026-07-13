#!/usr/bin/env bash

# SSH Setup Script for passwordless access to Windows
# Remote host: al@win.lan

REMOTE_USER="ssh-sync"
REMOTE_HOST="win.lan"
PUBKEY_FILE="$HOME/.ssh/id_ed25519.pub"

echo "=== SSH Key Setup to Windows ($REMOTE_USER@$REMOTE_HOST) ==="

# Check if public key exists, generate if missing
if [ ! -f "$PUBKEY_FILE" ]; then
    echo "SSH key not found at $PUBKEY_FILE. Generating one..."
    ssh-keygen -t ed25519 -N "" -f "${PUBKEY_FILE%.pub}"
fi

PUBKEY_CONTENT=$(cat "$PUBKEY_FILE")
echo "Using public key: $PUBKEY_CONTENT"
echo ""
echo "Connecting to Windows. You will be prompted for your Windows password once."
echo "Setting up remote .ssh directory, appending key, and setting ACL permissions..."

# Run PowerShell commands on Windows via SSH
# We escape $HOME, quotes, and backslashes carefully
ssh -o PreferredAuthentications=password -o PubkeyAuthentication=no "$REMOTE_USER@$REMOTE_HOST" \
    "powershell -Command \"\
    New-Item -ItemType Directory -Force -Path \$HOME/.ssh; \
    Add-Content -Path \$HOME/.ssh/authorized_keys -Value '$PUBKEY_CONTENT'; \
    icacls.exe \$HOME\\.ssh\\authorized_keys /inheritance:r /grant \\\"SYSTEM:(F)\\\" /grant \\\"Administrators:(F)\\\" /grant \\\"$REMOTE_USER:(F)\\\"; \
    Write-Output 'Authorized keys set successfully.'\""

if [ $? -eq 0 ]; then
    echo ""
    echo "SSH key successfully installed on Windows!"
    echo "Testing passwordless connection..."
    ssh -o PreferredAuthentications=publickey -o PasswordAuthentication=no "$REMOTE_USER@$REMOTE_HOST" "echo 'Success: Passwordless SSH works!'"
    
    echo ""
    echo "NOTE FOR ADMINISTRATORS:"
    echo "If Windows still asks for a password, your user '$REMOTE_USER' might be an Administrator."
    echo "By default, Windows OpenSSH redirects administrators to C:\\ProgramData\\ssh\\administrators_authorized_keys."
    echo "To fix this, you can either:"
    echo "1. Run PowerShell on Windows as Administrator and comment out these lines in C:\\ProgramData\\ssh\\sshd_config:"
    echo "   # Match Group administrators"
    echo "   #       AuthorizedKeysFile __PROGRAMDATA__/ssh/administrators_authorized_keys"
    echo "   And restart the SSH service: Restart-Service sshd"
    echo "2. Or write the key directly to C:\\ProgramData\\ssh\\administrators_authorized_keys and run:"
    echo "   icacls.exe C:\\ProgramData\\ssh\\administrators_authorized_keys /inheritance:r /grant \\\"SYSTEM:(F)\\\" /grant \\\"Administrators:(F)\\\""
else
    echo "Failed to install SSH key."
fi
