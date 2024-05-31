"""
Program to run our examples

It not only tests the simple examples (which just used alloc etc) but also
tests for examples on cross-program-calls among other things.
"""

# TODO: set up formatting and linting for Python files in CI.
import os
import re
import shlex
import subprocess
import unittest

import toml
from colorama import Fore, Style

os_environ = os.environ

# Comment the following line if you do not want verbose output
os_environ["MOZAK_STARK_DEBUG"] = "true"
# Turn the following to `True` if you do not want output capturing
capture_output = False
# Running timeout per prove-and-verify (in seconds)
timeout = 600


class ReadTomlError(Exception):
    """Error while reading TOML file."""


def read_toml_file(file_path: str):
    """Reads a toml file and returns the parsed content."""
    try:
        with open(file_path, "r", encoding="utf-8") as f:
            return toml.load(f)
    except FileNotFoundError:
        raise
    except Exception as e:
        raise ReadTomlError(f"Error reading TOML file: {e}") from e


def list_cargo_projects(directory: str):
    """Lists all cargo projects down one level at a given root directory"""
    try:
        return [
            dir
            for dir in os.listdir(directory)
            if os.path.exists(os.path.join(directory, dir, "Cargo.toml"))
        ]
    except OSError as e:
        raise OSError(f"Error while listing directory: {e}") from e


def has_sdk_dependency_beyond_core_features(cargo_file: str) -> bool:
    """Reads a `Cargo.toml` file and analyses whether the dependency on
    `mozak-sdk` is only on "core" features."""
    sdk_dependency = read_toml_file(cargo_file)["dependencies"]["mozak-sdk"]
    return (
        sdk_dependency.get("default-features", True)
        or len(sdk_dependency.get("features", [])) > 0
    )


