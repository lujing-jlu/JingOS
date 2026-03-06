#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEFAULT_DELAY_MS=12000
DELAY_MS="${JINGOS_SERIAL_DELAY_MS:-$DEFAULT_DELAY_MS}"
MODE="${1:-${JINGOS_REGRESSION_MODE:-full}}"

if [[ -d "$ROOT_DIR/.rustup-local" ]]; then
  export RUSTUP_HOME="$ROOT_DIR/.rustup-local"
fi
if [[ -d "$ROOT_DIR/.cargo-local" ]]; then
  export CARGO_HOME="$ROOT_DIR/.cargo-local"
fi

cleanup_qemu_lock() {
  local image
  shopt -s nullglob
  for image in "$ROOT_DIR"/target/debug/build/jingos-*/out/bios.img; do
    if command -v lsof >/dev/null 2>&1; then
      lsof "$image" 2>/dev/null | awk 'NR>1 {print $2}' | xargs -r kill -9 || true
    fi
  done
  shopt -u nullglob
}

run_case() {
  local name="$1"
  local script_rel="$2"
  shift 2

  local script_path="$ROOT_DIR/$script_rel"
  if [[ ! -f "$script_path" ]]; then
    echo "[FAIL] $name: missing script $script_rel"
    exit 1
  fi

  local log_base
  local log_file
  log_base="$(mktemp "/tmp/jingos-${name}.XXXXXX")"
  log_file="${log_base}.log"
  mv "$log_base" "$log_file"

  cleanup_qemu_lock
  (
    cd "$ROOT_DIR"
    cargo run -- bios --serial-only --serial-delay-ms "$DELAY_MS" --serial-script "$script_path" >"$log_file" 2>&1
  )

  local pattern
  for pattern in "$@"; do
    if ! rg -n "$pattern" "$log_file" >/dev/null; then
      echo "[FAIL] $name: pattern not found -> $pattern"
      echo "----- tail: $log_file -----"
      tail -n 120 "$log_file"
      exit 1
    fi
  done

  echo "[PASS] $name"
}

run_fast_suite() {
  run_case \
    "monitor_demo" \
    "scripts/monitor-demo.txt" \
    "commands: help, status, ticks"

  run_case \
    "scheduler_demo" \
    "scripts/scheduler-demo.txt" \
    "taskstep: ran id=2 kind=user_demo runs=1"

  run_case \
    "scheduler_taskrun" \
    "scripts/scheduler-taskrun-demo.txt" \
    "taskrun: ran id=" \
    "taskrun: requested=3 executed=3" \
    "user_demo result: ticks="

  run_case \
    "scheduler_sleep" \
    "scripts/scheduler-sleep-demo.txt" \
    "tasksleep: id=2 sleep_ticks=200" \
    "taskrun: requested=2 executed=2" \
    "taskstep: ran id=2 kind=user_demo runs=1"

  run_case \
    "usermode_return" \
    "scripts/usermode-return-demo.txt" \
    "returned from ring3 via int 0x81; resuming monitor"

  run_case \
    "usermode_fast_success" \
    "scripts/usermode-syscall-demo.txt" \
    "fast-syscall report \\(success-path\\): ok"

  run_case \
    "usermode_fast_error" \
    "scripts/usermode-syscall-fail-demo.txt" \
    "fast-syscall report \\(error-path\\): ok"

  run_case \
    "scheduler_fast_success" \
    "scripts/scheduler-fast-syscall-demo.txt" \
    "fast_syscall_success task: entering ring3 success-path syscall demo"
}

run_full_only_suite() {
  run_case \
    "scheduler_fast_error" \
    "scripts/scheduler-fast-syscall-fail-demo.txt" \
    "fast_syscall_error task: entering ring3 error-path syscall demo"

  run_case \
    "usermode_fast_sequential" \
    "scripts/usermode-syscall-sequential-demo.txt" \
    "fast-syscall report \\(success-path\\): ok" \
    "fast-syscall report \\(error-path\\): ok"

  run_case \
    "scheduler_fast_sequential" \
    "scripts/scheduler-fast-syscall-sequential-demo.txt" \
    "fast_syscall_success task: entering ring3 success-path syscall demo" \
    "fast_syscall_error task: entering ring3 error-path syscall demo"
}

print_usage() {
  cat <<USAGE
Usage: ./scripts/regression-suite.sh [fast|full]

  fast  快速回归（默认 CI PR/push）
  full  全量回归（默认模式，包含 sequential 场景）

Environment:
  JINGOS_SERIAL_DELAY_MS   串口发送延迟，默认 12000
  JINGOS_REGRESSION_MODE   仅在未传位置参数时生效
USAGE
}

case "$MODE" in
  fast)
    echo "Running jingOS regression suite (mode: fast, serial delay: ${DELAY_MS}ms)"
    run_fast_suite
    ;;
  full)
    echo "Running jingOS regression suite (mode: full, serial delay: ${DELAY_MS}ms)"
    run_fast_suite
    run_full_only_suite
    ;;
  -h|--help|help)
    print_usage
    exit 0
    ;;
  *)
    echo "[FAIL] unknown mode: $MODE"
    print_usage
    exit 2
    ;;
esac

echo "[PASS] all regression cases (mode: $MODE)"
