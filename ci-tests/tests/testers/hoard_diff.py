from dataclasses import dataclass
from pathlib import Path
import os
import platform
import stat
from .hoard_tester import HoardTester


CONFIG_FILE = "hoard-diff-config.toml"
DEFAULT_TEXT_CONTENT = "This is a text file"
CHANGED_TEXT_CONTENT = "This is different text content"
DEFAULT_BIN_CONTENT = b"\x12\xFE\x2D\x8A\xC1"
CHANGED_BIN_CONTENT = b"\x12\xFE\xD2\x8A\xC1"


@dataclass
class TestEntry:
    path: Path
    hoard_path: Path
    is_text: bool
    ignored: bool = False


class DiffCommandTester(HoardTester):
    def setup(self):
        self.env["HOARD_LOG"] = "info"

    def _assert_diff_contains(self, target, content, *, partial=False, verbose=False, invert=False):
        if verbose:
            self.targets = ["-v"]
        else:
            self.targets = []
        self.targets += [target]

        result = self.run_hoard("diff", capture_output=True)
        if invert:
            assert content not in result.stdout
        elif partial:
            assert content in result.stdout, f"expected \"{content}\" in \"{result.stdout}\""
        else:
            assert result.stdout == content, f"expected \"{content}\", got \"{result.stdout}\""

    def _test_files(self, hoard_pile_mapping):
        self.reset(config_file=CONFIG_FILE)

        for files in hoard_pile_mapping.values():
            for file in files:
                content = DEFAULT_TEXT_CONTENT if file.is_text else DEFAULT_BIN_CONTENT
                self._write_file(file.path, content, is_binary=not file.is_text)

        for hoard, files in hoard_pile_mapping.items():
            has_multiple_files = len(files) > 1
            for file in files:
                self._assert_diff_contains(
                    hoard,
                    f"{file.path}: on system, not in the hoard\n".encode(),
                    partial=has_multiple_files,
                    invert=file.ignored
                )

            self.run_hoard("backup")

            for file in files:
                if file.ignored:
                    continue
                file_perms = os.stat(file.path).st_mode
                # No diff yet
                self._assert_diff_contains(hoard, b"")
                # Toggle write permission
                os.chmod(file.path, file_perms ^ stat.S_IWUSR)

                if platform.system() == "Windows":
                    self._assert_diff_contains(hoard, f"{file.path}: permissions changed locally: hoard (writable), system (readonly)\n".encode())
                else:
                    self._assert_diff_contains(hoard, f"{file.path}: permissions changed locally: hoard (100644), system (100444)\n".encode())

                # Restore permissions
                os.chmod(file.path, file_perms)

                self._write_file(
                    file.path,
                    CHANGED_TEXT_CONTENT if file.is_text else CHANGED_BIN_CONTENT,
                    is_binary=not file.is_text
                )

                file_type = "text" if file.is_text else "binary"
                full_diff = (
                    f"--- {file.hoard_path}\n"
                    f"+++ {file.path}\n"
                    "@@ -1 +1 @@\n"
                    "-This is a text file\n"
                    "\\ No newline at end of file\n"
                    "+This is different text content\n"
                    "\\ No newline at end of file\n\n"
                ) if file.is_text else ""
                self._assert_diff_contains(hoard, f"{file.path}: {file_type} file changed locally\n".encode(), verbose=False)
                self._assert_diff_contains(
                    hoard,
                    f"{file.path}: {file_type} file changed locally\n{full_diff}".encode(),
                    verbose=True
                )

                os.remove(file.path)

                self._assert_diff_contains(hoard, f"{file.path}: in hoard, not on the system\n".encode())

                # Restore to previous state
                content = DEFAULT_TEXT_CONTENT if file.is_text else DEFAULT_BIN_CONTENT
                self._write_file(file.path, content, is_binary=not file.is_text)

    @staticmethod
    def _get_hoard_path(relative_path):
        system = platform.system()
        if system == "Windows":
            return Path.home().joinpath("AppData/shadow53/hoard/data/hoards").joinpath(relative_path)
        if system == "Darwin":
            return Path.home().joinpath("Library/Application Support/com.shadow53/hoard/hoards").joinpath(relative_path)
        if system == "Linux":
            return Path.home().joinpath(".local/share/hoard/hoards").joinpath(relative_path)
        raise RuntimeError(f"Unexpected system: {system}")

    def run_test(self):
        home = Path.home()

        mapping = {
            "anon_dir": [
                TestEntry(
                    path=home.joinpath("testdir", "test.txt"),
                    hoard_path=self._get_hoard_path("anon_dir/test.txt"),
                    is_text=True
                ),
                TestEntry(
                    path=home.joinpath("testdir", "test.bin"),
                    hoard_path=self._get_hoard_path("anon_dir/test.bin"),
                    is_text=False
                ),
                TestEntry(
                    path=home.joinpath("testdir", "ignore.txt"),
                    hoard_path=None,
                    is_text=True,
                    ignored=True,
                ),
            ],
            "anon_txt": [
                TestEntry(
                    path=home.joinpath("anon.txt"),
                    hoard_path=self._get_hoard_path("anon_txt"),
                    is_text=True
                )
            ],
            "named": [
                TestEntry(
                    path=home.joinpath("named.txt"),
                    hoard_path=self._get_hoard_path("named/text"),
                    is_text=True
                ),
                TestEntry(
                    path=home.joinpath("named.bin"),
                    hoard_path=self._get_hoard_path("named/binary"),
                    is_text=False
                )
            ],
        }

        self._test_files(mapping)