class ExamplesTester(unittest.TestCase):
    """Test class for running examples"""

    def test_workspace_members(self):
        """This test ensures that all the workspace members are accounted
        for and no dangling examples directory exists.
        """
        actual_directories = set(list_cargo_projects("examples"))
        listed_workspace_members = set(
            read_toml_file("examples/Cargo.toml")["workspace"]["members"]
        )
        self.assertEqual(actual_directories, listed_workspace_members)

    def test_core_only_examples(self):
        """This test runs examples that depend on just the core
        capabilities of the `sdk` i.e. alloc, heap, panic etc
        """
        prove_and_verify_exceptions = {"panic"}  # TODO: check why `panic` doesn't work
        dummy_prog_id = (
            "MZK-0000000000000000000000000000000000000000000000000000000000000001"
        )

        for folder in set(list_cargo_projects("examples")):
            if not has_sdk_dependency_beyond_core_features(
                f"examples/{folder}/Cargo.toml"
            ):
                print(
                    f"{Style.BRIGHT}{Fore.BLUE}{folder}{Style.RESET_ALL} is detected core-only example"
                )

                build_command = f"cargo build --release --bin {folder}"
                print(f"Testing build: {Fore.BLUE}{build_command}{Style.RESET_ALL}")

                subprocess.run(
                    args=shlex.split(build_command),
                    cwd="examples",
                    capture_output=capture_output,
                    timeout=timeout,
                    env=os_environ,
                    check=True,
                )

                if folder in prove_and_verify_exceptions:
                    print(
                        f"{Fore.RED}ZK prove and verify skipping for {Style.BRIGHT}{folder}{Style.NORMAL} as it is marked as an exception{Style.RESET_ALL}"
                    )
                else:
                    prove_and_verify_command = f"""cargo run --features=parallel --bin mozak-cli -- prove-and-verify -vvv \
                        examples/target/riscv32im-mozak-mozakvm-elf/release/{folder} \
                        --self-prog-id {dummy_prog_id}"""
                    print(
                        f"ZK prove and verify: {Fore.BLUE}{prove_and_verify_command}{Style.RESET_ALL}"
                    )

                    subprocess.run(
                        args=shlex.split(prove_and_verify_command),
                        capture_output=capture_output,
                        timeout=timeout,
                        env=os_environ,
                        check=True,
                    )
                print()

    def test_full_featured_examples(self):
        """This test runs examples that depend on more than just the core
        capabilities of the `sdk` i.e. make use of types, traits, system
        tape etc
        """
        prove_and_verify_exceptions = {}

        arch_triple = re.search(
            r"host: (.*)",
            subprocess.check_output(["rustc", "--verbose", "--version"], text=True),
        ).group(1)
        print(
            f"{Style.BRIGHT}{Fore.GREEN}Detected arch triple for host{Style.RESET_ALL}: {arch_triple}",
        )

        for folder in set(list_cargo_projects("examples")):
            if has_sdk_dependency_beyond_core_features(f"examples/{folder}/Cargo.toml"):
                print(
                    f"{Style.BRIGHT}{Fore.BLUE}{folder}{Style.RESET_ALL} is detected fully-featured example, building",
                )

                build_command = (
                    f'cargo build --release --bin {folder}bin'
                )
                print(
                    f"Testing build: {Fore.BLUE}{build_command}{Style.RESET_ALL}",
                )

                subprocess.run(
                    args=shlex.split(build_command),
                    cwd="examples",
                    capture_output=capture_output,
                    timeout=timeout,
                    env=os_environ,
                    check=True,
                )
                print()

        for folder in set(list_cargo_projects("examples")):
            if has_sdk_dependency_beyond_core_features(f"examples/{folder}/Cargo.toml"):
                print(
                    f"{Style.BRIGHT}{Fore.BLUE}{folder}{Style.RESET_ALL} is detected fully-featured example, ZK prove and verify",
                )

                if folder in prove_and_verify_exceptions:
                    print(
                        f"{Fore.RED}ZK prove and verify skipping for {Style.BRIGHT}{folder}{Style.NORMAL} as it is marked as an exception{Style.RESET_ALL}",
                    )
                else:
                    # Unlike core-only examples, fully featured examples make use of
                    # cross-program-calls and system-tapes. Hence, they don't only need
                    # to test their own prove-and-verify, but need to ensure that other
                    # programs also prove-and-verify for system-tape they generated. All
                    # dependent programs to be tested are supposed to be listed in
                    # `package.metadata.mozak.example_dependents` in respective `Cargo.toml` (the dependent's
                    # `Cargo.toml` is not read for recursive expansion).
                    extra_info = read_toml_file(f"examples/{folder}/Cargo.toml")[
                        "package"
                    ]["metadata"]["mozak"]
                    dependents = []
                    if "example_dependents" in extra_info.keys():
                        dependents = extra_info["example_dependents"]
                    # We assume this to be different from all dependents
                    prog_id = extra_info["example_program_id"]

                    system_tape_generation_command = f"""cargo run --release --features="native,std" --bin {folder}-native --target {arch_triple}"""
                    print(
                        f"System tape generation: {Fore.BLUE}{system_tape_generation_command}{Style.RESET_ALL}",
                    )

                    subprocess.run(
                        args=shlex.split(system_tape_generation_command),
                        cwd=f"examples/{folder}",
                        capture_output=capture_output,
                        timeout=timeout,
                        env=os_environ,
                        check=True,
                    )

                    print()

                    system_tape = f"examples/{folder}/out/tape.json"

                    programs_to_run = [
                        (
                            f"examples/target/riscv32im-mozak-mozakvm-elf/release/{folder}bin",
                            prog_id,
                        )
                    ]

                    for dependent in dependents:
                        dependent_prog_id = read_toml_file(
                            f"examples/{dependent}/Cargo.toml"
                        )["package"]["metadata"]["mozak"]["example_program_id"]
                        programs_to_run.append(
                            (
                                f"examples/target/riscv32im-mozak-mozakvm-elf/release/{dependent}bin",
                                dependent_prog_id,
                            )
                        )

                    for elf, id_ in programs_to_run:
                        print(
                            f"ZK prove and verify for {Style.BRIGHT}{Fore.BLUE}{folder}{Style.RESET_ALL} requires execution of {elf} with ID: {id_}",
                        )
                        execution_command = f"""cargo run --features=parallel --bin mozak-cli -- prove-and-verify -vvv {elf} --system-tape {system_tape} --self-prog-id {id_}"""
                        print(
                            f"ZK prove and verify (sub-proof): {Fore.BLUE}{execution_command}{Style.RESET_ALL}",
                        )

                        subprocess.run(
                            args=shlex.split(execution_command),
                            capture_output=capture_output,
                            timeout=timeout,
                            env=os_environ,
                            check=True,
                        )

                print()


if __name__ == "__main__":
    unittest.main()
