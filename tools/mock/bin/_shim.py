"""Shared by the herdr and gh shims: find the real command behind a shim."""
import os


def real(name, shim_dir):
    for d in os.environ.get("PATH", "").split(os.pathsep):
        if d and os.path.realpath(d) != os.path.realpath(shim_dir):
            p = os.path.join(d, name)
            if os.path.isfile(p) and os.access(p, os.X_OK):
                return p
    return None
