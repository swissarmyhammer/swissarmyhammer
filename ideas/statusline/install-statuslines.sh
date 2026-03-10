#!/bin/bash
# Claude Code Custom Statuslines Installer
# Features: Colored progress bars, brain emoji, git integration

echo "🧠 Installing Claude Code Custom Statuslines..."
echo ""

CLAUDE_DIR="$HOME/.claude"

# Check if .claude directory exists
if [ ! -d "$CLAUDE_DIR" ]; then
    echo "❌ Error: ~/.claude directory not found"
    echo "   Make sure Claude Code is installed first"
    exit 1
fi

# Create statusline-full.sh
cat > "$CLAUDE_DIR/statusline-full.sh" << 'EOF'
#!/bin/bash
input=$(cat)
current_dir=$(echo "$input" | jq -r '.workspace.current_dir // .cwd // empty')
if [ -z "$current_dir" ]; then current_dir=$(pwd); fi
dir_name=$(basename "$current_dir")

model_name=$(echo "$input" | jq -r '.model.display_name // "Unknown"')
if [[ "$model_name" =~ Claude.*Sonnet ]]; then
    short_model="Sonnet $(echo "$model_name" | grep -oE '[0-9]+\.[0-9]+')"
elif [[ "$model_name" =~ Claude.*Opus ]]; then
    short_model="Opus $(echo "$model_name" | grep -oE '[0-9]+\.[0-9]+')"
elif [[ "$model_name" =~ Claude.*Haiku ]]; then
    short_model="Haiku $(echo "$model_name" | grep -oE '[0-9]+\.[0-9]+')"
else
    short_model="$model_name"
fi

remaining_pct=$(echo "$input" | jq -r '.context_window.remaining_percentage // empty')
output="📁 ${dir_name}"

if git -C "$current_dir" rev-parse --git-dir > /dev/null 2>&1; then
    branch=$(git -C "$current_dir" --no-optional-locks branch --show-current 2>/dev/null)
    if [ -z "$branch" ]; then
        branch=$(git -C "$current_dir" --no-optional-locks rev-parse --short HEAD 2>/dev/null)
    fi
    if git -C "$current_dir" --no-optional-locks diff-index --quiet HEAD -- 2>/dev/null; then
        status="✓"
    else
        status="✗"
    fi
    output="${output} [${branch} ${status}]"
fi

output="${output} • 🧠 ${short_model}"

if [ -n "$remaining_pct" ]; then
    remaining_int=$(printf "%.0f" "$remaining_pct")
    used_pct=$((100 - remaining_int))
    
    if [ "$used_pct" -lt 50 ]; then
        bar_color="\033[32m"; pct_color="\033[36m"
    elif [ "$used_pct" -lt 80 ]; then
        bar_color="\033[33m"; pct_color="\033[36m"
    else
        bar_color="\033[31m"; pct_color="\033[35m"
    fi
    reset="\033[0m"
    
    bar_length=20
    filled=$((used_pct * bar_length / 100))
    empty=$((bar_length - filled))
    
    bar=""
    for ((i=0; i<filled; i++)); do bar="${bar}█"; done
    for ((i=0; i<empty; i++)); do bar="${bar}░"; done
    
    printf "%s • [%b%s%b] %b%d%%%b" "$output" "$bar_color" "$bar" "$reset" "$pct_color" "$used_pct" "$reset"
else
    printf "%s" "$output"
fi
EOF

# Create statusline-minimal.sh
cat > "$CLAUDE_DIR/statusline-minimal.sh" << 'EOF'
#!/bin/bash
input=$(cat)
current_dir=$(echo "$input" | jq -r '.workspace.current_dir // .cwd // empty')
if [ -z "$current_dir" ]; then current_dir=$(pwd); fi
dir_name=$(basename "$current_dir")

model_name=$(echo "$input" | jq -r '.model.display_name // "Unknown"')
if [[ "$model_name" =~ Sonnet ]]; then
    short_model="Sonnet $(echo "$model_name" | grep -oE '[0-9]+\.[0-9]+')"
elif [[ "$model_name" =~ Opus ]]; then
    short_model="Opus $(echo "$model_name" | grep -oE '[0-9]+\.[0-9]+')"
