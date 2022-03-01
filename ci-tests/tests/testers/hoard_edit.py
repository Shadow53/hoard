import os
import platform
import subprocess
import sys

if platform.system() == "Windows":
    import winreg
else:
    import pty

from enum import Enum
from pathlib import Path
from tempfile import TemporaryDirectory
from textwrap import dedent

from .hoard_tester import HoardTester


class EditCommandTester(HoardTester):
    def _test_uses_editor(self):
        print("=== Editor: $EDITOR ===")
        self.reset()
        self.using_gui = False
        self._set_editor(Editor.good())
        self._set_gui_editor(Editor.bad())
        self.run_hoard("edit")
        self.verify_editor_called()

    def _test_uses_gui_editor(self):
        print("=== Editor: XDG ===")
        self.reset()
        self.using_gui = True
        self._set_editor(None)
        self._set_gui_editor(Editor.good())
        self.run_hoard("edit")
        self.verify_editor_called()

    def _test_uses_editor_fails(self):
        print("=== Editor: $EDITOR (with error) ===")
        self.reset()
        self.using_gui = False
        self._set_editor(Editor.bad())
        self._set_gui_editor(Editor.good())
        result = self.run_hoard("edit", allow_failure=True)
        assert result.returncode != 0, f"$EDITOR returned exit code {result.returncode}"

    def _test_uses_gui_editor_fails(self):
        print("=== Editor: XDG (with error) ===")
        self.reset()
        self.using_gui = True
        self._set_editor(None)
        self._set_gui_editor(Editor.bad())
        result = self.run_hoard("edit", allow_failure=True)
        assert result.returncode != 0, f"GUI editor returned exit code {result.returncode}"

    def _ensure_watchdog_works(self):
        path = Path.home().joinpath("test.txt")
        if platform.system() == "Windows":
            subprocess.run(["powershell.exe", Editor.good().absolute_path, path], check=True)
        else:
            subprocess.run([Editor.good().absolute_path, path], check=True)
        self.verify_editor_called(path)

    def run_test(self):
        if platform.system() == "Darwin" or platform.system() == "Windows":
            # See note in _set_gui_editor
            return

        self._ensure_watchdog_works()
        self._test_uses_editor()
        self._test_uses_editor_fails()
        self._test_xdg_open_works()
        self._test_uses_gui_editor_fails()
        self._test_uses_gui_editor()
