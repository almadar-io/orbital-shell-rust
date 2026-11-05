"""desktop_shot.py <resolved.orb> <item> <out_dir> [route] [server]: one orb-desktop Screenshot request ($ORB_DESKTOP); prints the files written."""
import json
import os
import subprocess
import sys

orb, item, out_dir = sys.argv[1], sys.argv[2], sys.argv[3]
route = sys.argv[4] if len(sys.argv) > 4 else None
server = sys.argv[5] if len(sys.argv) > 5 else None
binary = os.environ['ORB_DESKTOP']
envelope = {
    'protocol': 1,
    'request': {'command': 'screenshot', 'renderer': 'dioxus', 'item': item, 'schema': json.load(open(orb)), 'outDir': out_dir, **({'route': route} if route else {}), **({'server': server} if server else {})},
}
proc = subprocess.run([binary, 'request'], input=json.dumps(envelope), capture_output=True, text=True, timeout=180)
sys.stdout.write(proc.stdout)
sys.stderr.write(proc.stderr)
if proc.returncode != 0:
    sys.exit(proc.returncode)
