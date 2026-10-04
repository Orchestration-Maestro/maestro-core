#!/usr/bin/env bash
# One reviewed N17 provisioner for standalone qualification and gate schema 1.
set -euo pipefail
python3 - "$@" <<'PY'
import contextlib
import fcntl
import hashlib
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import time

PHASES = ('normal', 'abandon', 'recover_abandon', 'preparing', 'recover_preparing')
TEST = 'n17_kernel::n17_real_kernel_controls_cleanup_and_supervisor_recovery'
FEATURES = ['maestro-acquisition/parser-containment-tests']
REQUEST_KEYS = {'schema', 'scenario', 'scenario_id', 'features', 'identity',
                'policy_sha256', 'provisioner_sha256', 'source_sha256', 'mutant',
                'patched_source_sha256', 'scratch', 'reports', 'baseline_posture',
                'scope_manifest', 'scope_sha256'}


class Infrastructure(RuntimeError):
    """Setup, watchdog and collection failures cannot prove a mutant kill."""


def digest(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def safe_path(path):
    path = pathlib.Path(path)
    if not path.is_absolute() or '..' in path.parts:
        raise ValueError(f'unsafe path: {path}')
    for part in [path, *path.parents]:
        if part.is_symlink():
            raise ValueError(f'symlink path: {part}')
    return path


def regular(path):
    path = safe_path(path)
    if not path.is_file():
        raise ValueError(f'not a regular file: {path}')
    return path


def run(command, log=None, check=True):
    if log is None:
        result = subprocess.run(command, check=False, text=True, capture_output=True)
    else:
        with pathlib.Path(log).open('a') as output:
            output.write('$ ' + ' '.join(map(str, command)) + '\n')
            output.flush()
            result = subprocess.run(command, check=False, text=True, stdout=output,
                                    stderr=subprocess.STDOUT)
    if check and result.returncode != 0:
        raise Infrastructure(f'command failed ({result.returncode}): {command}')
    return result


def owned_directory(path, scope_hash, create=True, root=False):
    path = safe_path(path)
    marker = path / '.gate-host-scope'
    if path.exists():
        if not path.is_dir() or regular(marker).read_text().strip() != scope_hash:
            raise ValueError(f'foreign root: {path}')
    elif create:
        if root:
            run(['sudo', 'install', '-d', '-o', 'root', '-g', 'root', '-m', '0755', str(path)])
            # Marker bytes go through a runner-owned file, never shell command text.
            with tempfile.NamedTemporaryFile(mode='w') as source:
                source.write(scope_hash + '\n')
                source.flush()
                run(['sudo', 'install', '-o', 'root', '-g', 'root', '-m', '0644',
                     source.name, str(marker)])
        else:
            path.mkdir(mode=0o700)
            marker.write_text(scope_hash + '\n')
    else:
        raise ValueError(f'root not created by this provisioner: {path}')
    owner = 0 if root else os.getuid()
    if path.stat().st_uid != owner or marker.stat().st_uid != owner:
        raise ValueError(f'wrong root owner: {path}')


def validate(request_path, result_path):
    request_path = regular(request_path)
    result_path = safe_path(result_path)
    if result_path == request_path:
        raise ValueError('request/result alias')
    if result_path.exists() and not result_path.is_file():
        raise ValueError('result is not regular')
    request = json.loads(request_path.read_text())
    if set(request) != REQUEST_KEYS or type(request['schema']) is not int or request['schema'] != 1:
        raise ValueError('unknown request schema or fields')
    if request['features'] != FEATURES:
        raise ValueError('mismatched host features')
    scenario = request['scenario_id']
    if not re.fullmatch(r'baseline-before|baseline-after|mutant-[0-9]+|cleanup', scenario):
        raise ValueError('invalid scenario')
    kind = 'mutant' if scenario.startswith('mutant-') else scenario
    if request['scenario'] != kind:
        raise ValueError('mismatched scenario')
    if kind == 'mutant':
        if (not isinstance(request['mutant'], dict)
                or not re.fullmatch('[0-9a-f]{64}', request['patched_source_sha256'])):
            raise ValueError('missing mutant identity')
    elif request['mutant'] is not None or request['patched_source_sha256'] != '':
        raise ValueError('baseline has mutant identity')
    posture = request['baseline_posture']
    if scenario in ('baseline-before', 'cleanup'):
        if posture is not None:
            raise ValueError('unexpected baseline posture')
    elif scenario == 'baseline-after' and posture is None:
        pass  # Gate-controlled recovery after baseline artifact validation failed.
    elif (not isinstance(posture, dict) or set(posture) != {'installed', 'apparmor'}
          or any(type(v) is not bool for v in posture.values())):
        raise ValueError('invalid baseline posture')
    scope_path = regular(request['scope_manifest'])
    runner_temp = safe_path(os.environ['RUNNER_TEMP'])
    if scope_path not in (runner_temp / 'host-executor/scope.json',
                          runner_temp / 'parser-containment/scope.json'):
        raise ValueError('scope manifest outside reserved runner roots')
    if digest(scope_path) != request['scope_sha256']:
        raise ValueError('mismatched scope digest')
    scope = json.loads(scope_path.read_text())
    scope_keys = {'schema', 'prefix', 'units', 'profile', 'installation', 'scratch', 'resources'}
    if (set(scope) != scope_keys or type(scope['schema']) is not int
            or scope['schema'] != 1):
        raise ValueError('unknown scope schema or fields')
    run_id, attempt = str(request['identity']['run_id']), str(request['identity']['attempt'])
    if not re.fullmatch('[0-9]+', run_id) or not re.fullmatch('[0-9]+', attempt):
        raise ValueError('non-numeric run identity')
    prefix = f'maestro-host-{run_id}-{attempt}'
    expected = {'prefix': prefix, 'installation': f'/opt/maestro/n17/{run_id}-{attempt}',
                'profile': f'/etc/apparmor.d/maestro-n17-parser-{run_id}-{attempt}',
                'scratch': str(scope_path.parent / 'scratch')}
    if any(scope[key] != value for key, value in expected.items()):
        raise ValueError('unreserved scope roots')
    units = scope['units']
    if not isinstance(units, list) or len(set(units)) != len(units) or not units:
        raise ValueError('invalid unit population')
    pattern = (re.escape(prefix) + r'-(baseline-before|baseline-after|mutant-[0-9]+)-('
               + '|'.join(PHASES) + r')\.service')
    if any(not re.fullmatch(pattern, unit) for unit in units):
        raise ValueError('out-of-scope unit')
    if scope['resources'] != units + [scope['installation'], scope['profile'], scope['scratch']]:
        raise ValueError('out-of-scope resource set')
    for key in ('scratch', 'installation', 'profile'):
        safe_path(scope[key])
    scratch, reports = safe_path(request['scratch']), safe_path(request['reports'])
    if (scratch != pathlib.Path(scope['scratch']) / scenario
            or reports != request_path.parent or result_path.parent != reports):
        raise ValueError('escaping scenario paths')
    if not reports.is_dir() or (kind != 'cleanup' and not scratch.is_dir()):
        raise ValueError('missing gate-created scenario roots')
    if kind != 'cleanup' and any(
            f'{prefix}-{scenario}-{phase}.service' not in units for phase in PHASES):
        raise ValueError('incomplete phase units')
    return request, scope, digest(request_path)


def verify_copy(built, copied):
    if digest(regular(built)) != digest(regular(copied)):
        raise ValueError('baseline/stale executable differs from current Cargo bytes')


def copy_artifact(log, test, destination):
    rows = [json.loads(line) for line in regular(log).read_text().splitlines()]
    if not rows or rows[-1] != {'reason': 'build-finished', 'success': True}:
        raise ValueError('Cargo build did not finish successfully')
    paths = {row['executable'] for row in rows if row.get('reason') == 'compiler-artifact'
             and row.get('profile', {}).get('test') is test and row.get('executable')
             and (test or 'bin' in row.get('target', {}).get('kind', []))}
    if len(paths) != 1:
        raise ValueError('Cargo must identify exactly one current executable')
    built = regular(paths.pop())
    shutil.copyfile(built, destination)
    destination.chmod(0o755)
    verify_copy(built, destination)
    return destination


def unit_command(unit, environment, test, phase):
    uid, gid = os.getuid(), os.getgid()
    if uid == 0:
        raise ValueError('delegated supervisor must be non-root')
    command = ['sudo', 'systemd-run', '--wait', '--pipe', '--collect', f'--unit={unit}']
    for prop in (f'User={uid}', f'Group={gid}', 'Delegate=cpu memory pids',
                 'KillMode=control-group', 'MemoryMax=8G', 'MemorySwapMax=0',
                 'RuntimeMaxSec=120', f'WorkingDirectory={os.getcwd()}'):
        command.extend(['-p', prop])
    phase = 'recover' if phase.startswith('recover_') else phase
    environment = dict(environment, MAESTRO_N17_DEATH_PHASE=phase, RUST_TEST_THREADS='1')
    command.extend(f'--setenv={key}={value}' for key, value in environment.items())
    return command + [str(test), TEST, '--exact', '--nocapture']


def collected(unit):
    deadline = time.monotonic() + 120
    while run(['sudo', 'systemctl', 'show', unit,
               '-p', 'LoadState', '--value']).stdout.strip() != 'not-found':
        if time.monotonic() >= deadline:
            raise Infrastructure(f'unit not collected within 120 s: {unit}')
        time.sleep(0.01)


def stop_units(units, log):
    removed, absent = [], []
    for unit in units:
        state = run(['sudo', 'systemctl', 'show', unit,
                     '-p', 'LoadState', '--value']).stdout.strip()
        if state != 'not-found':
            run(['sudo', 'systemctl', 'stop', unit], log)
            removed.append(unit)
        else:
            absent.append(unit)
        collected(unit)
        with pathlib.Path(log).open('a') as output:
            output.write(f'COLLECTED {unit}\n')
    return removed, absent


def all_phases(phase, cleanup):
    statuses = {}
    infrastructure = None
    try:
        for name in PHASES:
            try:
                phase(name)
                statuses[name] = 'passed'
            except Infrastructure as error:
                infrastructure = error
                statuses[name] = 'failed'
            except (AssertionError, ValueError):
                statuses[name] = 'failed'
    finally:
        try:
            cleanup()
            cleanup_status = 'passed'
        except (Infrastructure, ValueError):
            cleanup_status = 'failed'
    if infrastructure is not None:
        raise infrastructure
    return statuses, cleanup_status


def phase_preconditions(name, scratch, reports):
    if name == 'preparing':
        return None, not any(scratch.iterdir())
    if name == 'recover_abandon':
        record = reports / 'abandon-pending.json'
        if not record.is_file():
            return None, False
        data = json.loads(regular(record).read_text())
        if set(data) != {'receipt'}:
            raise Infrastructure('invalid abandon receipt record')
        pending = safe_path(data['receipt'])
        if (not pending.is_relative_to(scratch) or pending.name != 'cgroup-path'
                or not pending.parent.name.startswith('n17-')):
            raise Infrastructure('escaping abandon receipt')
        return pending, pending.is_file()
    if name == 'recover_preparing':
        pending = safe_path(scratch / '.n17-sigkill-preparing')
        safe_path(pending / 'locked')
        return pending, pending.is_dir() and (pending / 'locked').is_file()
    return None, True


def phase_run(name, unit, environment, test, reports):
    log = reports / f'{name}.log'
    scratch = pathlib.Path(environment['MAESTRO_N17_SCRATCH'])
    pending, valid_start = phase_preconditions(name, scratch, reports)
    if name.startswith('recover_') or name == 'preparing':
        with log.open('a') as output:
            output.write(f'N17_PHASE_START {name} valid={valid_start} pending={pending}\n')
    command = unit_command(unit, environment, test, name)
    if name not in ('abandon', 'preparing'):
        result = run(command, log, check=False)
        text = log.read_text()
        if (result.returncode in (124, 137)
                or re.search(r"(?:Result: |result ')(?:timeout|oom-kill|watchdog)", text)):
            raise Infrastructure('unit watchdog/OOM, not a behavioural failure')
        collected(unit)
        summary = re.search(
            r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;', text)
        if summary is None:
            raise Infrastructure('missing actual Rust counters')
        passed, failed, ignored = map(int, summary.groups())
        if passed + failed != 1 or ignored != 0:
            raise Infrastructure('zero/ignored/incomplete test selection')
        if result.returncode != 0 and failed == 0:
            raise Infrastructure('unit failed outside a Rust assertion')
        assert result.returncode == 0 and failed == 0, 'Rust host assertion failed'
        assert 'N17_ACCEPTED ' in text
        assert valid_start, 'phase started without its own specific pending receipt'
        if pending is not None:
            assert not pending.exists(), 'specific recovery receipt was not reclaimed'
            with log.open('a') as output:
                output.write(f'N17_RECOVERY_ABSENT {name} {pending}\n')
        assert not any(scratch.iterdir()), 'recovery left owned scratch'
        return
    with log.open('a') as output:
        output.write('$ ' + ' '.join(command) + '\n')
        output.flush()
        process = subprocess.Popen(command, stdout=output, stderr=subprocess.STDOUT)
        deadline = time.monotonic() + 120
        worker = receipt = None
        try:
            while True:
                status = process.poll()
                if status is not None:
                    if status in (124, 137, -9):
                        raise Infrastructure('death probe interrupted before intentional SIGKILL')
                    raise AssertionError('death probe exited before handshake')
                if time.monotonic() >= deadline:
                    raise Infrastructure('death handshake exceeded existing 120 s ceiling')
                if name == 'preparing':
                    receipt = scratch / '.n17-sigkill-preparing'
                    if (receipt / 'locked').is_file():
                        with open_directory(receipt) as lock:
                            try:
                                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                            except BlockingIOError:
                                break
                            raise AssertionError('preparer lock was not exclusive')
                else:
                    receipts = list(scratch.rglob('cgroup-path'))
                    if receipts:
                        receipt = receipts[0]
                        worker = pathlib.Path(receipt.read_text().strip())
                        counter = worker / 'pids.current'
                        if counter.is_file() and int(counter.read_text()) >= 2:
                            break
                time.sleep(0.01)
        finally:
            run(['sudo', 'systemctl', 'kill', '--kill-who=all', '--signal=KILL', unit],
                log, check=False)
            status = process.wait()
            collected(unit)
        assert status != 0, 'SIGKILL unexpectedly succeeded'
        assert receipt is not None and receipt.exists(), 'missing owned recovery receipt'
        assert worker is None or not worker.exists(), 'killed worker cgroup survived'
        if name == 'abandon':
            (reports / 'abandon-pending.json').write_text(json.dumps({'receipt': str(receipt)}))
        assert valid_start, 'preparing started with leftover abandon scratch'
        output.write(f'N17_PENDING_RECEIPT {name} {receipt}\n')
        output.write('N17_DEATH_ACCEPTED populated/locked supervisor killed; unit collected\n')


@contextlib.contextmanager
def open_directory(path):
    descriptor = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        yield descriptor
    finally:
        os.close(descriptor)


def teardown(scope, scope_hash, log):
    removed, absent = stop_units(scope['units'], log)
    profile = safe_path(scope['profile'])
    if profile.exists():
        if f'# gate-host-scope {scope_hash}\n' not in regular(profile).read_text():
            raise ValueError('foreign AppArmor profile')
        run(['sudo', 'apparmor_parser', '-R', str(profile)], log)
        run(['sudo', 'rm', '--', str(profile)], log)
        removed.append(str(profile))
    else:
        absent.append(str(profile))
    for path, root in ((scope['installation'], True), (scope['scratch'], False)):
        path = safe_path(path)
        if path.exists():
            owned_directory(path, scope_hash, create=False, root=root)
            run((['sudo'] if root else []) + ['rm', '-r', '--', str(path)], log)
            removed.append(str(path))
        else:
            absent.append(str(path))
    return removed, absent


def scenario(request, scope, request_hash):
    scratch, reports = pathlib.Path(request['scratch']), pathlib.Path(request['reports'])
    scope_hash = request['scope_sha256']
    scenario_id = request['scenario_id']
    units = [f'{scope["prefix"]}-{scenario_id}-{phase}.service' for phase in PHASES]
    receipt = dict(build='not-run', provision='not-run', test='not-run', cleanup='not-run',
                   selected_tests=[], passed=0, failed=0, ignored=0,
                   phases=dict.fromkeys(PHASES, 'not-run'),
                   test_sha256='', bootstrap_sha256='', logs={})
    result = dict(schema=1, request_sha256=request_hash,
                  posture=dict(installed=False, apparmor=False),
                  artifacts=dict(test='', bootstrap=''), installed_bootstrap=None,
                  cargo_json={kind: f'{scenario_id}/cargo-{kind}.json'
                              for kind in ('test', 'bootstrap')}, receipt=receipt)
    if request['scenario'] == 'mutant':
        receipt['test_failure'] = []
    log = reports / 'provision.log'
    stage = 'build'
    try:
        root = pathlib.Path(scope['scratch'])
        marker = root / '.gate-host-scope'
        if scenario_id == 'baseline-before' and not marker.exists():
            # The gate created this root and the first empty scenario, not reusable evidence.
            if list(root.iterdir()) != [scratch] or any(scratch.iterdir()):
                raise ValueError('foreign gate scratch root')
            marker.write_text(scope_hash + '\n')
        owned_directory(root, scope_hash, create=False)
        # Cargo test can rebuild the binary: finish both builds before any snapshot.
        for kind, command in (
                ('test', ['cargo', 'test', '-p', 'maestro-acquisition',
                          '--test', 'it', '--no-run']),
                ('bootstrap', ['cargo', 'build', '-p', 'maestro-acquisition',
                               '--bin', 'maestro-parser-bootstrap'])):
            cargo_log = reports / f'cargo-{kind}.json'
            with cargo_log.open('w') as output, log.open('a') as diagnostics:
                status = subprocess.run(command + ['--locked', '--features', FEATURES[0],
                                                   '--message-format=json'],
                                        stdout=output, stderr=diagnostics).returncode
            if status != 0:
                raise Infrastructure('Cargo build failed')
        for kind in ('test', 'bootstrap'):
            cargo_log = reports / f'cargo-{kind}.json'
            path = copy_artifact(cargo_log, kind == 'test', scratch / kind)
            result['artifacts'][kind] = str(path)
            receipt[f'{kind}_sha256'] = digest(path)
        receipt['build'] = 'passed'
        stage = 'provision'
        fixtures = pathlib.Path('crates/maestro-acquisition/tests/fixtures')
        for command in (
            ['rustc', '--edition=2024', '-C', 'target-feature=+crt-static',
             str(fixtures / 'hostile_child.rs'), '-o', str(scratch / 'hostile-static')],
            ['rustc', '--edition=2024', str(fixtures / 'hostile_child.rs'),
             '-o', str(scratch / 'hostile-dynamic')],
            ['cc', '-shared', '-fPIC', str(fixtures / 'loader_canary.c'),
             '-o', str(scratch / 'loader-canary.so')],
            ['cc', '-static', '-D_GNU_SOURCE', str(fixtures / 'executable_memory.c'),
             '-o', str(scratch / 'executable-memory')]):
            run(command, log)
        installation = pathlib.Path(scope['installation'])
        owned_directory(installation, scope_hash, root=True)
        directory = installation / receipt['bootstrap_sha256']
        safe_path(directory)
        run(['sudo', 'install', '-d', '-o', 'root', '-g', 'root', '-m', '0755',
             str(directory)], log)
        installed = directory / 'bootstrap'
        safe_path(installed)
        run(['sudo', 'install', '-o', 'root', '-g', 'root', '-m', '0555',
             result['artifacts']['bootstrap'], str(installed)], log)
        verify_copy(result['artifacts']['bootstrap'], installed)
        with log.open('a') as output:
            output.write(f'N17_CURRENT_BOOTSTRAP {digest(installed)} {installed}\n')
        result['installed_bootstrap'] = str(installed)
        result['posture']['installed'] = True
        profile = pathlib.Path(scope['profile'])
        # Recovery null never assumes posture; an existing owned profile stays mandatory.
        required = bool(profile.exists() or
                        (request['baseline_posture'] and request['baseline_posture']['apparmor']))
        probe = run([str(installed), 'probe'], log, check=False)
        if required or probe.returncode != 0:
            if (profile.exists()
                    and f'# gate-host-scope {scope_hash}\n' not in regular(profile).read_text()):
                raise ValueError('foreign AppArmor profile')
            source = scratch / 'apparmor-profile'
            label = 'maestro-n17-parser-bootstrap-' + scope['prefix'].removeprefix('maestro-host-')
            source.write_text(f'# gate-host-scope {scope_hash}\nabi <abi/4.0>,\n'
                              f'include <tunables/global>\nprofile {label} '
                              f'{installed} flags=(unconfined) {{\n  userns,\n}}\n')
            run(['sudo', 'install', '-m', '0644', '-o', 'root', '-g', 'root',
                 str(source), str(profile)], log)
            run(['sudo', 'apparmor_parser', '-r', str(profile)], log)
            result['posture']['apparmor'] = True
            # A mutant probe failure is not allowed to switch to weaker setup.
            run([str(installed), 'probe'], log, check=False)
        if request['baseline_posture'] and result['posture'] != request['baseline_posture']:
            raise ValueError('mutant changed baseline posture')
        environment = dict(
            MAESTRO_N17_REQUIRED='1', MAESTRO_N17_BOOTSTRAP=str(installed),
            MAESTRO_N17_BOOTSTRAP_MODE='installed',
            MAESTRO_N17_PROFILE='required' if result['posture']['apparmor'] else '',
            MAESTRO_N17_SCRATCH=str(scratch / 'owned-scratch'),
            MAESTRO_N17_CANARY=str(scratch / 'host-canary'))
        for key, name in (('STATIC', 'hostile-static'), ('DYNAMIC', 'hostile-dynamic'),
                          ('LIBRARY', 'loader-canary.so'), ('WX', 'executable-memory')):
            environment[f'MAESTRO_N17_{key}'] = str(scratch / name)
        (scratch / 'host-canary').write_text('synthetic host-only canary\n')
        (scratch / 'owned-scratch').mkdir(mode=0o700)
        selected = run([result['artifacts']['test'], '--list']).stdout.splitlines()
        if f'{TEST}: test' not in selected:
            raise ValueError('required real host test absent')
        receipt['selected_tests'] = [TEST]
        receipt['provision'] = 'passed'
        stage = 'test'
        statuses, cleanup = all_phases(
            lambda phase: phase_run(phase, units[PHASES.index(phase)], environment,
                                   pathlib.Path(result['artifacts']['test']), reports),
            lambda: stop_units(units, reports / 'cleanup.log'))
        receipt['phases'] = statuses
        receipt['cleanup'] = cleanup
        normal = (reports / 'normal.log').read_text()
        summary = re.search(
            r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;', normal)
        if summary is None:
            raise Infrastructure('missing actual Rust test counters')
        receipt['passed'], receipt['failed'], receipt['ignored'] = map(int, summary.groups())
        failures = [phase for phase in PHASES if statuses[phase] == 'failed']
        if receipt['failed']:
            failures.append(TEST)
        receipt['test'] = 'failed' if failures else 'passed'
        if request['scenario'] == 'mutant':
            receipt['test_failure'] = failures
    except (Infrastructure, ValueError, OSError) as error:
        receipt[stage] = 'failed'
        with log.open('a') as output:
            output.write(f'INFRASTRUCTURE: {error}\n')
    finally:
        try:
            stop_units(units, reports / 'cleanup.log')
            if receipt['cleanup'] == 'not-run':
                receipt['cleanup'] = 'passed'
        except (Infrastructure, ValueError, OSError) as error:
            receipt['cleanup'] = 'failed'
            with log.open('a') as output:
                output.write(f'CLEANUP: {error}\n')
        for path in reports.iterdir():
            if (path.suffix in ('.log', '.json')
                    and path.name not in ('request.json', 'result.json')
                    and path.is_file() and path.stat().st_size):
                receipt['logs'][f'{scenario_id}/{path.name}'] = digest(path)
    return result


def main():
    if len(sys.argv) == 4 and sys.argv[1] == '--gate-host-v1':
        request, scope, request_hash = validate(sys.argv[2], sys.argv[3])
        if request['scenario'] == 'cleanup':
            log = pathlib.Path(request['reports']) / 'cleanup.log'
            result = dict(schema=1, request_sha256=request_hash, cleanup='failed',
                          removed=[], absent=[])
            try:
                removed, absent = teardown(scope, request['scope_sha256'], log)
                result.update(cleanup='passed', removed=removed, absent=absent)
            except (Infrastructure, ValueError, OSError) as error:
                with log.open('a') as output:
                    output.write(f'CLEANUP FAILED: {error}\n')
        else:
            result = scenario(request, scope, request_hash)
        result_path = safe_path(sys.argv[3])
        if result_path.exists():
            regular(result_path).unlink()
        with result_path.open('x') as output:
            json.dump(result, output)
        if request['scenario'] == 'cleanup' and result['cleanup'] != 'passed':
            raise Infrastructure('scoped cleanup failed; failure receipt retained')
        return
    if len(sys.argv) != 1:
        raise ValueError('usage: parser-containment-ci.sh [--gate-host-v1 REQUEST RESULT]')
    # Standalone qualification creates the same scope and calls the same scenario.
    root = safe_path(pathlib.Path(os.environ['RUNNER_TEMP']) / 'parser-containment')
    root.mkdir()
    scratch = root / 'scratch'
    scratch.mkdir()
    scenario_scratch = scratch / 'baseline-before'
    scenario_scratch.mkdir()
    reports = root / 'baseline-before'
    reports.mkdir()
    run_id, attempt = os.environ['GITHUB_RUN_ID'], os.environ['GITHUB_RUN_ATTEMPT']
    prefix = f'maestro-host-{run_id}-{attempt}'
    scope = dict(schema=1, prefix=prefix,
                 units=[f'{prefix}-baseline-before-{phase}.service' for phase in PHASES],
                 profile=f'/etc/apparmor.d/maestro-n17-parser-{run_id}-{attempt}',
                 installation=f'/opt/maestro/n17/{run_id}-{attempt}', scratch=str(scratch))
    scope['resources'] = scope['units'] + [scope['installation'], scope['profile'],
                                          scope['scratch']]
    scope_path = root / 'scope.json'
    scope_path.write_text(json.dumps(scope))
    request = dict(schema=1, scenario='baseline-before', scenario_id='baseline-before',
                   features=FEATURES, identity=dict(run_id=run_id, attempt=attempt),
                   policy_sha256='', provisioner_sha256='',
                   source_sha256={}, mutant=None, patched_source_sha256='', baseline_posture=None,
                   scratch=str(scenario_scratch), reports=str(reports),
                   scope_manifest=str(scope_path), scope_sha256=digest(scope_path))
    request_path, result_path = reports / 'request.json', reports / 'result.json'
    request_path.write_text(json.dumps(request))
    request, scope, request_hash = validate(request_path, result_path)
    try:
        result = scenario(request, scope, request_hash)
        result_path.write_text(json.dumps(result))
        receipt = result['receipt']
        if any(receipt[key] != 'passed' for key in ('build', 'provision', 'test', 'cleanup')):
            raise Infrastructure('standalone qualification failed; inspect phase receipts')
        print('N17_RECOVERY_ACCEPTED all five phases; current Cargo bootstrap installed')
    finally:
        teardown(scope, request['scope_sha256'], reports / 'final-cleanup.log')


if __name__ == '__main__':
    main()
PY
