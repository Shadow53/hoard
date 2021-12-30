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


class Editor(str, Enum):
    UNIX = "fake-editor.sh"
    UNIX_ERROR = "fake-error-editor.sh"
    WIN = "fake-editor.ps"
    WIN_ERROR = "fake-error-editor.ps"

    @property
    def absolute_path(self) -> Path:
        return Path(__file__).parent.parent.parent.joinpath("bin", self.value)

    @property
    def desktop_file(self) -> str:
        return f"{self.value}.desktop"

    @staticmethod
    def bad():
        if platform.system() == "Windows":
            return Editor.WIN_ERROR
        else:
            return Editor.UNIX_ERROR

    @staticmethod
    def good():
        if platform.system() == "Windows":
            return Editor.WIN
        else:
            return Editor.UNIX


class EditCommandTester(HoardTester):
    def __init__(self):
        super().__init__()
        self.using_gui = False
        self.env["XDG_CURRENT_DESKTOP"] = "X-Generic"
        self.env["DISPLAY"] = ":0"
        # Create desktop files
        for editor in Editor:
            self._install_desktop_file_for(editor)

    @staticmethod
    def _install_desktop_file_for(editor: Editor) -> None:
        with TemporaryDirectory() as tempdir:
            file_name = f"{tempdir}/{editor.desktop_file}"
            with open(file_name, "w", encoding="utf-8") as file:
                file.write(dedent(f"""\
                    [Desktop Entry]
                    Type=Application
                    Name=Fake Editor
                    GenericName=Editor
                    Categories=System
                    MimeType=text/plain;application/x-yaml
                    Exec={editor.absolute_path} %f
                    """))
                file.close()
                subprocess.run(["xdg-desktop-menu", "install", "--novendor", "--mode", "user", file_name], check=True)

    @staticmethod
    def _set_editor(editor: Editor) -> None:
        if editor is None:
            if "EDITOR" in os.environ:
                os.environ.pop("EDITOR")
        else:
            os.environ["EDITOR"] = str(editor.absolute_path)

    @staticmethod
    def _set_gui_editor(editor: Editor) -> None:
        if platform.system() == "Linux":
            subprocess.run(["xdg-mime", "default", editor.desktop_file, "text/plain"], check=True)
            subprocess.run(["xdg-mime", "default", editor.desktop_file, "application/x-yaml"], check=True)
            result = subprocess.run(["xdg-mime", "query", "default", "text/plain"], check=True, capture_output=True)
            assert editor.desktop_file == result.stdout.decode().strip(), f"expected {editor.desktop_file} == {result.stdout}"
        elif platform.system() == "Windows":
            # Based on https://fekir.info/post/default-text-editor-in-windows/
            value = f"{editor.absolute_path} \"%1\""
            key = winreg.CreateKey(winreg.HKEY_CLASSES_ROOT, "Unknown\\shell\\editor\\command")
            winreg.SetValue(key, "(Default)", winreg.REG_SZ, value)
            key = winreg.CreateKey(winreg.HKEY_CLASSES_ROOT, "Unknown\\shell\\Open\\command")
            winreg.SetValue(key, "(Default)", winreg.REG_SZ, value)
            key = winreg.CreateKey(winreg.HKEY_CLASSES_ROOT, "txtfile\\shell\\Open\\command")
            winreg.SetValue(key, "(Default)", winreg.REG_SZ, value)
        elif platform.system() == "macOS":
            # I cannot for the life of me figure out how to do this. Best option seems to be `duti`,
            # but it does not look like it supports arbitrary script files, only installed packages.
            pass
        else:
            raise RuntimeError(f"Unexpected system {platform.system()}!")

    def _call_hoard(self, args, *, allow_failure, capture_output):
        if self.using_gui or platform.system() == "Windows":
            # Set capture_output to True to not use $EDITOR
            result =  super()._call_hoard(args, allow_failure=allow_failure, capture_output=True)
            sys.stdout.buffer.write(result.stdout)
            sys.stderr.buffer.write(result.stderr)
            self.flush()
            result.stdout = []
            result.stderr = []
            return result

        wait_status = pty.spawn(args)
        exit_code = os.waitstatus_to_exitcode(wait_status)

        if exit_code != 0 and not allow_failure:
            raise OSError(f"hoard process failed with code {exit_code}")
        return subprocess.CompletedProcess(
                args, exit_code,
                stdout=b"" if capture_output else None,
                stderr=b"" if capture_output else None)

    def verify_config(self):
        config_file = self.config_file_path()
        watch_file = Path.home().joinpath("watchdog.txt")
        with open(watch_file, "r") as file:
            # Assert that editor was called
            assert len(file.read().strip()) > 1
        with open(config_file, "r", encoding="utf-8") as file:
            content = file.readline().strip()
            expected = f"opened {config_file} in fake editor"
            assert content == expected, f"expected \"{expected}\", got \"{content}\""
        os.remove(watch_file)

    def _test_xdg_open_works(self):
        print("=== Raw XDG Open ===")
        self.reset()
        self._set_gui_editor(Editor.good())
        self.flush()
        subprocess.run(["xdg-open", self.config_file_path()], check=True)
        self.flush()
        self.verify_config()

    def _test_uses_editor(self):
        print("=== Editor: $EDITOR ===")
        self.reset()
        self.using_gui = False
        self._set_editor(Editor.good())
        self._set_gui_editor(Editor.bad())
        self.flush()
        self.run_hoard("edit")
        self.flush()
        self.verify_config()

    def _test_uses_gui_editor(self):
        print("=== Editor: XDG ===")
        self.reset()
        self.using_gui = True
        self._set_editor(None)
        self._set_gui_editor(Editor.good())
        self.flush()
        self.run_hoard("edit")
        self.verify_config()

    def _test_uses_editor_fails(self):
        print("=== Editor: $EDITOR (with error) ===")
        self.reset()
        self.using_gui = False
        self._set_editor(Editor.bad())
        self._set_gui_editor(Editor.good())
        self.flush()
        result = self.run_hoard("edit", allow_failure=True)
        assert result.returncode != 0, f"$EDITOR returned exit code {result.returncode}"

    def _test_uses_gui_editor_fails(self):
        print("=== Editor: XDG (with error) ===")
        self.reset()
        self.using_gui = True
        self._set_editor(None)
        self._set_gui_editor(Editor.bad())
        self.flush()
        result = self.run_hoard("edit", allow_failure=True)
        assert result.returncode != 0, f"GUI editor returned exit code {result.returncode}"

    def run_test(self):
        if platform.system() == "macOS":
            # See note in _set_gui_editor
            return

        self._test_uses_editor()
        self._test_uses_editor_fails()
        self._test_xdg_open_works()
        self._test_uses_gui_editor_fails()
        self._test_uses_gui_editor()
