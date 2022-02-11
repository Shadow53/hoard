import os
from pathlib import Path
import uuid

from .hoard_tester import HoardTester


DEFAULT_CONTENT = "default text"
CHANGED_CONTENT = "changed text"
OTHER_CONTENT = "other text"


class StatusCommandTester(HoardTester):
    def setup(self):
        self.env["HOARD_LOG"] = "info"
        self.run_hoard("validate")
        self.local_uuid = str(uuid.uuid4())

        self.run_hoard("validate")
        self.remote_uuid = str(uuid.uuid4())

        self.args = ["--force"]

    def _setup_no_changes(self):
        path = Path.home().joinpath("unchanged.txt")
        self.uuid = self.local_uuid
        self._write_file(path, DEFAULT_CONTENT, is_binary=False)
        self.targets = ["no_changes"]
        self.run_hoard("backup")

    def _setup_local_changes(self):
        path = Path.home().joinpath("local.txt")
        self.uuid = self.remote_uuid
        self._write_file(path, DEFAULT_CONTENT, is_binary=False)
        self.targets = ["local_changes"]
        self.run_hoard("backup")
        self.uuid = self.local_uuid
        self.run_hoard("restore")
        self._write_file(path, CHANGED_CONTENT, is_binary=False)

    def _setup_remote_changes(self):
        path = Path.home().joinpath("remote.txt")
        self.uuid = self.local_uuid
        self._write_file(path, DEFAULT_CONTENT, is_binary=False)
        self.targets = ["remote_changes"]
        self.run_hoard("backup")

        self.uuid = self.remote_uuid
        self.run_hoard("restore")
        self._write_file(path, CHANGED_CONTENT, is_binary=False)
        self.run_hoard("backup")

        self._write_file(path, DEFAULT_CONTENT, is_binary=False)

    def _setup_mixed_changes(self):
        path = Path.home().joinpath("mixed.txt")
        self.uuid = self.local_uuid
        self._write_file(path, DEFAULT_CONTENT, is_binary=False)
        self.targets = ["mixed_changes"]
        self.run_hoard("backup")

        self.uuid = self.remote_uuid
        self._write_file(path, CHANGED_CONTENT, is_binary=False)
        self.run_hoard("backup")

        self._write_file(path, OTHER_CONTENT, is_binary=False)

    def _setup_unexpected_changes(self):
        path = Path.home().joinpath("unexpected.txt")
        hoard_path = self.data_dir_path().joinpath("hoards", "unexpected_changes")
        self._write_file(path, DEFAULT_CONTENT, is_binary=False)
        self.uuid = self.local_uuid
        self.targets = ["unexpected_changes"]
        self.run_hoard("backup", capture_output=True)
        self._write_file(hoard_path, CHANGED_CONTENT, is_binary=False)

    def run_test(self):
        self.reset(config_file="hoard-status-config.toml")
        self._setup_no_changes()
        self._setup_local_changes()
        self._setup_remote_changes()
        self._setup_mixed_changes()
        self._setup_unexpected_changes()

        self.uuid = self.local_uuid
        self.args = []
        self.targets = []
        result = self.run_hoard("status", capture_output=True)

        assert b"no_changes: up to date\n" in result.stdout
        assert b"local_changes: modified locally -- sync with `hoard backup local_changes`\n" in result.stdout, f"got \"{result.stdout}\""
        assert b"remote_changes: modified remotely -- sync with `hoard restore remote_changes`\n" in result.stdout, f"got \"{result.stdout}\""
        assert b"mixed_changes: mixed changes -- manual intervention recommended (see `hoard diff mixed_changes`)\n" in result.stdout, f"got \"{result.stdout}\""
        assert b"unexpected_changes: unexpected changes -- manual intervention recommended (see `hoard diff unexpected_changes`)\n" in result.stdout, f"got \"{result.stdout}\""
