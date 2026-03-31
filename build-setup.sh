#!/bin/bash
# Install system dependencies for Linux builds.
# Tauri workspace members require WebKit/GTK even though they're not distributed.
set -euo pipefail

if [[ "$(uname)" == "Linux" ]]; then
    sudo apt-get update -qq
    sudo apt-get install -y -qq \
        libgtk-3-dev \
        libwebkit2gtk-4.1-dev \
        libjavascriptcoregtk-4.1-dev \
        libsoup-3.0-dev \
        libayatana-appindicator3-dev
fi
