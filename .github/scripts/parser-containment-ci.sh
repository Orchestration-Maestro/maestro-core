#!/usr/bin/env bash
# Reviewed N17-only administrative provisioner; application code never invokes sudo.
set -euo pipefail
project=$PWD
reports="$RUNNER_TEMP/parser-containment"
mkdir -p "$reports"
unit="maestro-n17-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}"
uid=$(id -u)
gid=$(id -g)
profile=/etc/apparmor.d/maestro-n17-parser-bootstrap
installed=
profile_loaded=0
cleanup() {
  local status=$?
  trap - EXIT
  local cleanup_failed=0
  for suffix in qualify death recover preparing prepare-recover; do
    local name="$unit-$suffix.service"
    if [[ $(sudo systemctl show "$name" -p LoadState --value) != not-found ]]; then
      sudo systemctl stop "$name" || cleanup_failed=1
    fi
  done
  if [[ $profile_loaded == 1 ]]; then
    sudo apparmor_parser -R "$profile" || cleanup_failed=1
    sudo rm -- "$profile" || cleanup_failed=1
  fi
  if [[ -n "$installed" ]]; then sudo rm -r -- "$installed" || cleanup_failed=1; fi
  if [[ $cleanup_failed != 0 ]]; then
    echo 'N17 cleanup failed' >&2
    exit 1
  fi
  exit "$status"
}
trap cleanup EXIT
# Build only before delegation. No Cargo process is placed in a worker leaf.
rustc --edition=2024 -C target-feature=+crt-static \
  crates/maestro-acquisition/tests/fixtures/hostile_child.rs -o "$reports/hostile-static"
rustc --edition=2024 crates/maestro-acquisition/tests/fixtures/hostile_child.rs \
  -o "$reports/hostile-dynamic"
cc -shared -fPIC crates/maestro-acquisition/tests/fixtures/loader_canary.c \
  -o "$reports/loader-canary.so"
cc -static -D_GNU_SOURCE crates/maestro-acquisition/tests/fixtures/executable_memory.c \
  -o "$reports/executable-memory"
cargo build -p maestro-acquisition --bin maestro-parser-bootstrap --locked
cargo test -p maestro-acquisition --locked --features parser-containment-tests \
  n17_ --no-run --message-format=json >"$reports/build.jsonl"
python3 - "$reports/build.jsonl" "$reports/test-paths" <<'PY'
import json, pathlib, sys
paths = {}
for line in pathlib.Path(sys.argv[1]).read_text().splitlines():
    row = json.loads(line)
    if (row.get('reason') == 'compiler-artifact'
            and row.get('executable') and row['profile']['test']):
        paths[row['target']['name']] = row['executable']
pathlib.Path(sys.argv[2]).write_text(paths['it'] + '\n' + paths['maestro_acquisition'] + '\n')
PY
mapfile -t tests <"$reports/test-paths"
"${tests[1]}" n17_ --nocapture | tee "$reports/pure-tests.log"
digest=$(sha256sum target/debug/maestro-parser-bootstrap | cut -d ' ' -f 1)
[[ $digest =~ ^[0-9a-f]{64}$ ]]
installed="/opt/maestro/n17/$digest"
sudo install -d -m 0755 -o root -g root "$installed"
sudo install -m 0555 -o root -g root target/debug/maestro-parser-bootstrap "$installed/bootstrap"
[[ $(sha256sum "$installed/bootstrap" | cut -d ' ' -f 1) == "$digest" ]]
profile_required=
# Probe the verified bootstrap first; administrative AppArmor permission only if needed.
if ! "$installed/bootstrap" probe >"$reports/namespace-probe.log" 2>&1; then
  cat >"$reports/apparmor-profile" <<EOF
abi <abi/4.0>,
include <tunables/global>
profile maestro-n17-parser-bootstrap $installed/bootstrap flags=(unconfined) {
  userns,
}
EOF
  sudo install -m 0644 -o root -g root "$reports/apparmor-profile" "$profile"
  profile_loaded=1
  sudo apparmor_parser -r "$profile"
  profile_required=required
  "$installed/bootstrap" probe >>"$reports/namespace-probe.log" 2>&1
