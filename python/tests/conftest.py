"""Puts the in-repo package on the path so the suite runs without any install step —
the development-loop counterpart of the other bindings' local native-library staging."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "src"))
