"""Unprivileged contracts for the single administrative N17 provisioner."""
import importlib.util
import json
import pathlib
import shutil
import tempfile
import unittest
from types import SimpleNamespace
from typing import Any
from unittest.mock import patch

SCRIPT = pathlib.Path(__file__).with_name("parser-containment-ci.sh")


def load():
    source = SCRIPT.read_text().split("<<'PY'\n", 1)[1].rsplit("\nPY", 1)[0]
    with tempfile.TemporaryDirectory() as directory:
        path = pathlib.Path(directory) / "provisioner.py"
        path.write_text(source)
        spec = importlib.util.spec_from_file_location("provisioner", path)
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module


class Contracts(unittest.TestCase):
    def setUp(self):
        self.p = load()
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = pathlib.Path(self.temp.name)

    def test_phase_order_preserves_each_specific_recovery_obligation(self):
        self.assertEqual(self.p.PHASES, ('normal', 'abandon', 'recover_abandon',
                                         'preparing', 'recover_preparing'))

    def test_recovery_refuses_missing_specific_receipt_after_still_running_unit(self):
        scratch = self.root / 'owned'
        scratch.mkdir()
        environment = {'MAESTRO_N17_SCRATCH': str(scratch)}
        for phase in ('recover_abandon', 'recover_preparing'):
            calls = []
            def command(command, log, check, calls=calls):
                calls.append(command)
                pathlib.Path(log).write_text('N17_ACCEPTED fixture\n'
                    'test result: ok. 1 passed; 0 failed; 0 ignored;\n')
                return SimpleNamespace(returncode=0)
            with patch.object(self.p, 'run', side_effect=command), \
                 patch.object(self.p, 'collected'), self.assertRaises(AssertionError):
                self.p.phase_run(phase, 'scoped.service', environment,
                                 self.root / 'test', self.root)
            self.assertEqual(len(calls), 1, 'a failed precondition cannot skip its fresh unit')

    def test_specific_pending_receipts_and_preparing_empty_start(self):
        scratch = self.root / 'owned'
        scratch.mkdir()
        self.assertEqual(self.p.phase_preconditions('preparing', scratch, self.root), (None, True))
        abandoned = scratch / 'n17-owned' / 'cgroup-path'
        abandoned.parent.mkdir()
        abandoned.write_text('/synthetic/worker')
        (self.root / 'abandon-pending.json').write_text(json.dumps({'receipt': str(abandoned)}))
        self.assertEqual(self.p.phase_preconditions('recover_abandon', scratch, self.root),
                         (abandoned, True))
        self.assertEqual(self.p.phase_preconditions('preparing', scratch, self.root), (None, False))
        abandoned.unlink()
        abandoned.parent.rmdir()
        preparing = scratch / '.n17-sigkill-preparing'
        preparing.mkdir()
        (preparing / 'locked').write_text('held')
        self.assertEqual(self.p.phase_preconditions('recover_preparing', scratch, self.root),
                         (preparing, True))
        (self.root / 'abandon-pending.json').write_text(json.dumps({'receipt': str(self.root)}))
        with self.assertRaises(self.p.Infrastructure):
            self.p.phase_preconditions('recover_abandon', scratch, self.root)

    def test_each_recovery_logs_and_reclaims_its_exact_pending_receipt(self):
        scratch = self.root / 'owned'
        scratch.mkdir()
        environment = {'MAESTRO_N17_SCRATCH': str(scratch)}
        for phase in ('recover_abandon', 'recover_preparing'):
            directory = scratch / ('n17-owned' if phase == 'recover_abandon'
                                   else '.n17-sigkill-preparing')
            directory.mkdir()
            receipt = directory / ('cgroup-path' if phase == 'recover_abandon' else 'locked')
            receipt.write_text('pending')
            if phase == 'recover_abandon':
                (self.root / 'abandon-pending.json').write_text(json.dumps({'receipt': str(receipt)}))
            def command(command, log, check, directory=directory):
                shutil.rmtree(directory)
                with pathlib.Path(log).open('a') as output:
                    output.write('N17_ACCEPTED fixture\n'
                                 'test result: ok. 1 passed; 0 failed; 0 ignored;\n')
                return SimpleNamespace(returncode=0)
            with patch.object(self.p, 'run', side_effect=command), patch.object(self.p, 'collected'):
                self.p.phase_run(phase, 'scoped.service', environment, self.root / 'test', self.root)
            log = (self.root / f'{phase}.log').read_text()
            self.assertIn(f'N17_PHASE_START {phase} valid=True', log)
            self.assertIn(f'N17_RECOVERY_ABSENT {phase}', log)

    def test_current_cargo_bytes_and_baseline_reuse(self):
        built = self.root / "built"
        built.write_bytes(b"mutated bootstrap")
        log = self.root / "cargo.json"
        log.write_text(json.dumps({"reason": "compiler-artifact", "profile": {"test": False},
                                   "target": {"kind": ["bin"]}, "executable": str(built)}) +
                       '\n{"reason":"build-finished","success":true}\n')
        copied = self.root / "copy"
        self.p.copy_artifact(log, False, copied)
        self.assertEqual(copied.read_bytes(), built.read_bytes())
        copied.write_bytes(b"baseline bootstrap")
        with self.assertRaises(ValueError):
            self.p.verify_copy(built, copied)
        log.write_text(log.read_text().replace('"success":true', '"success":false'))
        with self.assertRaises(ValueError):
            self.p.copy_artifact(log, False, copied)

    def test_delegation_is_nonroot_and_watchdog_is_existing_ceiling(self):
        with patch.object(self.p.os, "getuid", return_value=1001), \
             patch.object(self.p.os, "getgid", return_value=1002):
            command = self.p.unit_command("maestro-host-123-1-baseline-before-normal.service", {}, self.root / "test", "normal")
        self.assertIn("User=1001", command)
        self.assertIn("Group=1002", command)
        self.assertIn("Delegate=cpu memory pids", command)
        self.assertIn("RuntimeMaxSec=120", command)
        with patch.object(self.p.os, "getuid", return_value=0), self.assertRaises(ValueError):
            self.p.unit_command("unit", {}, self.root / "test", "normal")

    def test_each_phase_failure_runs_remaining_phases_and_cleanup(self):
        for failed in self.p.PHASES:
            calls = []
            def phase(name, calls=calls, failed=failed):
                calls.append(name)
                if name == failed:
                    raise AssertionError(name)
            def cleanup(calls=calls):
                calls.append("cleanup")
            statuses, cleanup_status = self.p.all_phases(phase, cleanup)
            self.assertEqual(calls, list(self.p.PHASES) + ["cleanup"])
            self.assertEqual(statuses[failed], "failed")
            self.assertEqual(cleanup_status, "passed")
        def refuse():
            raise ValueError("cleanup failed")
        self.assertEqual(self.p.all_phases(lambda _: None, refuse)[1], "failed")

    def test_infrastructure_failure_is_not_behavioural_credit(self):
        def timeout(_):
            raise self.p.Infrastructure("watchdog")
        with self.assertRaises(self.p.Infrastructure):
            self.p.all_phases(timeout, lambda: None)

    def request(self):
        root = self.root / 'host-executor'
        root.mkdir()
        scratch = root / 'scratch'
        scratch.mkdir()
        scenario = scratch / 'baseline-before'
        scenario.mkdir()
        reports = self.root / 'reports' / 'baseline-before'
        reports.mkdir(parents=True)
        prefix = 'maestro-host-123-1'
        scope: dict[str, Any] = {
            'schema': 1, 'prefix': prefix,
            'units': [f'{prefix}-baseline-before-{name}.service' for name in self.p.PHASES],
            'profile': '/etc/apparmor.d/maestro-n17-parser-123-1',
            'installation': '/opt/maestro/n17/123-1', 'scratch': str(scratch),
        }
        scope['resources'] = scope['units'] + [scope['installation'], scope['profile'], scope['scratch']]
        scope_path = root / 'scope.json'
        scope_path.write_text(json.dumps(scope))
        request: dict[str, Any] = {
            'schema': 1, 'scenario': 'baseline-before', 'scenario_id': 'baseline-before',
            'features': self.p.FEATURES, 'identity': {'run_id': '123', 'attempt': '1'},
            'policy_sha256': '', 'provisioner_sha256': '', 'source_sha256': {}, 'mutant': None,
            'patched_source_sha256': '', 'scratch': str(scenario), 'reports': str(reports),
            'baseline_posture': None, 'scope_manifest': str(scope_path),
            'scope_sha256': self.p.digest(scope_path),
        }
        request_path = reports / 'request.json'
        request_path.write_text(json.dumps(request))
        return request_path, reports / 'result.json', request, scope

    def test_request_shape_paths_features_and_scope_fail_closed(self):
        path, result, request, scope = self.request()
        with patch.dict(self.p.os.environ, RUNNER_TEMP=str(self.root)):
            self.p.validate(path, result)
            for field, value in [('schema', 2), ('schema', True), ('extra', 1),
                                 ('features', ['other/feature']), ('scenario_id', '../escape'),
                                 ('scope_sha256', 'wrong'), ('scratch', str(self.root))]:
                changed = dict(request, **{field: value})
                path.write_text(json.dumps(changed))
                with self.assertRaises(ValueError, msg=field):
                    self.p.validate(path, result)
            for field, value in [('prefix', 'foreign'), ('installation', '/opt/maestro'),
                                 ('units', ['foreign.service']), ('resources', ['foreign'])]:
                changed = dict(scope, **{field: value})
                scope_path = pathlib.Path(request['scope_manifest'])
                scope_path.write_text(json.dumps(changed))
                path.write_text(json.dumps(dict(request, scope_sha256=self.p.digest(scope_path))))
                with self.assertRaises(ValueError, msg=field):
                    self.p.validate(path, result)

    def test_build_failure_emits_failure_receipt_and_still_collects_every_unit(self):
        _, _, request, scope = self.request()
        calls = []
        def command(command, **kwargs):
            calls.append(command)
            return SimpleNamespace(returncode=1, stdout='')
        def cleanup(units, log):
            calls.append(list(units))
        with patch.object(self.p.subprocess, 'run', side_effect=command), \
             patch.object(self.p, 'stop_units', side_effect=cleanup):
            result = self.p.scenario(request, scope, 'bound-request')
        receipt = result['receipt']
        self.assertEqual(receipt['build'], 'failed')
        self.assertEqual(receipt['provision'], 'not-run')
        self.assertEqual(receipt['test'], 'not-run')
        self.assertEqual(receipt['cleanup'], 'passed')
        self.assertEqual(calls[-1], scope['units'])
        self.assertTrue(receipt['logs'])
        self.assertEqual(result['request_sha256'], 'bound-request')

    def test_cargo_test_rebuild_cannot_leave_a_stale_bootstrap_snapshot(self):
        _, _, request, scope = self.request()
        built_test, built_bootstrap = self.root / 'built-test', self.root / 'built-bootstrap'
        calls = []
        def cargo(command, stdout, stderr):
            calls.append(command[1])
            if command[1] == 'test':
                built_test.write_bytes(b'current test')
                built_bootstrap.write_bytes(b'bootstrap overwritten by Cargo test')
                executable, test = built_test, True
            else:
                built_bootstrap.write_bytes(b'current final Cargo bootstrap')
                executable, test = built_bootstrap, False
            stdout.write(json.dumps({'reason': 'compiler-artifact', 'profile': {'test': test},
                                     'target': {'kind': ['bin']}, 'executable': str(executable)}) +
                         '\n{"reason":"build-finished","success":true}\n')
            return SimpleNamespace(returncode=0)
        with patch.object(self.p.subprocess, 'run', side_effect=cargo), \
             patch.object(self.p, 'run', side_effect=self.p.Infrastructure('end after builds')), \
             patch.object(self.p, 'stop_units'):
            result = self.p.scenario(request, scope, 'bound-request')
        self.assertEqual(calls, ['test', 'build'])
        self.assertEqual(result['receipt']['bootstrap_sha256'], self.p.digest(built_bootstrap))
        self.p.verify_copy(built_bootstrap, pathlib.Path(result['artifacts']['bootstrap']))

    def test_after_null_is_recovery_only_and_mutant_posture_stays_strict(self):
        path, result, request, scope = self.request()
        scope_path = pathlib.Path(request['scope_manifest'])
        for scenario in ['baseline-after', 'mutant-0']:
            scope['units'] += [f'{scope["prefix"]}-{scenario}-{name}.service'
                               for name in self.p.PHASES]
            scope['resources'] = scope['units'] + [scope['installation'], scope['profile'], scope['scratch']]
            scope_path.write_text(json.dumps(scope))
            scratch = pathlib.Path(scope['scratch']) / scenario
            scratch.mkdir()
            request.update(scenario='mutant' if scenario.startswith('mutant-') else scenario,
                           scenario_id=scenario, scratch=str(scratch),
                           scope_sha256=self.p.digest(scope_path), baseline_posture=None,
                           mutant={} if scenario.startswith('mutant-') else None,
                           patched_source_sha256='a'*64 if scenario.startswith('mutant-') else '')
            with patch.dict(self.p.os.environ, RUNNER_TEMP=str(self.root)):
                path.write_text(json.dumps(request))
                if scenario == 'baseline-after':
                    self.p.validate(path, result)
                else:
                    with self.assertRaises(ValueError):
                        self.p.validate(path, result)
                request['baseline_posture'] = {'installed': True, 'apparmor': True}
                path.write_text(json.dumps(request))
                self.p.validate(path, result)
                request['baseline_posture'] = {'installed': True, 'apparmor': 'assumed'}
                path.write_text(json.dumps(request))
                with self.assertRaises(ValueError):
                    self.p.validate(path, result)

    def test_provision_failure_cannot_be_a_kill_and_cleanup_is_retained(self):
        _, _, request, scope = self.request()
        calls = []
        def copy_artifact(log, test, destination):
            destination.write_bytes(b'current test' if test else b'current bootstrap')
            return destination
        def command(command, log):
            calls.append(command)
            raise self.p.Infrastructure('fixture compiler failed')
        def cleanup(units, log):
            calls.append(list(units))
        with patch.object(self.p.subprocess, 'run', return_value=SimpleNamespace(returncode=0)), \
             patch.object(self.p, 'copy_artifact', side_effect=copy_artifact), \
             patch.object(self.p, 'run', side_effect=command), \
             patch.object(self.p, 'stop_units', side_effect=cleanup):
            result = self.p.scenario(request, scope, 'bound-request')
        receipt = result['receipt']
        self.assertEqual(receipt['build'], 'passed')
        self.assertEqual(receipt['provision'], 'failed')
        self.assertEqual(receipt['test'], 'not-run')
        self.assertEqual(receipt['cleanup'], 'passed')
        self.assertEqual(calls[-1], scope['units'])

    def test_normal_unit_zero_selection_and_watchdog_never_pass(self):
        log = self.root / 'normal.log'
        environment = {'MAESTRO_N17_SCRATCH': str(self.root)}
        for text, code, error in [
            ('test result: ok. 0 passed; 0 failed; 0 ignored; N17_ACCEPTED ', 0, self.p.Infrastructure),
            ('test result: ok. 1 passed; 0 failed; 0 ignored; N17_ACCEPTED ', 1, self.p.Infrastructure),
            ("Failed with result 'timeout'.", 1, self.p.Infrastructure),
            ('test result: FAILED. 0 passed; 1 failed; 0 ignored;', 101, AssertionError),
        ]:
            def command(command, output, check, text=text, code=code):
                self.assertIn('Delegate=cpu memory pids', command)
                log.write_text(text)
                return SimpleNamespace(returncode=code)
            with patch.object(self.p, 'run', side_effect=command), \
                 patch.object(self.p, 'collected'), self.assertRaises(error):
                self.p.phase_run('normal', 'scoped.service', environment, self.root / 'test', self.root)

    def test_paths_and_markers_refuse_foreign_roots_and_symlinks(self):
        outside = self.root / "outside"
        outside.mkdir()
        link = self.root / "link"
        link.symlink_to(outside, target_is_directory=True)
        with self.assertRaises(ValueError):
            self.p.safe_path(link / "child")
        with self.assertRaises(ValueError):
            self.p.owned_directory(outside, "digest", create=False)
        (outside / ".gate-host-scope").write_text("wrong")
        with self.assertRaises(ValueError):
            self.p.owned_directory(outside, "digest", create=False)


if __name__ == "__main__":
    unittest.main()
