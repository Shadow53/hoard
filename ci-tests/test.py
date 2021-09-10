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
        return home.joinpath(".config", "hoard", "config.toml")
    elif system == 'Darwin':
        return home.joinpath("Library", "Application Support", "com.shadow53.hoard", "config.toml")
    elif system == 'Windows':
        return home.joinpath("AppData", "Roaming", "shadow53", "hoard", "config", "config.toml")
    else:
        raise OSError("could not determine system for CI tests")


def data_dir_path():
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


def generate_file(path, size=1024):
    os.makedirs(path.parent, exist_ok=True)
    with open(path, "wb") as file:
        content = secrets.token_bytes(size)
        file.write(content)


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
        for item in ["anon_dir", "named_dir1", "named_dir2"]:
            for num in [1, 2, 3]:
                generate_file(home.joinpath(f"{env}_{item}", str(num)), 1024 * num)
        for item in ["anon_file", "named_file"]:
            generate_file(home.joinpath(f"{env}_{item}"))
    os.makedirs(config_file_path().parent)
    shutil.copy2(Path.cwd().joinpath("ci-tests", "config.toml"), config_file_path())


def assert_same_tree(path1, path2, *, direntries=None):
    if direntries is None:
        if not filecmp.cmp(path1, path2, shallow=False):
            raise RuntimeError(f"content of files {path1} and {path2} differ")
    else:
        matches, mismatches, errors = filecmp.cmpfiles(
            path1, path2, direntries, shallow=False
        )
        if errors:
            raise RuntimeError(
                f"could not check {errors} inside {path1} and/or {path2}"
            )
        if mismatches:
            raise RuntimeError(
                f"contents of files {mismatches} in {path1} and {path2} differ"
            )


def assert_first_tree():
    home = Path.home()
    data_dir = data_dir_path()
    assert_same_tree(
        home.joinpath("first_anon_dir"),
        data_dir.joinpath("hoards", "anon_dir"),
        direntries=["1", "2", "3"]
    )
    assert_same_tree(
        home.joinpath("first_anon_file"),
        data_dir.joinpath("hoards", "anon_file")
    )
    assert_same_tree(
        home.joinpath("first_named_dir1"),
        data_dir.joinpath("hoards", "named", "dir1"),
        direntries=["1", "2", "3"]
    )
    assert_same_tree(
        home.joinpath("first_named_dir2"),
        data_dir.joinpath("hoards", "named", "dir2"),
        direntries=["1", "2", "3"]
    )
    assert_same_tree(
        home.joinpath("first_named_file"),
        data_dir.joinpath("hoards", "named", "file")
    )


def assert_second_tree():
    home = Path.home()
    data_dir = data_dir_path()
    assert_same_tree(
        home.joinpath("second_anon_dir"),
        data_dir.joinpath("hoards", "anon_dir"),
        direntries=["1", "2", "3"]
    )
    assert_same_tree(
        home.joinpath("second_anon_file"),
        data_dir.joinpath("hoards", "anon_file")
    )
    assert_same_tree(
        home.joinpath("second_named_dir1"),
        data_dir.joinpath("hoards", "named", "dir1"),
        direntries=["1", "2", "3"]
    )
    assert_same_tree(
        home.joinpath("second_named_dir2"),
        data_dir.joinpath("hoards", "named", "dir2"),
        direntries=["1", "2", "3"]
    )
    assert_same_tree(
        home.joinpath("second_named_file"),
        data_dir.joinpath("hoards", "named", "file")
    )


def run_hoard(command, *, force=False, targets=[], env=None):
    # Run the specified hoard command
    # Should automatically operate on all hoards when targets is empty
    for key, val in env.items():
        os.environ[key] = val

    args = ["target/debug/hoard"]
    if force:
        args.append("--force")
    args.append(command)
    args += targets

    subprocess.run(args, check=True)


def test_last_paths_checker():
    # Do setup
    setup()
    # Run hoard with env "first"
    run_hoard("backup", env={"USE_ENV": "1"})
    # Doing it again should still succeed
    run_hoard("backup", env={"USE_ENV": "1"})
    # Run hoard with env "second" - this should fail
    try:
        run_hoard("backup", env={"USE_ENV": "2"})
        raise AssertionError(
            "Changing environment should have caused last_paths to fail"
        )
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


