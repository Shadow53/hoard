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

    def run_test(self):
        self._run_missing_parent_test()
        self._run_pile_named_config_test()
        self._run_env_string_named_config_test()
        self._run_hoard_named_config_test()
        self._run_warn_on_invalid_uuid()


