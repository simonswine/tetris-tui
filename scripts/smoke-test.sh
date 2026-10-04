#!/usr/bin/env bash
# Drive the real TUI inside a tmux pty and print the frames it renders.
# Usage: scripts/smoke-test.sh [release]
set -uo pipefail

cd "$(dirname "$0")/.."
PROFILE="${1:-debug}"
BIN="target/$PROFILE/tetris-tui"
SESSION="tetris-smoke-$$"
SEED="${TETRIS_SEED:-3}"

cargo build ${1:+--release} -q || exit 1
tmux kill-session -t "$SESSION" 2>/dev/null
tmux new-session -d -s "$SESSION" -x 70 -y 28 "env TETRIS_SEED=$SEED $BIN"
trap 'tmux kill-session -t "$SESSION" 2>/dev/null' EXIT

shot() {
  echo "--------------- $1 ---------------"
  tmux capture-pane -pt "$SESSION"
}

drops() { # drops <count> <key-per-piece...>
  local count=$1
  shift
  for i in $(seq 1 "$count"); do
    for key in "$@"; do
      tmux send-keys -t "$SESSION" "$key"
    done
    tmux send-keys -t "$SESSION" Space
    sleep 0.05
  done
}

sleep 1
shot "1. spawn: the piece is already visible on the top row"

tmux send-keys -t "$SESSION" Up Right Right; sleep 0.2
tmux send-keys -t "$SESSION" c; sleep 0.2
shot "2. rotate + move + hold"

# Scatter the pieces across the field; somewhere in here rows have to clear.
scatter() { # scatter <count> - spread pieces over the whole width
  local count=$1
  for i in $(seq 1 "$count"); do
    for _ in $(seq 1 $(((i * 3) % 4))); do
      tmux send-keys -t "$SESSION" Right
    done
    for _ in $(seq 1 $(((i * 7) % 6))); do
      tmux send-keys -t "$SESSION" Left
    done
    if ((i % 4 == 0)); then
      tmux send-keys -t "$SESSION" Up
    fi
    tmux send-keys -t "$SESSION" Space
    sleep 0.05
  done
  return 0
}
scatter 40
sleep 0.3
stats=$(tmux capture-pane -pt "$SESSION" | grep -E 'Lines *[0-9]+')
echo "--------------- 3. after 40 scattered hard drops ---------------"
tmux capture-pane -pt "$SESSION"
echo "stats line: ${stats:-not found}"
echo "note: mashing keys is not a strategy - step 8 watches the bot play properly"

tmux send-keys -t "$SESSION" r; sleep 0.3
drops 60 Space
sleep 0.3
shot "4. after 60 more drops on an empty field: expect GAME OVER"

tmux send-keys -t "$SESSION" r; sleep 0.3
shot "5. restart clears the field, best score survives"

tmux send-keys -t "$SESSION" p; sleep 0.3
shot "6. paused"

tmux send-keys -t "$SESSION" p Space q; sleep 0.5
if tmux has-session -t "$SESSION" 2>/dev/null; then
  echo "FAIL: q did not exit"
  exit 1
fi
echo "OK: q exited cleanly"

echo "--------------- 7. --demo: the built-in bot plays itself ---------------"
tmux new-session -d -s "$SESSION-demo" -x 70 -y 28 "env TETRIS_SEED=42 $BIN --demo"
sleep 20
tmux capture-pane -pt "$SESSION-demo" | grep -E "attract|Score|Lines|Level|Pieces"
tmux capture-pane -pt "$SESSION-demo" | tail -12
tmux kill-session -t "$SESSION-demo" 2>/dev/null

echo "--------------- 8. tiny terminal ---------------"
tmux new-session -d -s "$SESSION-tiny" -x 40 -y 12 "$BIN"
sleep 0.8
tmux capture-pane -pt "$SESSION-tiny"
tmux kill-session -t "$SESSION-tiny" 2>/dev/null
