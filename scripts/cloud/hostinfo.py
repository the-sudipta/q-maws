"""The computer a job runs on (Amendment 10): processor, cores, memory, disk, runner
image and a 10-second single-thread processor calibration (openssl speed sha256)."""
import os
import shutil
import subprocess


def _cpu_model():
    try:
        with open('/proc/cpuinfo', encoding='utf-8') as f:
            for line in f:
                if line.startswith('model name'):
                    return line.split(':', 1)[1].strip()
    except OSError:
        pass
    return 'unknown'


def _memory_gb():
    try:
        with open('/proc/meminfo', encoding='utf-8') as f:
            for line in f:
                if line.startswith('MemTotal:'):
                    return f'{int(line.split()[1]) / 1024 / 1024:.1f}'
    except OSError:
        pass
    return 'NA'


def _calibration():
    """Throughput of one thread on 16 KiB blocks, in kB/s (higher is faster)."""
    try:
        out = subprocess.run(['openssl', 'speed', '-seconds', '10', '-bytes', '16384', 'sha256'],
                             capture_output=True, text=True, timeout=120).stdout
        last = [l for l in out.splitlines() if l.startswith('sha256')][-1]
        return last.split()[-1].rstrip('k')
    except (OSError, IndexError, subprocess.SubprocessError):
        return 'NA'


def describe(work_dir):
    """One dict with the host columns of hosts.tsv."""
    return {
        'cpu_model': _cpu_model(),
        'cores': str(os.cpu_count() or 'NA'),
        'memory_gb': _memory_gb(),
        'disk_free_gb': f'{shutil.disk_usage(work_dir).free / 1e9:.0f}',
        'image': f"{os.environ.get('ImageOS', 'NA')} {os.environ.get('ImageVersion', '')}".strip(),
        'calibration_sha256_16k_kbps': _calibration(),
    }
