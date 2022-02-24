import os

from .hoard_tester import HoardTester


class CorrectErrorsTester(HoardTester):
    def _run_no_invalid_filter_directive(self):
        self.reset()
        self.env = {"HOARD_LOG": None}
        result = self.run_hoard("validate", capture_output=True)
        expected = b"ignoring ``: invalid filter directive"
        assert result.stderr is None or expected not in result.stderr
        assert result.stdout is None or expected not in result.stdout

    def _run_missing_config(self):
        self.reset()
        os.remove(self.config_file_path())
        result = self.run_hoard("validate", allow_failure=True, capture_output=True)
        assert b"could not find any of config." in result.stdout

    def run_test(self):
        self._run_missing_config()
        self._run_no_invalid_filter_directive()
