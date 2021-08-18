from pathlib import Path
import filecmp
import os
import platform
import secrets
import shutil
import subprocess
import sys

# Before continuing, ensure this is running on GitHub Actions
for var in ["CI", "GITHUB_ACTIONS"]:
    val = os.environ.get(var)
    if val is None or val != "true":
        raise RuntimeError("These tests must be run on GitHub Actions!")


def config_file_path():
    system = platform.system()
    home = Path.home()
    if system == 'Linux':
        return Path(f"{home}/.config/hoard/config.toml")
    elif system == 'Darwin':
        return Path(f"{home}/Library/Application Support/com.shadow53.hoard/config.toml")
    elif system == 'Windows':
        return Path(f"{home}/AppData/Roaming/shadow53/hoard/config/config.toml")
    else:
        raise OSError("could not determine system for CI tests")


def data_dir_path():
    system = platform.system()
    home = Path.home()
    if system == 'Linux':
        return Path(f"{home}/.local/share/hoard")
    elif system == 'Darwin':
        return Path(f"{home}/Library/Application Support/com.shadow53.hoard")
    elif system == 'Windows':
        return Path(f"{home}/AppData/Roaming/shadow53/hoard/data")
    else:
        raise OSError("could not determine system for CI tests")


def setup():
    home = Path.home()

    try:
        shutil.rmtree(data_dir_path())
    except FileNotFoundError:
        pass

    try:
        shutil.rmtree(config_file_path().parent)
    except FileNotFoundError:
        pass

    for env in ["first", "second"]:
        for item in ["anon_dir", "named_dir"]:
            for num in [1, 2, 3]:
                os.makedirs(f"{home}/{env}_{item}", exist_ok=True)
                with open(f"{home}/{env}_{item}/{num}", "wb") as file:
                    content = secrets.token_bytes(num * 1024)
                    file.write(content)
        for item in ["anon_file", "named_file"]:
            with open(f"{home}/{env}_{item}", "wb") as file:
                content = secrets.token_bytes(2048)
                file.write(content)
    os.makedirs(config_file_path().parent)
    shutil.copy2("ci-tests/config.toml", config_file_path())


def assert_same_tree(path1, path2, direntries=None):
    if direntries is None:
        if not filecmp.cmp(path1, path2, shallow=False):
            raise RuntimeError(f"content of files {path1} and {path2} differ")
    else:
        matches, mismatches, errors = filecmp.cmpfiles(path1, path2, direntries, shallow=False)
        if errors:
            raise RuntimeError(f"could not check {errors} inside {path1} and/or {path2}")
        if mismatches:
            raise RuntimeError(f"contents of files {mismatches} in {path1} and {path2} differ")


def assert_first_tree():
    home = Path.home()
    data_dir = data_dir_path()
    assert_same_tree(f"{home}/first_anon_dir", f"{data_dir}/hoards/anon_dir")
    assert_same_tree(f"{home}/first_anon_file", f"{data_dir}/hoards/anon_file")
    assert_same_tree(f"{home}/first_named_dir", f"{data_dir}/hoards/named/dir")
    assert_same_tree(f"{home}/first_named_file", f"{data_dir}/hoards/named/file")


def assert_second_tree():
    home = Path.home()
    data_dir = data_dir_path()
    assert_same_tree(f"{home}/second_anon_dir", f"{data_dir}/hoards/anon_dir")
    assert_same_tree(f"{home}/second_anon_file", f"{data_dir}/hoards/anon_file")
    assert_same_tree(f"{home}/second_named_dir", f"{data_dir}/hoards/named/dir")
    assert_same_tree(f"{home}/second_named_file", f"{data_dir}/hoards/named/file")


def run_hoard(command, force=False, targets=[], env=None):
    # Run the specified hoard command
    # Should automatically operate on all hoards when targets is empty
    for key, val in env.items():
        os.environ[key] = val
    if force:
        targets.insert(0, "--force")
    subprocess.run(["target/debug/hoard", command, *targets], check=True)


def test_last_paths():
    # Do setup
    setup()
    # Run hoard with env "first"
    run_hoard("backup", env={"USE_ENV": "1"})
    # Doing it again should still succeed
    run_hoard("backup", env={"USE_ENV": "1"})
    # Run hoard with env "second" - this should fail
    try:
        run_hoard("backup", env={"USE_ENV": "2"})
        raise RuntimeError("Changing environment should have caused last_paths to fail")
    except subprocess.CalledProcessError:
        pass
    # Doing it again with "first" should still succeed
    run_hoard("backup", env={"USE_ENV": "1"})
    # Make sure the files are consistent with backing up "first"
    assert_first_tree()
    # Doing it with "second" but forced should succeed
    run_hoard("backup", force=True, env={"USE_ENV": "2"})
    # Make sure the files were overwritten
    assert_second_tree()


if __name__ == "__main__":
    if len(sys.argv) == 1:
        raise RuntimeError("One argument - the test - is required")
    if sys.argv[1] == "last_paths":
        test_last_paths()
