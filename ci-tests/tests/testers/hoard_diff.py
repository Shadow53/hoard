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

    def _setup_test(self, hoard_pile_mapping, *, remote, backup=True):
        self.reset(config_file=CONFIG_FILE)
        self.location = "remotely" if remote else "locally"

        for hoard, files in hoard_pile_mapping.items():
            for file in files:
                self._reset_file(file)

            if backup:
                self.targets = [hoard]
                self.run_hoard("backup")
                self.original_uuid = self.uuid

        if backup and remote:
            os.remove(self.get_uuid_path())

    def _reset_file(self, file):
        content = DEFAULT_TEXT_CONTENT if file.is_text else DEFAULT_BIN_CONTENT
        self._write_file(file.path, content, is_binary=not file.is_text)

    def _test_files_modified(self, hoard_pile_mapping, *, remote):
        self._setup_test(hoard_pile_mapping, remote=remote)
        for hoard, files in hoard_pile_mapping.items():
            for file in files:
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

                self._assert_diff_contains(
                    hoard,
                    f"{file.path}: {file_type} file changed {self.location}\n".encode(),
                    verbose=False,
                    invert=file.ignored,
                )
                self._assert_diff_contains(
                    hoard,
                    f"{file.path}: {file_type} file changed {self.location}\n{full_diff}".encode(),
                    verbose=True,
                    invert=file.ignored,
                )

                self._write_file(
                    file.path,
                    DEFAULT_TEXT_CONTENT if file.is_text else DEFAULT_BIN_CONTENT,
                    is_binary=not file.is_text
                )

    def _test_files_permissions_changed(self, hoard_pile_mapping, *, remote):
        self._setup_test(hoard_pile_mapping, remote=remote)
        for hoard, files in hoard_pile_mapping.items():
            for file in files:
                file_perms = os.stat(file.path).st_mode
                # No diff yet
                self._assert_diff_contains(hoard, b"")
                # Toggle write permission
                os.chmod(file.path, file_perms ^ stat.S_IWUSR)

                if platform.system() == "Windows":
                    self._assert_diff_contains(
                        hoard,
                        f"{file.path}: permissions changed {self.location}: hoard (writable), system (readonly)\n".encode(),
                        invert=file.ignored
                    )
                else:
                    self._assert_diff_contains(hoard, f"{file.path}: permissions changed {self.location}: hoard (100644), system (100444)\n".encode(),
                        invert=file.ignored
                    )

                # Restore permissions
                os.chmod(file.path, file_perms)

    def _test_files_created(self, hoard_pile_mapping, *, remote):
        self._setup_test(hoard_pile_mapping, backup=remote, remote=remote)
        for hoard, files in hoard_pile_mapping.items():
            has_multiple_files = len(files) > 1
            for file in files:
                if remote:
                    os.remove(file.path)

                self._assert_diff_contains(
                    hoard,
                    f"{file.path}: created {self.location}\n".encode(),
                    partial=has_multiple_files,
                    invert=file.ignored
                )

    def _test_files_recreated(self, hoard_pile_mapping, remote):
        # TODO: Remove early return when syncing deletions is implemented in next task
        return 
        self._setup_test(hoard_pile_mapping, remote=remote)
        for hoard, files in hoard_pile_mapping.items():
            has_multiple_files = len(files) > 1
            for file in files:
                os.remove(file.path)

            self.targets = [hoard]
            self.args = ["--force"]
            self.run_hoard("backup")
            self.args = []

            if remote:
                os.remove(self.get_uuid_path())

            for file in files:
                self._reset_file(file)

            if remote:
                self.args = ["--force"]
                self.run_hoard("backup")
                self.args = []
                self.uuid = self.original_uuid
                for file in files:
                    os.remove(file.path)

            for file in files:
                self._assert_diff_contains(
                    hoard,
                    f"{file.path}: recreated {self.location}\n".encode(),
                    partial=has_multiple_files,
                    invert=file.ignored
                )

    def _test_files_deleted(self, hoard_pile_mapping, *, remote):
        # TODO: Remove early return when syncing deletions is implemented in next task
        return 
        self._setup_test(hoard_pile_mapping, remote=remote)
        for hoard, files in hoard_pile_mapping.items():
            has_multiple_files = len(files) > 1
            for file in files:
                os.remove(file.path)

            if remote:
                self.args = ["--force"]
                self.run_hoard("backup")
                self.args = []
                self.uuid = self.original_uuid
                for file in files:
                    self._reset_file(file)

            for file in files:
                self._assert_diff_contains(
                    hoard,
                    f"{file.path}: deleted {self.location}\n".encode(),
                    partial=has_multiple_files,
                    invert=file.ignored
                )

                self._reset_file(file)

    def _test_files_unchanged(self, hoard_pile_mapping, *, remote):
        self._setup_test(hoard_pile_mapping, remote=remote)
        for hoard in hoard_pile_mapping.keys():
            self._assert_diff_contains(hoard, b"")

    @staticmethod
    def _get_hoard_path(relative_path):
        system = platform.system()
        if system == "Windows":
            return Path.home().joinpath("AppData/shadow53/hoard/data/hoards").joinpath(relative_path)
        if system == "Darwin":
            return Path.home().joinpath("Library/Application Support/com.shadow53.hoard/hoards").joinpath(relative_path)
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

        for remote in [True, False]:
            self._test_files_created(mapping, remote=remote)
            self._test_files_recreated(mapping, remote=remote)
            self._test_files_deleted(mapping, remote=remote)
            self._test_files_modified(mapping, remote=remote)
            self._test_files_permissions_changed(mapping, remote=remote)
            self._test_files_unchanged(mapping, remote=remote)

