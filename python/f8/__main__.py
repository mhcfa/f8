"""Module execution entrypoint for python -m f8."""

from __future__ import annotations

import sys

from f8.cli import main

if __name__ == "__main__":
    sys.exit(main())
