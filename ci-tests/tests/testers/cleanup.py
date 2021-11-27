from .hoard_tester import HoardTester, Hoard, HoardFile, Environment
from uuid import uuid4
import os
import secrets
import subprocess


class LogCleanupTester(HoardTester):
    def __init__(self):
        super().__init__()
        self.reset()
        self.env = {"USE_ENV": "1"}
        self.system1 = str(uuid4())
        self.system2 = str(uuid4())
        # The number of `hoard` runs may seem excessive, but this is to
        # simulate enough runs that one might want to clean up log files.
        self.strategy = [
            # All AnonDir operations
            (self.system1, "backup", Hoard.AnonDir),
            (self.system2, "restore", Hoard.AnonDir),
            (self.system1, "backup", Hoard.AnonDir),
            (self.system2, "restore", Hoard.AnonDir),
            (self.system2, "backup", Hoard.AnonDir),
            (self.system1, "restore", Hoard.AnonDir),
            (self.system1, "backup", Hoard.AnonDir),
            (self.system2, "restore", Hoard.AnonDir),
            (self.system2, "backup", Hoard.AnonDir),
            (self.system1, "restore", Hoard.AnonDir),
            (self.system2, "restore", Hoard.AnonDir),
            # All AnonFile operations
            (self.system1, "backup", Hoard.AnonFile),
            (self.system1, "backup", Hoard.AnonFile),
            (self.system1, "restore", Hoard.AnonFile),
            (self.system2, "restore", Hoard.AnonFile),
            (self.system1, "backup", Hoard.AnonFile),
            (self.system1, "restore", Hoard.AnonFile),
            (self.system1, "backup", Hoard.AnonFile),
            (self.system2, "restore", Hoard.AnonFile),
            # All Named operations
            (self.system2, "backup", Hoard.Named),
            (self.system1, "restore", Hoard.Named),
            (self.system2, "backup", Hoard.Named),
            (self.system1, "restore", Hoard.Named),
            (self.system1, "backup", Hoard.Named),
            (self.system1, "restore", Hoard.Named),
            (self.system2, "restore", Hoard.Named),
            (self.system2, "backup", Hoard.Named),
            (self.system1, "restore", Hoard.Named),
            (self.system2, "backup", Hoard.Named),
        ]
        self.retained = {
            self.system1: {
                Hoard.AnonFile: [5],
                Hoard.AnonDir: [3, 4],
                Hoard.Named: [2, 4],
            },
            self.system2: {
                Hoard.AnonFile: [1],
                Hoard.AnonDir: [4, 5],
                Hoard.Named: [4],
            }
        }

        # In the end, these should be the final results:
        # bkup = backups
        # rstr = restore
        # ALL CAPS indicates the most recent operation type
        #
        # +-------------------------------------------+
        # |         | anon_dir | anon_file |  named   |
        # +---------+----------+-----------+----------+
        # | system1 | bkup x 3 | BKUP x 4  | bkup x 1 |
        # |         | RSTR x 2 | rstr x 2  | RSTR x 4 |
        # +---------+----------+-----------+----------+
        # | system2 | bkup x 2 | bkup x 0  | BKUP x 4 |
        # |         | RSTR x 4 | RSTR x 2  | rstr x 1 |
        # +---------+----------+-----------+----------+

    @classmethod
    def _set_uuid_file(cls, system_id):
        with open(cls.get_uuid_path(), 'w') as file:
            file.write(system_id)

    @classmethod
    def _regenerate_file_in_hoard(cls, hoard):
        content = secrets.token_bytes(1024)
        if hoard is Hoard.AnonFile:
            cls.write_hoard_file(Environment.First, HoardFile.AnonFile, content)
        elif hoard is Hoard.AnonDir:
            cls.write_hoard_file(Environment.First, HoardFile.AnonDir1, content)
        elif hoard is Hoard.Named:
            cls.write_hoard_file(Environment.First, HoardFile.NamedDir11, content)

    def _run_operation(self, system_id, cmd, hoard):
        self._regenerate_file_in_hoard(hoard)
        self._set_uuid_file(system_id)
        self.targets = [hoard.value]
        self.run_hoard(cmd)

    def run_test(self):
        # Run all of the commands
        for system_id, command, hoard in self.strategy:
            self._run_operation(system_id, command, hoard)

        expected = {}

        for system_id, retained in self.retained.items():
            expected[system_id] = {}
            for hoard, indices in retained.items():
                path = self.data_dir_path().joinpath("history").joinpath(system_id).joinpath(hoard.value)
                files = list(os.listdir(path))
                files.sort()
                files = [file for (i, file) in enumerate(files) if i in indices and "last_paths" not in file]
                expected[system_id][hoard] = files

        self.targets = []
        self.run_hoard("cleanup")

        for system_id, retained in self.retained.items():
            for hoard, _ in retained.items():
                path = self.data_dir_path().joinpath("history").joinpath(system_id).joinpath(hoard.value)
                retained_files = [file for file in os.listdir(path) if "last_paths" not in file]
                expected_files = expected[system_id][hoard]
                assert len(retained_files) == len(expected_files)
                for file in expected_files:
                    assert file in retained_files

        # The following should still fail after cleanup
        try:
            self._run_operation(self.system1, "backup", Hoard.Named)
            raise AssertionError("Backup should have failed")
        except subprocess.CalledProcessError:
            pass

        # The following should succeed
        self._run_operation(self.system1, "backup", Hoard.AnonDir)
        self._run_operation(self.system1, "backup", Hoard.AnonFile)
