"""Optional Linux QMP capture for the instrumented ordinary group service."""
import sys
from qemu_capture import main

if __name__ == "__main__":
    sys.exit(main("group_journal", "GROUP_CAPTURE_DIR", idle_symbol="fixed_runtime_idle"))