elif [[ "$model_name" =~ Haiku ]]; then
    short_model="Haiku $(echo "$model_name" | grep -oE '[0-9]+\.[0-9]+')"
else
    short_model="$model_name"
fi

branch=""
if git -C "$current_dir" rev-parse --git-dir > /dev/null 2>&1; then
    branch=$(git -C "$current_dir" --no-optional-locks branch --show-current 2>/dev/null)
fi

remaining_pct=$(echo "$input" | jq -r '.context_window.remaining_percentage // empty')
output="${dir_name}"

if [ -n "$branch" ]; then output="${output} [${branch}]"; fi
output="${output} • 🧠 ${short_model}"

if [ -n "$remaining_pct" ]; then
    remaining_int=$(printf "%.0f" "$remaining_pct")
    used_pct=$((100 - remaining_int))
    
    if [ "$used_pct" -lt 50 ]; then
        bar_color="\033[32m"; pct_color="\033[36m"
    elif [ "$used_pct" -lt 80 ]; then
        bar_color="\033[33m"; pct_color="\033[36m"
    else
        bar_color="\033[31m"; pct_color="\033[35m"
    fi
    reset="\033[0m"
    
    bar_length=10
    filled=$((used_pct * bar_length / 100))
    empty=$((bar_length - filled))
    
    bar=""
    for ((i=0; i<filled; i++)); do bar="${bar}█"; done
    for ((i=0; i<empty; i++)); do bar="${bar}░"; done
    
    printf "%s [%b%s%b] %b%d%%%b" "$output" "$bar_color" "$bar" "$reset" "$pct_color" "$used_pct" "$reset"
else
    printf "%s" "$output"
fi
EOF

# Create statusline-context.sh
cat > "$CLAUDE_DIR/statusline-context.sh" << 'EOF'
#!/bin/bash
input=$(cat)
model_name=$(echo "$input" | jq -r '.model.display_name // "Unknown"')

if [[ "$model_name" =~ Claude.*Sonnet ]]; then
    short_model="Sonnet $(echo "$model_name" | grep -oE '[0-9]+\.[0-9]+')"
elif [[ "$model_name" =~ Claude.*Opus ]]; then
    short_model="Opus $(echo "$model_name" | grep -oE '[0-9]+\.[0-9]+')"
elif [[ "$model_name" =~ Claude.*Haiku ]]; then
    short_model="Haiku $(echo "$model_name" | grep -oE '[0-9]+\.[0-9]+')"
else
    short_model="$model_name"
fi

remaining_pct=$(echo "$input" | jq -r '.context_window.remaining_percentage // empty')
total_tokens=$(echo "$input" | jq -r '.context_window.context_window_size // empty')
total_input=$(echo "$input" | jq -r '.context_window.total_input_tokens // 0')
total_output=$(echo "$input" | jq -r '.context_window.total_output_tokens // 0')
used_tokens=$((total_input + total_output))

if [ -n "$remaining_pct" ]; then
    remaining_int=$(printf "%.0f" "$remaining_pct")
    used_pct=$((100 - remaining_int))
    
    if [ "$used_pct" -lt 50 ]; then
        bar_color="\033[32m"; pct_color="\033[36m"
    elif [ "$used_pct" -lt 80 ]; then
        bar_color="\033[33m"; pct_color="\033[36m"
    else
        bar_color="\033[31m"; pct_color="\033[35m"
    fi
    reset="\033[0m"
    
    bar_length=30
    filled=$((used_pct * bar_length / 100))
    empty=$((bar_length - filled))
    
    bar=""
    for ((i=0; i<filled; i++)); do bar="${bar}█"; done
    for ((i=0; i<empty; i++)); do bar="${bar}░"; done
    
    if [ -n "$total_tokens" ] && [ "$total_tokens" -gt 0 ]; then
        used_k=$((used_tokens / 1000))
        total_k=$((total_tokens / 1000))
        printf "🧠 %s [%b%s%b] %b%d%%%b (%dK/%dK)" "$short_model" "$bar_color" "$bar" "$reset" "$pct_color" "$used_pct" "$reset" "$used_k" "$total_k"
    else
        printf "🧠 %s [%b%s%b] %b%d%%%b" "$short_model" "$bar_color" "$bar" "$reset" "$pct_color" "$used_pct" "$reset"
    fi
