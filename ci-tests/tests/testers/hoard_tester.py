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
    def __init__(self):
        self.env = {}
        self.force = False
        self.targets = []
        self.args = []

    @classmethod
    def sync(cls):
        if platform.system() != 'Windows':
            os.sync()

    @classmethod
    def flush(cls):
        sys.stdout.flush()
        sys.stderr.flush()


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

    def clean(self, config_file="config.toml"):
        try:
            shutil.rmtree(self.data_dir_path())
        except FileNotFoundError:
            pass

        config_parent = self.config_file_path().parent
        try:
            shutil.rmtree(config_parent)
        except FileNotFoundError:
            pass

        os.makedirs(config_parent, exist_ok=True)
        config_file_src = Path.cwd().joinpath("ci-tests", config_file)
        shutil.copy2(config_file_src, self.config_file_path())
        assert self.config_file_path().is_file()

    def setup(self):
        home = Path.home()
        for env in list(Environment):
            for item in list(HoardFile):
                path = home.joinpath(f"{env}_{item}")
                if item is HoardFile.AnonDir or item is HoardFile.NamedDir1 or item is HoardFile.NamedDir2:
                    try:
                        shutil.rmtree(path)
                    except FileNotFoundError:
                        pass
                    continue
                self.generate_file(path)

    def reset(self, config_file="config.toml"):
        self.clean(config_file)
        self.setup()

    @classmethod
    def assert_same_tree(cls, root1, root2):
        root1 = Path(root1)
        root2 = Path(root2)
        if root1.is_file():
            if not filecmp.cmp(root1, root2, shallow=False):
                raise RuntimeError(f"content of files {root1} and {root2} differ")
        elif root1.is_dir():
            comparison = filecmp.dircmp(root1, root2)
            if comparison.diff_files is not None and len(comparison.diff_files) > 0:
                raise AssertionError(f"files differ in {root1} and {root2}: {comparison.diff_files}")

            if comparison.left_only is not None and len(comparison.left_only) > 0:
                raise AssertionError(f"files only in {root1}: {comparison.left_only}")

            if comparison.right_only is not None and len(comparison.right_only) > 0:
                raise AssertionError(f"files only in {root2}: {comparison.right_only}")

            for subdir in comparison.common_dirs:
                cls.assert_same_tree(root1.joinpath(subdir), root2.joinpath(subdir))

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

    def _call_hoard(self, args, *, allow_failure, capture_output):
        return subprocess.run(args, check=(not allow_failure), capture_output=capture_output)

    def run_hoard(self, command, allow_failure=False, capture_output=False):
        # Run the specified hoard command
        # Should automatically operate on all hoards when targets is empty
        for key, val in self.env.items():
            if val is None:
                del os.environ[key]
            else:
                os.environ[key] = val

        args = ["target/debug/hoard"]
        if self.force:
            args.append("--force")
        args += self.args
        args.append(command)
        args += self.targets

        # Write Python buffered output before calling Hoard
        self.flush()
        try:
            result = self._call_hoard(args, allow_failure=allow_failure, capture_output=capture_output)
            if capture_output:
                sys.stdout.buffer.write(result.stdout)
                sys.stderr.buffer.write(result.stderr)
        except subprocess.CalledProcessError as e:
            if capture_output:
                sys.stdout.buffer.write(e.stdout)
                sys.stderr.buffer.write(e.stderr)
            raise
        finally:
            self.flush()
        return result

    @classmethod
    def _read_file(cls, path, *, is_binary=True):
        access = "r"
        if is_binary:
            access += "b"
        with open(path, access) as file:
            return file.read()

    @classmethod
    def _write_file(cls, path, content, *, is_binary=True):
        access = "w+"
        if is_binary:
            access += "b"
        path = Path(path)
        if not path.parent.exists():
            os.makedirs(path.parent)
        with open(path, access) as file:
            file.write(content)
            file.flush()
            os.fsync(file.fileno())
        cls.sync()
        #time.sleep(2)
        #cls.sync()

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
