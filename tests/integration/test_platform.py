#!/usr/bin/env python3
"""Cross-language IPC, real empty-host behavior, and synthetic workflow tests."""
import importlib.util
import json
import os
from pathlib import Path
import selectors
import socket
import struct
import subprocess
import tempfile
import threading
import unittest

ROOT = Path(__file__).resolve().parents[2]
BIN = ROOT / 'target/debug'
spec = importlib.util.spec_from_file_location('ipc', ROOT / 'examples/ipc_client.py')
ipc = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ipc)


def start(*args):
    p = subprocess.Popen([str(x) for x in args], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    with selectors.DefaultSelector() as sel:
        sel.register(p.stderr, selectors.EVENT_READ)
        if not sel.select(5):
            p.kill(); p.wait()
            raise RuntimeError('service startup timeout')
        line = p.stderr.readline()
    if b'listening on' not in line:
        p.kill(); p.wait()
        raise RuntimeError(line)
    return p


def stop(p):
    p.terminate()
    try:
        p.wait(timeout=3)
    except subprocess.TimeoutExpired:
        p.kill(); p.wait()
    p.stderr.close()


class Platform(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='rfv-test-')
        self.dir = Path(self.tmp.name)
        self.adapter = self.dir / 'adapter.sock'
        self.core = self.dir / 'core.sock'
        self.processes = []

    def tearDown(self):
        for p in reversed(self.processes):
            stop(p)
        self.tmp.cleanup()

    def serve(self):
        self.processes += [start(BIN / 'rfv-adapter-rtw88', 'serve', self.adapter)]
        self.processes += [start(BIN / 'rfvd', 'serve', self.core, self.adapter)]

    def cli(self, *args):
        return subprocess.run([str(BIN / 'rfv'), *map(str, args)], capture_output=True, text=True, timeout=25)

    def test_real_discovery_and_cross_language_ipc(self):
        self.serve()
        rows = ipc.query(str(self.core))
        self.assertTrue(any(r.get('kind') == 'processing' for r in rows))
        self.assertFalse(any(r.get('kind') == 'measurement' for r in rows))
        local = self.cli('discover')
        self.assertEqual(local.returncode, 0, local.stderr)
        self.assertEqual(sum(r.get('kind') == 'device' for r in rows),
                         sum(r.get('kind') == 'device' for r in json.loads(local.stdout)))
        self.assertEqual(self.core.stat().st_mode & 0o777, 0o600)

    def test_unsupported_device_is_error_not_fake_measurements(self):
        self.serve()
        p = self.cli('sample', self.core, 'rfv_missing')
        self.assertEqual(p.returncode, 1)
        self.assertIn('DEVICE_NOT_FOUND', p.stderr)
        p = self.cli('sample', self.core, '../wlan0')
        self.assertEqual(p.returncode, 1)
        self.assertIn('invalid interface', p.stderr)

    def test_handshake_and_opcode_rejection(self):
        self.serve()
        with socket.socket(socket.AF_UNIX) as s:
            s.settimeout(5); s.connect(str(self.adapter))
            s.sendall(ipc.encode(2, 1))
            self.assertEqual(ipc.receive(s)[0], 255)
        with socket.socket(socket.AF_UNIX) as s:
            s.settimeout(5); s.connect(str(self.adapter))
            s.sendall(ipc.encode(1, 1, [{'protocol': 'RFV/1.0'}])); ipc.receive(s)
            s.sendall(ipc.encode(666, 2))
            self.assertEqual(ipc.receive(s)[0], 255)

    def test_oversized_frame_and_slow_peer(self):
        self.serve()
        with socket.socket(socket.AF_UNIX) as s:
            s.settimeout(5); s.connect(str(self.adapter))
            s.sendall(ipc.HEADER.pack(b'RFV1', 1, 0, 1, 0, 1, 2**32 - 1))
            self.assertEqual(s.recv(1), b'')
        with socket.socket(socket.AF_UNIX) as s:
            s.settimeout(5); s.connect(str(self.adapter))
            s.sendall(b'R')
            self.assertEqual(s.recv(1), b'')  # absolute 3s frame deadline
        self.assertTrue(ipc.query(str(self.core)))

    def test_refuses_socket_clobber_and_public_directory(self):
        self.adapter.write_text('preserve me')
        p = subprocess.run([str(BIN / 'rfv-adapter-rtw88'), 'serve', str(self.adapter)], capture_output=True, timeout=3)
        self.assertNotEqual(p.returncode, 0)
        self.assertEqual(self.adapter.read_text(), 'preserve me')
        public = self.dir / 'public'; public.mkdir(mode=0o755)
        p = subprocess.run([str(BIN / 'rfv-adapter-rtw88'), 'serve', str(public / 'x.sock')], capture_output=True, timeout=3)
        self.assertNotEqual(p.returncode, 0)
        self.assertFalse((public / 'x.sock').exists())

    def test_adapter_restart_rescanned(self):
        self.serve()
        initial = ipc.query(str(self.core))
        stop(self.processes[0]); self.processes.pop(0)
        p = self.cli('report', self.core)
        self.assertEqual(p.returncode, 1)
        self.assertIn('ADAPTER_UNAVAILABLE', p.stderr)
        # SIGTERM leaves a stale socket; explicit operator cleanup, never server auto-unlink.
        self.adapter.unlink()
        self.processes.append(start(BIN / 'rfv-adapter-rtw88', 'serve', self.adapter))
        self.assertEqual(ipc.query(str(self.core)), initial)

    def test_install_plan_rollback_and_refusal(self):
        prefix = self.dir / 'installed'
        p = self.cli('wizard', 'plan', prefix, BIN)
        self.assertEqual(p.returncode, 0); self.assertFalse(prefix.exists())
        p = self.cli('wizard', 'install', prefix, BIN)
        self.assertNotEqual(p.returncode, 0); self.assertFalse(prefix.exists())
        p = self.cli('wizard', 'install', prefix, BIN, '--apply')
        self.assertEqual(p.returncode, 0, p.stderr)
        self.assertEqual(prefix.stat().st_mode & 0o777, 0o700)
        p = self.cli('wizard', 'install', prefix, BIN, '--apply')
        self.assertNotEqual(p.returncode, 0)
        (prefix / 'notes').write_text('not owned by installer')
        p = self.cli('wizard', 'rollback', prefix, '--apply')
        self.assertNotEqual(p.returncode, 0); self.assertTrue((prefix / 'rfvd').exists())
        (prefix / 'notes').unlink()
        saved = (prefix / 'rfvd').read_bytes()
        with (prefix / 'rfvd').open('ab') as f: f.write(b'changed')
        p = self.cli('wizard', 'rollback', prefix, '--apply')
        self.assertNotEqual(p.returncode, 0); self.assertTrue((prefix / 'rfv').exists())
        (prefix / 'rfvd').write_bytes(saved)
        p = self.cli('wizard', 'rollback', prefix, '--apply')
        self.assertEqual(p.returncode, 0, p.stderr); self.assertFalse(prefix.exists())

    def test_install_rtw88_userspace_adapter_only(self):
        prefix = self.dir / 'rtw88-adapter'
        p = self.cli('wizard', 'plan-rtw88', prefix, BIN)
        self.assertEqual(p.returncode, 0, p.stderr)
        plan = json.loads(p.stdout)[0]
        self.assertIn('only rfv-adapter-rtw88', plan['actions'])
        self.assertEqual(plan['driver_changes'], 'none')
        self.assertFalse(prefix.exists())

        p = self.cli('wizard', 'install-rtw88', prefix, BIN)
        self.assertNotEqual(p.returncode, 0)
        self.assertFalse(prefix.exists())
        p = self.cli('wizard', 'install-rtw88', prefix, BIN, '--apply')
        self.assertEqual(p.returncode, 0, p.stderr)
        result = json.loads(p.stdout)[0]
        self.assertEqual(result['driver_changes'], 'none')
        self.assertEqual(sorted(path.name for path in prefix.iterdir()),
                         ['manifest.tsv', 'rfv-adapter-rtw88'])
        self.assertEqual((prefix / 'rfv-adapter-rtw88').read_bytes(),
                         (BIN / 'rfv-adapter-rtw88').read_bytes())
        self.assertFalse((prefix / 'rfvd').exists())
        self.assertFalse((prefix / 'rfv').exists())

        (prefix / 'unexpected').write_text('not installer-owned')
        p = self.cli('wizard', 'rollback', prefix, '--apply')
        self.assertNotEqual(p.returncode, 0)
        self.assertTrue((prefix / 'rfv-adapter-rtw88').exists())
        (prefix / 'unexpected').unlink()
        p = self.cli('wizard', 'rollback', prefix, '--apply')
        self.assertEqual(p.returncode, 0, p.stderr)
        self.assertFalse(prefix.exists())

    def test_synthetic_adapter_validates_driver_independence(self):
        # Deliberately not real RF. Synthetic measurements carry quality=simulation.
        listener = socket.socket(socket.AF_UNIX); listener.bind(str(self.adapter)); listener.listen(1)
        os.chmod(self.adapter, 0o600)
        failures = []
        def fixture_adapter():
            try:
                listener.settimeout(5)
                s, _ = listener.accept()
                with s:
                    s.settimeout(5)
                    kind, _, rid, _ = ipc.receive(s)
                    s.sendall(ipc.encode(kind, rid, [{'protocol': 'RFV/1.0'}], flags=1))
                    kind, _, rid, _ = ipc.receive(s)
                    rows = [
                        {'kind': 'device', 'interface': 'sim0', 'device_id': 'synthetic:1'},
                        {'kind': 'measurement', 'interface': 'sim0', 'device_id': 'synthetic:1', 'name': 'rssi', 'value': '-52', 'unit': 'dBm', 'source': 'integration_fixture', 'quality': 'simulation', 'observed_unix_ns': '1', 'time_basis': 'synthetic'},
                        {'kind': 'capability', 'interface': 'sim0', 'device_id': 'synthetic:1', 'feature': 'rssi', 'status': 'AVAILABLE_WITH_ADAPTER', 'implementation': 'fixture', 'evidence': 'synthetic'},
                    ]
                    s.sendall(ipc.encode(kind, rid, rows, flags=1))
            except Exception as e:
                failures.append(e)
            finally:
                listener.close()
        thread = threading.Thread(target=fixture_adapter); thread.start()
        self.processes.append(start(BIN / 'rfvd', 'serve', self.core, self.adapter))
        rows = ipc.query(str(self.core), 'sim0')
        thread.join(timeout=5)
        self.assertFalse(thread.is_alive()); self.assertFalse(failures)
        self.assertEqual(next(r for r in rows if r['kind'] == 'measurement')['quality'], 'simulation')
        self.assertEqual(next(r for r in rows if r['kind'] == 'capability')['status'], 'AVAILABLE_WITH_ADAPTER')

if __name__ == '__main__':
    unittest.main(verbosity=2)