else
    printf "🧠 %s" "$short_model"
fi
EOF

# Create statusline-git.sh
cat > "$CLAUDE_DIR/statusline-git.sh" << 'EOF'
#!/bin/bash
input=$(cat)
current_dir=$(echo "$input" | jq -r '.workspace.current_dir // .cwd // empty')
if [ -z "$current_dir" ]; then current_dir=$(pwd); fi
dir_name=$(basename "$current_dir")

if git -C "$current_dir" rev-parse --git-dir > /dev/null 2>&1; then
    branch=$(git -C "$current_dir" --no-optional-locks branch --show-current 2>/dev/null)
    if [ -z "$branch" ]; then
        branch=$(git -C "$current_dir" --no-optional-locks rev-parse --short HEAD 2>/dev/null)
    fi
    if git -C "$current_dir" --no-optional-locks diff-index --quiet HEAD -- 2>/dev/null; then
        status="✓"
    else
        status="✗"
    fi
    printf "📁 %s [%s %s]" "$dir_name" "$branch" "$status"
else
    printf "📁 %s" "$dir_name"
fi
EOF

# Create statusline-session.sh
cat > "$CLAUDE_DIR/statusline-session.sh" << 'EOF'
#!/bin/bash
input=$(cat)
session_id=$(echo "$input" | jq -r '.session_id // "unknown"')
short_session="${session_id:0:7}"
workspace_dir=$(echo "$input" | jq -r '.workspace.current_dir // .cwd // empty')
if [ -z "$workspace_dir" ]; then workspace_dir=$(pwd); fi
printf "Session: %s • %s" "$short_session" "$workspace_dir"
EOF

# Create switch helper
cat > "$CLAUDE_DIR/switch-statusline.sh" << 'EOF'
#!/bin/bash
SETTINGS_FILE="$HOME/.claude/settings.json"

if [ ! -f "$SETTINGS_FILE" ]; then
    echo "Error: Settings file not found"
    exit 1
fi

case "${1:-}" in
    git|context|session|full|minimal)
        SCRIPT="statusline-${1}.sh"
        ;;
    *)
        echo "Usage: $0 [git|context|session|full|minimal]"
        exit 1
        ;;
esac

TEMP_FILE=$(mktemp)
jq --arg script "/bin/bash $HOME/.claude/$SCRIPT" \
   '.statusLine = {type: "command", command: $script}' \
   "$SETTINGS_FILE" > "$TEMP_FILE"
mv "$TEMP_FILE" "$SETTINGS_FILE"

echo "✓ StatusLine switched to: $1"
echo "  Restart Claude Code to see changes"
EOF

# Make all executable
chmod +x "$CLAUDE_DIR"/statusline-*.sh "$CLAUDE_DIR"/switch-statusline.sh

# Update settings.json
SETTINGS_FILE="$CLAUDE_DIR/settings.json"
if [ -f "$SETTINGS_FILE" ]; then
    TEMP_FILE=$(mktemp)
    jq '.statusLine = {type: "command", command: "/bin/bash '"$CLAUDE_DIR"'/statusline-minimal.sh"}' \
       "$SETTINGS_FILE" > "$TEMP_FILE"
    mv "$TEMP_FILE" "$SETTINGS_FILE"
    echo "✅ Updated settings.json"
else
    echo "⚠️  Settings file not found - you'll need to configure manually"
fi

echo ""
echo "✅ Installation complete!"
echo ""
echo "📋 Created statuslines:"
echo "   • statusline-full.sh    - Full view with git, model, progress"
echo "   • statusline-minimal.sh - Clean minimal view (active)"
echo "   • statusline-context.sh - Model and context focused"
echo "   • statusline-git.sh     - Git branch focused"
echo "   • statusline-session.sh - Session info"
echo ""
echo "🔄 Switch statuslines:"
echo "   ~/.claude/switch-statusline.sh [git|context|session|full|minimal]"
echo ""
echo "🎨 Features:"
echo "   • 🧠 Brain emoji next to model name"
echo "   • Colored progress bars (green/yellow/red)"
echo "   • Colored percentages (cyan/magenta)"
echo "   • Git branch status indicators"
echo ""
echo "🚀 Restart Claude Code to see your new statusline!"
