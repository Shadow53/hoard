from dataclasses import dataclass
from enum import Enum
from pathlib import Path
import os
import platform
import stat
import uuid
from .hoard_tester import HoardTester


CONFIG_FILE = "hoard-diff-config.toml"
DEFAULT_CONTENT = ("This is a text file", b"\x12\xFB\x3D\x00\x3A")
CHANGED_CONTENT = ("This is different text content", b"\x12\xFB\x45\x00\x3A")
OTHER_CONTENT = ("This is yet other text content", b"\x12\xFB\x91\x00\x3A")


@dataclass
class TestEntry:
    path: Path
    hoard_path: Path
    is_text: bool
    ignored: bool = False


class ContentType(Enum):
    DEFAULT = 0
    CHANGE_FILE_A = 1
    CHANGE_FILE_B = 2


class DiffCommandTester(HoardTester):
    def setup(self):
        self.env["HOARD_LOG"] = "info"
        self.local_uuid = str(uuid.uuid4())
        self.remote_uuid = str(uuid.uuid4())

    def _assert_diff_contains(self, target, content, *, partial=False, verbose=False, invert=False):
        if verbose:
            self.targets = ["-v"]
        else:
            self.targets = []
        self.targets += [target]

        result = self.run_hoard("diff", capture_output=True)
        if invert:
            assert content not in result.stdout
        #elif partial:
        else:
            assert content in result.stdout, f"expected \"{content}\" in \"{result.stdout}\""
        #else:
        #    assert result.stdout == content, f"expected \"{content}\", got \"{result.stdout}\""

    def _backup(self, hoard, *, is_remote):
        if is_remote:
            self.uuid = self.remote_uuid
        else:
            self.uuid = self.local_uuid

        self.targets = [hoard]
        self.args = []
        self.run_hoard("backup")

    def _restore(self, hoard, *, is_remote):
        if is_remote:
            self.uuid = self.remote_uuid
        else:
            self.uuid = self.local_uuid

        self.targets = [hoard]
        self.args = []
        self.run_hoard("restore")

    def _test_files_unchanged(self, hoard_pile_mapping, *, remote):
        self.reset(config_file=CONFIG_FILE)
        for hoard, files in hoard_pile_mapping.items():
            self._reset_all(files)
            self._backup(hoard, is_remote=remote)
            if remote:
                self._restore(hoard, is_remote=False)
            self._assert_diff_contains(hoard, b"")

    def _get_hoard_path(self, relative_path):
        return self.data_dir_path().joinpath("hoards", relative_path)

    def _test_local(self, hoard_pile_mapping, *, setup, file_contents, modify_file, check_diff):
        # Last update locally, then local changes
        location = "locally"
        self.reset(config_file=CONFIG_FILE)
        for hoard, files in hoard_pile_mapping.items():
            for file in files:
                setup(path=file.path, content=file_contents[ContentType.DEFAULT], is_text=file.is_text, hoard=hoard)
            for file in files:
                modify_file(path=file.path, content=file_contents[ContentType.CHANGE_FILE_A], is_text=file.is_text, hoard=hoard)
                self.uuid = self.local_uuid
                check_diff(
                    file=file,
                    hoard=hoard,
                    location=location,
                    partial=len(files) > 1,
                    hoard_content=file_contents[ContentType.DEFAULT],
                    system_content=file_contents[ContentType.CHANGE_FILE_A],
                )
                modify_file(path=file.path, content=file_contents[ContentType.DEFAULT], is_text=file.is_text, hoard=hoard)

        # Last update remotely, then local changes
        self.reset(config_file=CONFIG_FILE)
        for hoard, files in hoard_pile_mapping.items():
            for file in files:
                setup(path=file.path, content=file_contents[ContentType.DEFAULT], is_text=file.is_text, hoard=hoard)
            self._backup(hoard, is_remote=True)
            self._restore(hoard, is_remote=False)
            for file in files:
                modify_file(path=file.path, content=file_contents[ContentType.CHANGE_FILE_A], is_text=file.is_text, hoard=hoard)
                check_diff(
                    file=file,
                    hoard=hoard,
                    location=location,
                    partial=len(files) > 1,
                    hoard_content=file_contents[ContentType.DEFAULT],
                    system_content=file_contents[ContentType.CHANGE_FILE_A],
                )

    def _test_remote(self, hoard_pile_mapping, *, setup, file_contents, modify_file, check_diff):
        location = "remotely"
        self.reset(config_file=CONFIG_FILE)
        for hoard, files in hoard_pile_mapping.items():
            for file in files:
                setup(path=file.path, content=file_contents[ContentType.DEFAULT], is_text=file.is_text, hoard=hoard)
            for file in files:
                modify_file(path=file.path, content=file_contents[ContentType.CHANGE_FILE_A], is_text=file.is_text, hoard=hoard)
            self._backup(hoard, is_remote=True)
            for file in files:
                modify_file(path=file.path, content=file_contents[ContentType.DEFAULT], is_text=file.is_text, hoard=hoard)

            self.uuid = self.local_uuid
            for file in files:
                check_diff(
                    file=file,
                    hoard=hoard,
                    location=location,
                    partial=len(files) > 1,
                    hoard_content=file_contents[ContentType.CHANGE_FILE_A],
                    system_content=file_contents[ContentType.DEFAULT]
                )

    def _test_mixed(self, hoard_pile_mapping, *, setup, file_contents, modify_file, check_diff):
        location = "locally and remotely"
        self.reset(config_file=CONFIG_FILE)
        for hoard, files in hoard_pile_mapping.items():
            for file in files:
                setup(path=file.hoard_path, content=file_contents[ContentType.DEFAULT], is_text=file.is_text, hoard=hoard)
            for file in files:
                modify_file(path=file.path, content=file_contents[ContentType.CHANGE_FILE_A], is_text=file.is_text, hoard=hoard)
            self._backup(hoard, is_remote=True)
            self.uuid = self.local_uuid
            for file in files:
                modify_file(path=file.path, content=file_contents[ContentType.CHANGE_FILE_B], is_text=file.is_text, hoard=hoard)
                check_diff(
                    file=file,
                    hoard=hoard,
                    location=location,
                    partial=len(files) > 1,
                    hoard_content=file_contents[ContentType.CHANGE_FILE_A],
                    system_content=file_contents[ContentType.CHANGE_FILE_B]
                )

    def _test_unexpected(self, hoard_pile_mapping, *, setup, file_contents, modify_file, check_diff):
        location = "out-of-band"
        self.reset(config_file=CONFIG_FILE)
        for hoard, files in hoard_pile_mapping.items():
            for file in files:
                setup(path=file.path, content=file_contents[ContentType.DEFAULT], is_text=file.is_text, hoard=hoard)
            for file in files:
                modify_file(path=file.hoard_path, content=file_contents[ContentType.CHANGE_FILE_A], is_text=file.is_text, hoard=hoard)
                check_diff(
                    file=file,
                    hoard=hoard,
                    location=location,
                    partial=len(files) > 1,
                    hoard_content=file_contents[ContentType.CHANGE_FILE_A],
                    system_content=file_contents[ContentType.DEFAULT],
                )

    def _test_unchanged(self, hoard_pile_mapping, *, setup, file_contents, modify_file):
        self.reset(config_file=CONFIG_FILE)
        for hoard, files in hoard_pile_mapping.items():
            for file in files:
                setup(path=file.path, content=file_contents[ContentType.DEFAULT], is_text=file.is_text, hoard=hoard)
            self._backup(hoard, is_remote=False)
            for file in files:
                modify_file(path=file.path, content=file_contents[ContentType.DEFAULT], is_text=file.is_text, hoard=hoard)
                self._assert_diff_contains(hoard, b"")

    def no_op(self, *, hoard, path, is_text):
        return None

    def setup_modify(self, *, path, content, is_text, hoard):
        self.modify_file(path=path, content=content, is_text=is_text, hoard=hoard)
        self._backup(hoard, is_remote=False)
        self._restore(hoard, is_remote=True)

    def setup_permissions(self, *, path, content, is_text, hoard):
        self.modify_file(path=path, content=DEFAULT_CONTENT, is_text=is_text, hoard=hoard)
        self.setup_modify(path=path, content=content, is_text=is_text, hoard=hoard)

    def setup_recreate(self, *, path, content, is_text, hoard):
        self.modify_file(path=path, content=content, is_text=is_text, hoard=hoard)
        self._backup(hoard, is_remote=False)
        self.modify_file(path=path, content=None, is_text=is_text, hoard=hoard)
        self._backup(hoard, is_remote=False)

    def modify_file(self, *, path, content, is_text, hoard):
        if path is None:
            return
        if content is None:
            if path.exists():
                os.remove(path)
        elif isinstance(content, int):
            os.chmod(path, content)
        else:
            content = content[0] if is_text else content[1]
            self._write_file(path, content, is_binary=not is_text)

    def check_recreated_file(self, *, file, hoard, location, partial, hoard_content, system_content):
        self._assert_diff_contains(
            hoard,
            f"{file.path}: recreated {location}\n".encode(),
            partial=partial,
            invert=file.ignored
        )

    def check_created_file(self, *, file, hoard, location, partial, hoard_content, system_content):
        self._assert_diff_contains(
            hoard,
            f"{file.path}: created {location}\n".encode(),
            partial=partial,
            invert=file.ignored
        )

    def check_modified_file(self, *, file, hoard, location, partial, hoard_content, system_content):
        file_type = "text" if file.is_text else "binary"
        full_diff = (
            f"--- {file.hoard_path}\n"
            f"+++ {file.path}\n"
            "@@ -1 +1 @@\n"
            f"-{hoard_content[0]}\n"
            "\\ No newline at end of file\n"
            f"+{system_content[0]}\n"
            "\\ No newline at end of file\n\n"
        ) if file.is_text else ""

        self._assert_diff_contains(
            hoard,
            f"{file.path}: {file_type} file changed {location}\n".encode(),
            verbose=False,
            invert=file.ignored,
            partial=partial,
        )
        self._assert_diff_contains(
            hoard,
            f"{file.path}: {file_type} file changed {location}\n{full_diff}".encode(),
            verbose=True,
            invert=file.ignored,
            partial=partial,
        )

    def check_modified_perms(self, *, file, hoard, location, partial, system_content, hoard_content):
        if platform.system() == "Windows":
            hoard_writable = "writable" if hoard_content & (stat.S_IWUSR) != 0 else "readonly"
            system_writable = "writable" if system_content & (stat.S_IWUSR) != 0 else "readonly"
            self._assert_diff_contains(
                hoard,
                f"{file.path}: permissions changed: hoard ({hoard_writable}), system ({system_writable})\n".encode(),
                invert=file.ignored,
                partial=partial,
            )
        else:
            self._assert_diff_contains(
                hoard,
                f"{file.path}: permissions changed: hoard ({oct(hoard_content)[2:]}), system ({oct(system_content)[2:]})\n".encode(),
                invert=file.ignored,
                partial=partial,
            )

    def check_deleted_file(self, *, file, hoard, location, partial):
        self._assert_diff_contains(
            hoard,
            f"{file.path}: deleted {location}\n".encode(),
            partial=partial,
            invert=file.ignored
        )

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

        create_file_contents = {
            ContentType.DEFAULT: None,
            ContentType.CHANGE_FILE_A: DEFAULT_CONTENT,
            ContentType.CHANGE_FILE_B: CHANGED_CONTENT,
        }

        delete_file_contents = {
            ContentType.DEFAULT: DEFAULT_CONTENT,
            ContentType.CHANGE_FILE_A: None,
            ContentType.CHANGE_FILE_B: None,
        }

        modify_file_contents = {
            ContentType.DEFAULT: DEFAULT_CONTENT,
            ContentType.CHANGE_FILE_A: CHANGED_CONTENT,
            ContentType.CHANGE_FILE_B: OTHER_CONTENT,
        }

        modify_file_perms = {
            ContentType.DEFAULT: 0o100644,
            ContentType.CHANGE_FILE_A: 0o100444,
            ContentType.CHANGE_FILE_B: 0o100755,
        }

        # [ setup, file_contents, modify_file, check_diff ]
        funcs = [
            # Created
            (self.modify_file, create_file_contents, self.modify_file, self.check_created_file),
            # Modified
            (self.setup_modify, modify_file_contents, self.modify_file, self.check_modified_file),
            # Permissions
            (self.setup_permissions, modify_file_perms, self.modify_file, self.check_modified_perms),
            # Deleted
            # (self.setup_modify, delete_file_contents, self.modify_file, self.check_deleted_file),
            # Recreated
            # (self.setup_recreate, create_file_contents, self.modify_file, self.check_recreated_file),
        ]

        try:
            for (setup, file_contents, modify_file, check_diff) in funcs:
                self._test_local(
                    mapping,
                    setup=setup,
                    file_contents=file_contents,
                    modify_file=modify_file,
                    check_diff=check_diff,
                )
                self._test_remote(
                    mapping,
                    setup=setup,
                    file_contents=file_contents,
                    modify_file=modify_file,
                    check_diff=check_diff,
                )
                self._test_mixed(
                    mapping,
                    setup=setup,
                    file_contents=file_contents,
                    modify_file=modify_file,
                    check_diff=check_diff,
                )
                self._test_unexpected(
                    mapping,
                    setup=setup,
                    file_contents=file_contents,
                    modify_file=modify_file,
                    check_diff=check_diff,
                )
                self._test_unchanged(
                    mapping,
                    setup=setup,
                    file_contents=file_contents,
                    modify_file=modify_file,
                )
        except AssertionError:
            for files in mapping.values():
                for file in files:
                    if file.path.exists():
                        with open(file.path, "r" if file.is_text else "rb") as f:
                            print(f"{file.path}: \"{f.read()}\"")
                    else:
                        print(f"{file.path}: <empty>")
            raise

