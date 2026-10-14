#!/usr/bin/env python3
"""RF subsystem integration tests. Fixture values are algorithm tests, never captures."""
import http.client
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import unittest
from test_platform import BIN, ROOT, ipc, start, stop


class RFWorkspace(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='rfv-rf-test-')
        self.dir = Path(self.tmp.name)
        self.processes = []

    def tearDown(self):
        for p in reversed(self.processes):
            stop(p)
        self.tmp.cleanup()

    def cli(self, *args):
        return subprocess.run([str(BIN / 'rfv'), *map(str, args)], text=True, capture_output=True, timeout=20)

    def test_budget_cli_and_provenance(self):
        path = self.dir / 'budget.tsv'
        values = dict(tx_power=20, tx_gain=2, rx_gain=0, tx_loss=1, rx_loss=0,
                      distance=100, frequency=2400, fade_reserve=10, sensitivity=-85)
        path.write_text('\n'.join(f'{k}\t{v}\testimated\talgorithm test only' for k, v in values.items()))
        result = self.cli('budget', path)
        self.assertEqual(result.returncode, 0, result.stderr)
        rows = json.loads(result.stdout)
        prediction = next(r for r in rows if r['name'] == 'predicted_received_power')
        self.assertEqual(prediction['kind'], 'prediction')
        self.assertAlmostEqual(float(prediction['value']), -59.052, places=2)
        path.write_text('tx_power\t20\tunknown\tno provenance')
        self.assertNotEqual(self.cli('budget', path).returncode, 0)

    def test_csi_import_is_explicitly_synthetic(self):
        path = self.dir / 'synthetic-algorithm-only.tsv'
        path.write_text('RFVCSI\t1\ndevice_id\talgorithm_fixture\ndriver\tnone\n'
                        'source\talgorithm test only\nenvironment\tsimulation\n'
                        'center_frequency_hz\t5180000000\nbandwidth_hz\t20000000\n'
                        'rx_chains\t1\ntx_streams\t1\nscale\t1\n'
                        'frame\t1\niq\t-1\t0\t0\t3\t4\niq\t1\t0\t0\t0\t0\n')
        result = self.cli('csi', 'analyze', path)
        self.assertEqual(result.returncode, 0, result.stderr)
        rows = json.loads(result.stdout)
        self.assertEqual(rows[0]['environment'], 'simulation')
        self.assertEqual(rows[1]['amplitude'], '5')
        self.assertNotIn('phase_rad', rows[2])
        self.assertNotIn('observed_frame_rate_hz', rows[0])
        path.write_text(path.read_text().replace('frame\t1', 'frame\t0'))
        self.assertNotEqual(self.cli('csi', 'analyze', path).returncode, 0)

    def test_recording_replay_preserves_simulation_and_rejects_coverage(self):
        path = self.dir / 'synthetic.rfv'
        rows = [dict(kind='device', device_id='algorithm_fixture', interface='test0'),
                dict(kind='measurement', device_id='algorithm_fixture', interface='test0',
                     name='rssi', value='-60', unit='dBm', source='algorithm test only',
                     quality='simulation', time_basis='synthetic', observed_unix_ns='1')]
        data = b'RFVREC1\n' + ipc.encode(3, 1, rows, flags=1)
        path.write_bytes(data)
        replay = self.cli('replay', path)
        self.assertEqual(replay.returncode, 0, replay.stderr)
        self.assertEqual(json.loads(replay.stdout)[1]['quality'], 'simulation')
        self.assertNotEqual(self.cli('coverage', path).returncode, 0)
        path.write_bytes(data[:-1])
        self.assertNotEqual(self.cli('replay', path).returncode, 0)

    def test_range_registry_is_not_automatic_compatibility(self):
        path = self.dir / 'nodes.tsv'
        path.write_text('AP1\tmesh\tunknown\tunknown\talgorithm test registration\n'
                        'AP2\tethernet_ap\tethernet\tconfirmed\tuser supplied documentation\n')
        result = self.cli('range', path)
        self.assertEqual(result.returncode, 0, result.stderr)
        rows = json.loads(result.stdout)
        self.assertEqual(len(rows), 2)
        self.assertEqual(rows[0]['compatibility'], 'unknown')
        self.assertEqual(rows[1]['basis'], 'user_registered_not_auto_verified')

    def test_ui_auth_empty_host_and_fixed_routes(self):
        adapter, core = self.dir / 'adapter.sock', self.dir / 'core.sock'
        self.processes.extend([start(BIN / 'rfv-adapter-rtw88', 'serve', adapter),
                               start(BIN / 'rfvd', 'serve', core, adapter)])
        with socket.socket() as s:
            s.bind(('127.0.0.1', 0))
            port = s.getsockname()[1]
        token = 'test-only-token-not-for-production-' + 'a' * 32
        old = os.environ.get('RFV_UI_TOKEN')
        os.environ['RFV_UI_TOKEN'] = token
        try:
            self.processes.append(start(BIN / 'rfv', 'ui', core, 'test0', port))
        finally:
            if old is None:
                del os.environ['RFV_UI_TOKEN']
            else:
                os.environ['RFV_UI_TOKEN'] = old
        def request(method, route, body=None, authorized=False):
            conn = http.client.HTTPConnection('127.0.0.1', port, timeout=10)
            headers = {'Authorization': 'Bearer ' + token} if authorized else {}
            conn.request(method, route, body, headers)
            response = conn.getresponse()
            status, content, hdrs = response.status, response.read(), dict(response.getheaders())
            conn.close()
            return status, content, hdrs
        status, content, headers = request('GET', '/')
        self.assertEqual(status, 200)
        self.assertIn(b'RF Link Optimizer', content)
        self.assertNotIn(token.encode(), content)
        self.assertEqual(headers['Cache-Control'], 'no-store')
        self.assertEqual(request('GET', '/api/report')[0], 401)
        status, content, _ = request('GET', '/api/report', authorized=True)
        self.assertEqual(status, 200)
        self.assertFalse(any(r.get('kind') == 'measurement' for r in json.loads(content)))
        self.assertEqual(request('GET', '/api/sample', authorized=True)[0], 400)
        self.assertEqual(request('POST', '/api/txpower', '100', True)[0], 400)
        self.assertEqual(request('GET', '/../../etc/passwd', authorized=True)[0], 400)
        status, content, _ = request('POST', '/api/range', 'N1\tmesh\tunknown\tunknown\tfixture', True)
        self.assertEqual(status, 200)
        self.assertEqual(json.loads(content)[0]['compatibility'], 'unknown')
        self.assertEqual(request('POST', '/api/csi', 'invalid CSI', True)[0], 400)


if __name__ == '__main__':
    unittest.main(verbosity=2)
