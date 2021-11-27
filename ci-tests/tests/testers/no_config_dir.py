import os

from tempfile import TemporaryDirectory
from .hoard_tester import HoardTester


class MissingConfigDirTester(HoardTester):
    def run_test(self):
        self.reset()
        old_config_path = self.config_file_path()
        with TemporaryDirectory() as tmpdir:
            os.environ.setdefault("XDG_CONFIG_HOME", tmpdir)
            self.args = ["--config-file", old_config_path]
            result = self.run_hoard("backup", allow_failure=True, capture_output=True)
            os.environ.pop("XDG_CONFIG_HOME")
            assert b"error while saving uuid to file" not in result.stderr and b"No such file or directory" not in result.stderr
            assert b"error while saving uuid to file" not in result.stdout and b"No such file or directory" not in result.stdout
