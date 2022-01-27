import os

from .hoard_tester import HoardTester


class CorrectErrorsTester(HoardTester):
    def _run_missing_parent_test(self):
        self.reset(config_file="missing-parent-config.toml")
        result = self.run_hoard("backup", allow_failure=True, capture_output=True)
        assert b"source path does not exist" in result.stdout

    def _run_env_string_named_config_test(self):
        self.reset(config_file="env-string-named-config.toml")
        result = self.run_hoard("validate", allow_failure=True, capture_output=True)
        assert b'the name "config" is not allowed at: ["envs", "config"]' in result.stdout

    def _run_hoard_named_config_test(self):
        self.reset(config_file="hoard-named-config.toml")
        result = self.run_hoard("validate", allow_failure=True, capture_output=True)
        assert b'the name "config" is not allowed at: ["hoards", "config"]' in result.stdout

    def _run_pile_named_config_test(self):
        self.reset(config_file="pile-named-config.toml")
        result = self.run_hoard("validate", allow_failure=True, capture_output=True)
        assert b'expected "config" to be a pile config: at ["hoards", "invalid_named_pile", "config"]' in result.stdout

    def _run_warn_on_invalid_uuid(self):
        self.reset()
        invalid_id = "invalid"
        self.uuid = invalid_id
        result = self.run_hoard("backup", capture_output=True)
        assert self.uuid != invalid_id
        assert b'failed to parse uuid in file' in result.stdout

    def _run_no_invalid_filter_directive(self):
        self.reset()
        self.env = {"HOARD_LOG": None}
        result = self.run_hoard("validate", capture_output=True)
        expected = b"ignoring ``: invalid filter directive"
        assert result.stderr is None or expected not in result.stderr
        assert result.stdout is None or expected not in result.stdout

    def _run_invalid_config_extension(self):
        self.reset()
        expected_text = b"configuration file must have file extension \""

        # Missing extension
        old_config_file = self.config_file_path()
        new_config_file = old_config_file.parent.joinpath(old_config_file.stem)
        os.rename(old_config_file, new_config_file)
        self.args = ["--config-file", str(new_config_file)]
        result = self.run_hoard("validate", allow_failure=True, capture_output=True)
        assert expected_text in result.stdout

        # Bad extension
        old_config_file = new_config_file
        new_config_file = old_config_file.parent.joinpath(f"{old_config_file.stem}.conf")
        os.rename(old_config_file, new_config_file)
        self.args = ["--config-file", str(new_config_file)]
        result = self.run_hoard("validate", allow_failure=True, capture_output=True)
        self.args = []
        assert expected_text in result.stdout

    def _run_missing_config(self):
        self.reset()
        os.remove(self.config_file_path())
        result = self.run_hoard("validate", allow_failure=True, capture_output=True)
        assert b"could not find any of config." in result.stdout

    def run_test(self):
        self._run_missing_parent_test()
        self._run_pile_named_config_test()
        self._run_env_string_named_config_test()
        self._run_hoard_named_config_test()
        self._run_warn_on_invalid_uuid()
        self._run_invalid_config_extension()
        self._run_missing_config()
        self._run_no_invalid_filter_directive()
