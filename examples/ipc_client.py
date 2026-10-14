#!/usr/bin/env python3
"""Independent RFV/1 client example; JSON strings retain precise counter integers."""
import json
import socket
import struct
import sys
HEADER = struct.Struct('!4sHHHHQI')

def encode(kind, request_id, records=(), flags=0):
    payload = bytearray(struct.pack('!I', len(records)))
    for row in records:
        payload += struct.pack('!H', len(row))
        for key, val in sorted(row.items()):
            key, val = key.encode(), val.encode()
            if not 1 <= len(key) <= 64 or len(val) > 65536:
                raise ValueError('field limit')
            payload += struct.pack('!HI', len(key), len(val)) + key + val
    if len(payload) > 1048576:
        raise ValueError('frame limit')
    return HEADER.pack(b'RFV1', 1, 0, kind, flags, request_id, len(payload)) + payload

def exact(sock, size):
    buf = bytearray()
    while len(buf) < size:
        chunk = sock.recv(size - len(buf))
        if not chunk:
            raise EOFError('truncated RFV frame')
        buf += chunk
    return bytes(buf)

def receive(sock):
    magic, major, minor, kind, flags, request_id, length = HEADER.unpack(exact(sock, 24))
    if (magic, major, minor) != (b'RFV1', 1, 0) or flags not in (0, 1) or not 4 <= length <= 1048576:
        raise ValueError('unsupported header')
    payload = exact(sock, length)
    at = 0
    def take(n):
        nonlocal at
        if at + n > len(payload):
            raise ValueError('truncated payload')
        part = payload[at:at + n]
        at += n
        return part
    count, = struct.unpack('!I', take(4))
    if count > 4096:
        raise ValueError('record limit')
    rows = []
    for _ in range(count):
        fields, = struct.unpack('!H', take(2))
        if fields > 64:
            raise ValueError('field count')
        row = {}
        for _ in range(fields):
            klen, vlen = struct.unpack('!HI', take(6))
            if not 1 <= klen <= 64 or vlen > 65536:
                raise ValueError('field limit')
            key, val = take(klen).decode(), take(vlen).decode()
            if key in row:
                raise ValueError('duplicate field')
            row[key] = val
        rows.append(row)
    if at != len(payload):
        raise ValueError('trailing bytes')
    return kind, flags, request_id, rows

def query(path, interface=None):
    with socket.socket(socket.AF_UNIX) as s:
        s.settimeout(20)
        s.connect(path)
        s.sendall(encode(1, 1, [{'protocol': 'RFV/1.0'}]))
        hello = receive(s)
        if hello[:3] != (1, 1, 1) or hello[3][0].get('protocol') != 'RFV/1.0':
            raise ValueError('negotiation failed')
        kind = 3 if interface else 2
        s.sendall(encode(kind, 2, [{'interface': interface}] if interface else []))
        response = receive(s)
        if response[:3] != (kind, 1, 2):
            raise ValueError(response)
        return response[3]
if __name__ == '__main__':
    if len(sys.argv) not in (2, 3):
        raise SystemExit('usage: ipc_client.py PRIVATE_SOCKET [INTERFACE]; use only a trusted same-user socket')
    print(json.dumps(query(*sys.argv[1:]), indent=2))