def test_operation_checker():
    # Do setup
    setup()
    # We are not changing env on this test
    env = {"USE_ENV": "1"}
    # Run hoard
    print("========= HOARD RUN #1 =========")
    run_hoard("backup",  env=env)
    # Read UUID and delete file
    uuid_path = config_file_path().parent.joinpath("uuid")
    with open(uuid_path, "r") as file:
        uuid = file.readline()
    os.remove(uuid_path)

    # Go again, this time with a different uuid
    # This should still succeed because the files have the same checksum
    print("========= HOARD RUN #2 =========")
    print("  After removing the UUID file  ")
    run_hoard("backup", env=env)

    # Modify a file and backup again so checksums are different the next time
    # This should succeed because this UUID had the last backup
    with open(Path.home().joinpath("first_anon_file"), "rb") as file:
        old_content = file.read()
    with open(Path.home().joinpath("first_anon_file"), "wb") as file:
        content = secrets.token_bytes(1024)
        file.write(content)
    print("========= HOARD RUN #3 =========")
    print(" After replacing a file content ")
    run_hoard("backup", env=env)

    # Swap UUIDs and change the file again and try to back up
    # Should fail because a different machine has the most recent backup
    with open(uuid_path, "w") as file:
        file.write(uuid)
    with open(Path.home().joinpath("first_anon_file"), "wb") as file:
        file.write(old_content)
    try:
        print("========= HOARD RUN #4 =========")
        print("   After using first UUID/File  ")
        run_hoard("backup", env=env)
        raise AssertionError("Using the first UUID should have failed")
    except subprocess.CalledProcessError:
        pass
    # Once more to verify it should always fail
    try:
        print("========= HOARD RUN #5 =========")
        print("    Doing it again to be sure   ")
        run_hoard("backup", env=env)
        raise AssertionError("Using the first UUID should have failed")
    except subprocess.CalledProcessError:
        pass
    # Do it again but forced, and it should succeed
    print("========= HOARD RUN #6 =========")
    print("    Doing it again to be sure   ")
    run_hoard("backup", env=env, force=True)


def test_ignore_filter():
    # Do setup
    setup()
    # We are not changing env on this test
    env = {"USE_ENV": "1"}
    # All three of global, hoard, and pile-ignore files should be ignored.
    global_file = "global_ignore"
    hoard_file = "ignore_for_hoard"
    pile_file = "spilem"

    # Create files to ignore
    anon_dir_root = Path.home().joinpath("first_anon_dir")
    named_dir1_root = Path.home().joinpath("first_named_dir1")
    named_dir2_root = Path.home().joinpath("first_named_dir2")

    for root in [anon_dir_root, named_dir1_root, named_dir2_root]:
        for file in [global_file, hoard_file, pile_file]:
            generate_file(root.joinpath(file))

    # Run hoard
    run_hoard("backup", env=env)

    # Delete unexpected files for assert_same_tree
    # Named dir1 pile should only ignore pile (overwrites global and hoard)
    os.remove(named_dir1_root.joinpath(pile_file))
    # Named dir2 pile should only ignore hoard (overwrites global)
    os.remove(named_dir2_root.joinpath(hoard_file))
    # Anon dir should only ignore global
    os.remove(anon_dir_root.joinpath(global_file))

    # Assert trees
    home = Path.home()
    data_dir = data_dir_path()
    assert_same_tree(
        home.joinpath("first_anon_dir"),
        data_dir.joinpath("hoards", "anon_dir"),
        direntries=["1", "2", "3", pile_file, hoard_file]
    )
    assert_same_tree(
        home.joinpath("first_named_dir1"),
        data_dir.joinpath("hoards", "named", "dir1"),
        direntries=["1", "2", "3", hoard_file, global_file]
    )
    assert_same_tree(
        home.joinpath("first_named_dir2"),
        data_dir.joinpath("hoards", "named", "dir2"),
        direntries=["1", "2", "3", pile_file, global_file]
    )


if __name__ == "__main__":
    if len(sys.argv) == 1:
        raise RuntimeError("One argument - the test - is required")
    try:
        if sys.argv[1] == "last_paths":
            print("Running last_paths test")
            test_last_paths_checker()
        elif sys.argv[1] == "operation":
            print("Running operation test")
            test_operation_checker()
        elif sys.argv[1] == "ignore":
            print("Running ignore filter test")
            test_ignore_filter()
        else:
            raise RuntimeError(f"Invalid argument {sys.argv[1]}")
    except Exception:
        print("\nHoards:")
        subprocess.run(["tree", str(data_dir_path())])
        print("\nHome:")
        subprocess.run(["tree", "-L", "4"])
        raise
