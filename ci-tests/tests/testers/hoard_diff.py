from .hoard_tester import HoardTester
from pathlib import Path
import os
import stat
import sys


CONFIG_FILE = "hoard-diff-config.toml"
DEFAULT_TEXT_CONTENT = "This is a text file"
CHANGED_TEXT_CONTENT = "This is different text content"
DEFAULT_BIN_CONTENT = b"\x12\xFE\x2D\x8A\xC1"
CHANGED_BIN_CONTENT = b"\x12\xFE\xD2\x8A\xC1"


class DiffCommandTester(HoardTester):
    def setup(self):
        home = Path.home()
        self.env["HOARD_LOG"] = "info"
        content = {
            "txt": DEFAULT_TEXT_CONTENT,
            "bin": DEFAULT_BIN_CONTENT,
        }

        for hoard_type in ["anon", "named"]:
            for ext, file_content in content.items():
                path = home.joinpath(f"{hoard_type}.{ext}")
                self._write_file(path, file_content, is_binary=(ext == "bin"))

    def _assert_diff_contains(self, target, content, *, partial=False, verbose=False):
        if verbose:
            self.targets = ["-v"]
        else:
            self.targets = []
        self.targets += [target]

        result = self.run_hoard("diff", capture_output=True)
        if partial:
            assert content in result.stdout, f"expected \"{content}\" in \"{result.stdout}\""
        else:
            assert result.stdout == content, f"expected \"{content}\", got \"{result.stdout}\""

    def _test_bin_files(self):
        self.reset(config_file=CONFIG_FILE)
        home = Path.home()
        anon_path = home.joinpath("anon.bin")
        named_path = home.joinpath("named.bin")

        # Not yet backed up
        self._assert_diff_contains("anon_bin", f"{anon_path}: on system, not in the hoard\n".encode())
        self._assert_diff_contains("named", f"{named_path}: on system, not in the hoard\n".encode(), partial=True)

        self.targets = []
        self.run_hoard("backup")

        anon_perms = os.stat(anon_path).st_mode
        named_perms = os.stat(named_path).st_mode

        # No diff
        self._assert_diff_contains("anon_bin", b"")

        os.chmod(anon_path, anon_perms ^ stat.S_IWUSR)
        os.chmod(named_path, named_perms ^ stat.S_IWUSR)

        if sys.platform == "Windows":
            self._assert_diff_contains("anon_bin", f"{anon_path}: permissions changed locally: hoard (writable), system (readonly)\n".encode())
            self._assert_diff_contains("named", f"{named_path}: permissions changed locally: hoard (writable), system (readonly)\n".encode())
        else:
            self._assert_diff_contains("anon_bin", f"{anon_path}: permissions changed locally: hoard (100644), system (100444)\n".encode())
            self._assert_diff_contains("named", f"{named_path}: permissions changed locally: hoard (100644), system (100444)\n".encode())

        os.chmod(anon_path, anon_perms ^ stat.S_IWUSR)
        os.chmod(named_path, named_perms ^ stat.S_IWUSR)

        self._write_file(anon_path, CHANGED_TEXT_CONTENT, is_binary=False)
        self._write_file(named_path, CHANGED_TEXT_CONTENT, is_binary=False)
        # Anon diff
        self._assert_diff_contains("anon_bin", f"{anon_path}: binary file changed locally\n".encode())
        # Named diff
        self._assert_diff_contains("named", f"{named_path}: binary file changed locally\n".encode())

        # Verbose (Unified Diff)
        # Anon diff
        self._assert_diff_contains(
            "anon_bin",
            f"{anon_path}: binary file changed locally\n".encode(),
            verbose=True)
        # Named diff
        self._assert_diff_contains(
            "named",
            f"{named_path}: binary file changed locally\n".encode(),
            verbose=True)

        os.remove(anon_path)
        os.remove(named_path)
        # Anon diff
        self._assert_diff_contains("anon_bin", f"{anon_path}: in hoard, not on the system\n".encode())
        # Named diff
        self._assert_diff_contains("named", f"{named_path}: in hoard, not on the system\n".encode())

    def _test_text_files(self):
        self.reset(config_file=CONFIG_FILE)
        home = Path.home()
        anon_path = home.joinpath("anon.txt")
        named_path = home.joinpath("named.txt")

        # Not yet backed up
        self._assert_diff_contains("anon_txt", f"{anon_path}: on system, not in the hoard\n".encode())
        self._assert_diff_contains("named", f"{named_path}: on system, not in the hoard\n".encode(), partial=True)

        self.targets = []
        self.run_hoard("backup")

        anon_perms = os.stat(anon_path).st_mode
        named_perms = os.stat(named_path).st_mode

        # No diff
        self._assert_diff_contains("anon_txt", b"")

        os.chmod(anon_path, anon_perms ^ stat.S_IWUSR)
        os.chmod(named_path, named_perms ^ stat.S_IWUSR)

        if sys.platform == "Windows":
            self._assert_diff_contains("anon_txt", f"{anon_path}: permissions changed locally: hoard (writable), system (readonly)\n".encode())
            self._assert_diff_contains("named", f"{named_path}: permissions changed locally: hoard (writable), system (readonly)\n".encode())
        else:
            self._assert_diff_contains("anon_txt", f"{anon_path}: permissions changed locally: hoard (100644), system (100444)\n".encode())
            self._assert_diff_contains("named", f"{named_path}: permissions changed locally: hoard (100644), system (100444)\n".encode())

        os.chmod(anon_path, anon_perms)
        os.chmod(named_path, named_perms)

        self._write_file(anon_path, CHANGED_TEXT_CONTENT, is_binary=False)
        self._write_file(named_path, CHANGED_TEXT_CONTENT, is_binary=False)
        # Anon diff
        self._assert_diff_contains("anon_txt", f"{anon_path}: text file changed locally\n".encode())
        # Named diff
        self._assert_diff_contains("named", f"{named_path}: text file changed locally\n".encode())

        # Verbose (Unified Diff)
        # Anon diff
        self._assert_diff_contains(
            "anon_txt",
            (
                f"{anon_path}: text file changed locally\n"
                f"--- {home}/.local/share/hoard/hoards/anon_txt\n"
                f"+++ {anon_path}\n"
            ).encode() + (
                b"@@ -1 +1 @@\n"
                b"-This is a text file\n"
                b"\\ No newline at end of file\n"
                b"+This is different text content\n"
                b"\\ No newline at end of file\n\n"
            ),
            verbose=True)
        # Named diff
        self._assert_diff_contains(
            "named",
            (
                f"{named_path}: text file changed locally\n"
                f"--- {home}/.local/share/hoard/hoards/named/text\n"
                f"+++ {named_path}\n"
            ).encode() + (
                b"@@ -1 +1 @@\n"
                b"-This is a text file\n"
                b"\\ No newline at end of file\n"
                b"+This is different text content\n"
                b"\\ No newline at end of file\n\n"
            ),
            verbose=True)

        os.remove(anon_path)
        os.remove(named_path)
        # Anon diff
        self._assert_diff_contains("anon_txt", f"{anon_path}: in hoard, not on the system\n".encode())
        # Named diff
        self._assert_diff_contains("named", f"{named_path}: in hoard, not on the system\n".encode())



    def run_test(self):
        self._test_text_files()
        self._test_bin_files()
