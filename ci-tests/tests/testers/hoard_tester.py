from abc import ABC, abstractmethod
from pathlib import Path
from enum import Enum
import filecmp
import os
import platform
import secrets
import shutil
import subprocess
import sys
import time


HOARDS_DIRNAME = "hoards"


class Environment(str, Enum):
    First = "first"
    Second = "second"


class Hoard(str, Enum):
    AnonDir = "anon_dir"
    AnonFile = "anon_file"
    Named = "named"


class HoardFile(str, Enum):
    AnonFile = "anon_file"
    AnonDir = "anon_dir"
    AnonDir1 = "anon_dir/1"
    AnonDir2 = "anon_dir/2"
    AnonDir3 = "anon_dir/3"
    NamedFile = "named_file"
    NamedDir1 = "named_dir1"
    NamedDir11 = "named_dir1/1"
    NamedDir12 = "named_dir1/2"
    NamedDir13 = "named_dir1/3"
    NamedDir2 = "named_dir2"
    NamedDir21 = "named_dir2/1"
    NamedDir22 = "named_dir2/2"
    NamedDir23 = "named_dir2/3"


class ConfigFile(str, Enum):
    Uuid = "uuid"
    Config = "config.toml"


class DataFile(str, Enum):
    pass


class HoardTester(ABC):
    env = {}
    force = False
    targets = []

    @classmethod
    def sync(cls):
        if platform.system() != 'Windows':
            os.sync()

    @classmethod
    def config_file_path(cls):
        system = platform.system()
        home = Path.home()
        if system == 'Linux':
            return home.joinpath(".config", "hoard", "config.toml")
        elif system == 'Darwin':
            return home.joinpath("Library", "Application Support", "com.shadow53.hoard", "config.toml")
        elif system == 'Windows':
            return home.joinpath("AppData", "Roaming", "shadow53", "hoard", "config", "config.toml")
        else:
            raise OSError("could not determine system for CI tests")

    @classmethod
    def data_dir_path(cls):
        system = platform.system()
        home = Path.home()
        if system == 'Linux':
            return home.joinpath(".local", "share", "hoard")
        elif system == 'Darwin':
            return home.joinpath("Library", "Application Support", "com.shadow53.hoard")
        elif system == 'Windows':
            return home.joinpath("AppData", "Roaming", "shadow53", "hoard", "data")
        else:
            raise OSError("could not determine system for CI tests")

    @classmethod
    def generate_file(cls, path, size=1024):
        path = Path(path)
        try:
            os.remove(path)
        except (FileNotFoundError, NotADirectoryError):
            pass

        os.makedirs(path.parent, exist_ok=True)

        with open(path, "wb") as file:
            content = secrets.token_bytes(size)
            file.write(content)

    @classmethod
    def reset(cls):
        home = Path.home()

        try:
            shutil.rmtree(cls.data_dir_path())
        except FileNotFoundError:
            pass

        config_parent = cls.config_file_path().parent
        try:
            shutil.rmtree(config_parent)
        except FileNotFoundError:
            pass

        for env in list(Environment):
            for item in list(HoardFile):
                if item is HoardFile.AnonDir or item is HoardFile.NamedDir1 or item is HoardFile.NamedDir2:
                    continue
                path = home.joinpath(f"{env}_{item}")
                cls.generate_file(path)
        os.makedirs(config_parent, exist_ok=True)
        config_file_src = Path.cwd().joinpath("ci-tests", "config.toml")
        shutil.copy2(config_file_src, cls.config_file_path())
        assert cls.config_file_path().is_file()

    @classmethod
    def assert_same_tree(cls, root1, root2, *, extra_files=None):
        if Path(root1).is_file():
            if not filecmp.cmp(root1, root2, shallow=False):
                raise RuntimeError(f"content of files {root1} and {root2} differ")
        elif Path(root1).is_dir():
            direntries = ["1", "2", "3"]
            if extra_files is not None:
                direntries.extend(extra_files)

            matches, mismatches, errors = filecmp.cmpfiles(
                root1, root2, direntries, shallow=False
            )
            if errors:
                raise RuntimeError(
                    f"could not check {errors} inside {root1} and/or {root2}"
                )
            if mismatches:
                raise RuntimeError(
                    f"contents of files {mismatches} in {root1} and {root2} differ"
                )

    @classmethod
    def assert_first_tree(cls):
        home = Path.home()
        data_dir = cls.data_dir_path()
        cls.assert_same_tree(
            home.joinpath(f"{Environment.First}_{HoardFile.AnonDir}"),
            data_dir.joinpath(HOARDS_DIRNAME, HoardFile.AnonDir.value),
        )
        cls.assert_same_tree(
            home.joinpath(f"{Environment.First}_{HoardFile.AnonFile}"),
            data_dir.joinpath(HOARDS_DIRNAME, HoardFile.AnonFile.value),
        )
        cls.assert_same_tree(
            home.joinpath(f"{Environment.First}_{HoardFile.NamedDir1}"),
            data_dir.joinpath(HOARDS_DIRNAME, "named", "dir1"),
        )
        cls.assert_same_tree(
            home.joinpath(f"{Environment.First}_{HoardFile.NamedDir2}"),
            data_dir.joinpath(HOARDS_DIRNAME, "named", "dir2"),
        )
        cls.assert_same_tree(
            home.joinpath(f"{Environment.First}_{HoardFile.NamedFile}"),
            data_dir.joinpath(HOARDS_DIRNAME, "named", "file"),
        )

    @classmethod
    def assert_second_tree(cls):
        home = Path.home()
        data_dir = cls.data_dir_path()
        cls.assert_same_tree(
            home.joinpath(f"{Environment.Second}_{HoardFile.AnonDir}"),
            data_dir.joinpath(HOARDS_DIRNAME, HoardFile.AnonDir.value),
        )
        cls.assert_same_tree(
            home.joinpath(f"{Environment.Second}_{HoardFile.AnonFile}"),
            data_dir.joinpath(HOARDS_DIRNAME, HoardFile.AnonFile.value),
        )
        cls.assert_same_tree(
            home.joinpath(f"{Environment.Second}_{HoardFile.NamedDir1}"),
            data_dir.joinpath(HOARDS_DIRNAME, "named", "dir1"),
        )
        cls.assert_same_tree(
            home.joinpath(f"{Environment.Second}_{HoardFile.NamedDir2}"),
            data_dir.joinpath(HOARDS_DIRNAME, "named", "dir2"),
        )
        cls.assert_same_tree(
            home.joinpath(f"{Environment.Second}_{HoardFile.NamedFile}"),
            data_dir.joinpath(HOARDS_DIRNAME, "named", "file"),
        )

    def run_hoard(self, command):
        # Run the specified hoard command
        # Should automatically operate on all hoards when targets is empty
        for key, val in self.env.items():
            os.environ[key] = val

        args = ["target/debug/hoard"]
        if self.force:
            args.append("--force")
        args.append(command)
        args += self.targets

        subprocess.run(args, check=True)
        sys.stdout.flush()

    @classmethod
    def _read_file(cls, path, *, is_binary=True):
        access = "r"
        if is_binary:
            access += "b"
        with open(path, access) as file:
            return file.read()

    @classmethod
    def _write_file(cls, path, content, *, is_binary=True):
        access = "w"
        if is_binary:
            access += "b"
        with open(path, access) as file:
            file.write(content)
            file.flush()
            os.fsync(file.fileno())
        cls.sync()
        time.sleep(2)
        cls.sync()

    @classmethod
    def read_hoard_file(cls, env, file):
        path = Path.home().joinpath(f"{env.value}_{file.value}")
        return cls._read_file(path, is_binary=True)

    @classmethod
    def write_hoard_file(cls, env, file, content):
        path = Path.home().joinpath(f"{env.value}_{file.value}")
        return cls._write_file(path, content, is_binary=True)

    @classmethod
    def get_uuid_path(cls):
        return cls.config_file_path().parent.joinpath(ConfigFile.Uuid.value)

    @property
    def uuid(self):
        return self._read_file(self.get_uuid_path(), is_binary=False)

    @uuid.setter
    def uuid(self, uuid):
        return self._write_file(self.get_uuid_path(), uuid, is_binary=False)

    @abstractmethod
    def run_test(self):
        pass