fi
export MAESTRO_N17_REQUIRED=1 MAESTRO_N17_BOOTSTRAP="$installed/bootstrap"
export MAESTRO_N17_BOOTSTRAP_MODE=installed MAESTRO_N17_PROFILE="$profile_required"
export MAESTRO_N17_STATIC="$reports/hostile-static" MAESTRO_N17_DYNAMIC="$reports/hostile-dynamic"
export MAESTRO_N17_LIBRARY="$reports/loader-canary.so" MAESTRO_N17_CANARY="$reports/host-canary"
export MAESTRO_N17_WX="$reports/executable-memory"
export MAESTRO_N17_SCRATCH="$reports/owned-scratch"
printf 'synthetic host-only canary\n' >"$MAESTRO_N17_CANARY"
mkdir -m 0700 "$MAESTRO_N17_SCRATCH"
# Each transient service's supervisor is the non-root runner UID/GID.
run_unit() {
  local suffix=$1 phase=$2
  sudo systemd-run --wait --pipe --collect --unit="$unit-$suffix" \
    -p "User=$uid" -p "Group=$gid" -p 'Delegate=cpu memory pids' \
    -p KillMode=control-group -p MemoryMax=8G -p MemorySwapMax=0 \
    -p "WorkingDirectory=$project" \
    --setenv="MAESTRO_N17_DEATH_PHASE=$phase" --setenv=RUST_TEST_THREADS=1 \
    --setenv="MAESTRO_N17_REQUIRED=$MAESTRO_N17_REQUIRED" \
    --setenv="MAESTRO_N17_BOOTSTRAP=$MAESTRO_N17_BOOTSTRAP" \
    --setenv="MAESTRO_N17_BOOTSTRAP_MODE=$MAESTRO_N17_BOOTSTRAP_MODE" \
    --setenv="MAESTRO_N17_PROFILE=$MAESTRO_N17_PROFILE" \
    --setenv="MAESTRO_N17_STATIC=$MAESTRO_N17_STATIC" \
    --setenv="MAESTRO_N17_DYNAMIC=$MAESTRO_N17_DYNAMIC" \
    --setenv="MAESTRO_N17_LIBRARY=$MAESTRO_N17_LIBRARY" \
    --setenv="MAESTRO_N17_WX=$MAESTRO_N17_WX" \
    --setenv="MAESTRO_N17_CANARY=$MAESTRO_N17_CANARY" \
    --setenv="MAESTRO_N17_SCRATCH=$MAESTRO_N17_SCRATCH" \
    "${tests[0]}" n17_ --nocapture
}
# Collection is asynchronous after SIGKILL; use the same provisioning hang ceiling.
collected() {
  local name=$1 limit=$((SECONDS + 120))
  while [[ $(sudo systemctl show "$name" -p LoadState --value) != not-found ]]; do
    if ((SECONDS >= limit)); then
      echo "unit not collected: $name" >&2
      return 1
    fi
    sleep 0.01
  done
}
{
  git rev-parse HEAD
  sha256sum Cargo.lock target/debug/maestro-parser-bootstrap \
    "$reports/hostile-static" "$reports/hostile-dynamic"
  uname -a
  systemd --version
  id
  python3 - <<'PY'
import ctypes
abi = ctypes.CDLL(None, use_errno=True).syscall(444, 0, 0, 1)
print('Landlock ABI', abi)
assert abi >= 3
PY
} >"$reports/receipt.txt"
run_unit qualify normal 2>&1 | tee "$reports/kernel-tests.log"
grep -q '^N17_ACCEPTED ' "$reports/kernel-tests.log"
if [[ $profile_loaded == 1 ]]; then
  grep -q '^N17_APPARMOR_ATTACHED maestro-n17-parser-bootstrap ' "$reports/kernel-tests.log"
fi
collected "$unit-qualify.service"
run_unit death abandon >"$reports/supervisor-death.log" 2>&1 &
launcher=$!
# Owner-approved test-provisioning hang ceiling; Pi has no equivalent handshake.
# Wait for an actual populated worker, not merely a receipt written before launch.
deadline=$((SECONDS + 120))
while :; do
  if ((SECONDS >= deadline)); then
    sudo systemctl kill --kill-who=all --signal=KILL "$unit-death.service"
    find "$MAESTRO_N17_SCRATCH" -name cgroup-path -print -exec cat {} \;
    echo 'no populated worker within 120 s' >&2
    exit 1
  fi
  if ! kill -0 "$launcher" 2>/dev/null; then
    wait "$launcher"
    echo 'death probe exited before worker launch' >&2
    exit 1
  fi
  receipt=$(find "$MAESTRO_N17_SCRATCH" -name cgroup-path -print -quit)
  if [[ -n "$receipt" ]]; then
    worker=$(cat "$receipt" 2>/dev/null) || continue
    if [[ -r "$worker/pids.current" ]] && [[ $(cat "$worker/pids.current") -ge 2 ]]; then break; fi
  fi
  sleep 0.01
done
sudo systemctl kill --kill-who=all --signal=KILL "$unit-death.service"
if wait "$launcher"; then
  echo 'SIGKILL supervisor unexpectedly succeeded' >&2
  exit 1
fi
collected "$unit-death.service"
[[ ! -d "$worker" ]]
[[ -f "$receipt" ]]
run_unit recover recover 2>&1 | tee "$reports/recovery-tests.log"
[[ $(find "$MAESTRO_N17_SCRATCH" -mindepth 1 -print -quit) == '' ]]
run_unit preparing preparing >"$reports/preparing-death.log" 2>&1 &
launcher=$!
deadline=$((SECONDS + 120))
preparing="$MAESTRO_N17_SCRATCH/.n17-sigkill-preparing"
while [[ ! -f "$preparing/locked" ]]; do
  if ((SECONDS >= deadline)); then
    sudo systemctl kill --kill-who=all --signal=KILL "$unit-preparing.service"
    find "$MAESTRO_N17_SCRATCH" -maxdepth 2 -print
    echo 'no locked preparer within 120 s' >&2
    exit 1
  fi
  if ! kill -0 "$launcher" 2>/dev/null; then
    wait "$launcher"
    echo 'preparer exited before SIGKILL' >&2
    exit 1
  fi
  sleep 0.01
done
if flock -n "$preparing" true; then
  echo 'live preparer lock was not exclusive' >&2
  exit 1
fi
sudo systemctl kill --kill-who=all --signal=KILL "$unit-preparing.service"
if wait "$launcher"; then
  echo 'SIGKILL preparer unexpectedly succeeded' >&2
  exit 1
fi
collected "$unit-preparing.service"
[[ -d "$preparing" ]]
run_unit prepare-recover recover 2>&1 | tee "$reports/preparing-recovery-tests.log"
[[ $(find "$MAESTRO_N17_SCRATCH" -mindepth 1 -print -quit) == '' ]]
rmdir "$MAESTRO_N17_SCRATCH"
echo 'N17_RECOVERY_ACCEPTED SIGKILL tree/scope/locks collected; owned scratch reclaimed' |
  tee -a "$reports/receipt.txt"
