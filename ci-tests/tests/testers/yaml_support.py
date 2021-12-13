from typing import Dict, Any

import os
from pathlib import Path
import shutil

import yaml
import toml

from .hoard_tester import HoardTester, Hoard, HoardFile, Environment


class YAMLSupportTester(HoardTester):
    def __init__(self):
        super().__init__()
        self.env = {"USE_ENV": "1"}

    @staticmethod
    def _read_toml(toml_path) -> Dict[str, Any]:
        return toml.load(toml_path)

    @staticmethod
    def _save_as_yaml(data: Dict[str, Any], yaml_path) -> None:
        with open(yaml_path, "w", encoding="utf-8") as yaml_file:
            yaml.dump(data, yaml_file)

    @staticmethod
    def _yaml_path(toml_path, yaml_suffix):
        toml_path = Path(toml_path)
        return toml_path.parent.joinpath(f"{toml_path.stem}.{yaml_suffix}")

    def _toml_to_yaml_file(self, *, toml_path, yaml_suffix):
        data = self._read_toml(toml_path)
        with open(self._yaml_path(toml_path, yaml_suffix), "w", encoding="utf-8") as yaml_file:
            yaml.dump(data, yaml_file)

    def _toml_takes_priority(self, *, yaml_suffix):
        self.reset()

        # Create YAML as copy of TOML but without named
        toml_path = self.config_file_path()
        data = self._read_toml(toml_path)
        del data["hoards"][Hoard.Named]
        self._save_as_yaml(data, self._yaml_path(toml_path, yaml_suffix))

        # Run with both files present
        self.run_hoard("backup")
        self.assert_first_tree()

        # Remove config file
        os.remove(toml_path)

        # Reset hoards
        #shutil.rmtree(self.data_dir_path())

        self.run_hoard("backup")

        shutil.rmtree(Path.home().joinpath(f"{Environment.First}_{HoardFile.NamedDir1.value}"))
        shutil.rmtree(Path.home().joinpath(f"{Environment.First}_{HoardFile.NamedDir2.value}"))
        os.remove(Path.home().joinpath(f"{Environment.First}_{HoardFile.NamedFile.value}"))

        self.assert_first_tree()

    def _yaml_behavior_matches_toml(self, yaml_suffix):
        self.reset()
        self._toml_to_yaml_file(toml_path=self.config_file_path(), yaml_suffix=yaml_suffix)
        os.remove(self.config_file_path())

        self.run_hoard("backup")
        self.assert_first_tree()

    def run_test(self):
        for suffix in ["yaml", "yml"]:
            self._toml_takes_priority(yaml_suffix=suffix)
            self._yaml_behavior_matches_toml(yaml_suffix=suffix)
