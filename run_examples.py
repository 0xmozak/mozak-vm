"""
Program to run our examples

It not only tests the simple examples (which just used alloc etc) but also
tests for examples on cross-program-calls among other things.
"""

# TODO: set up formatting and linting for Python files in CI.
import os
import unittest

import toml
from colorama import Fore, Style


class ReadTomlError(Exception):
    """Error while reading TOML file."""


def read_toml_file(file_path: str):
    """Reads a toml file and returns the parsed content."""
    try:
        with open(file_path, "r", encoding="utf-8") as f:
            return toml.load(f)
    except FileNotFoundError as e:
        raise FileNotFoundError(f"Error: File '{file_path}' not found.") from e
    except Exception as e:
        raise ReadTomlError(f"Error reading TOML file: {e}") from e


def list_directories(directory: str):
    """Lists the different sub-directories (one level) at a given root directory"""
    skip_directories = {".cargo", "target"}
    try:
        dirs = [
            dir
            for dir in os.listdir(directory)
            if (
                dir not in skip_directories
                and os.path.isdir(os.path.join(directory, dir))
            )
        ]
        return dirs
    except OSError as e:
        raise OSError(f"Error while listing directory: {e}") from e


def is_sdk_dependency_beyond_core_features(cargo_file: str) -> bool:
    """Reads a `Cargo.toml` file and analyses whether the dependency on
    `mozak-sdk` is only on "core" features."""
    sdk_dependency = read_toml_file(cargo_file)["dependencies"]["mozak-sdk"]
    if "default-features" in sdk_dependency:
        return sdk_dependency["default-features"]
    elif "features" in sdk_dependency:
        return len(sdk_dependency["features"]) > 0
    else:
        return True

class ExamplesTester(unittest.TestCase):
    """Test class for running examples"""

    def test_workspace_members(self):
        """This test ensures that all the workspace members are accounted
        for and no dangling examples directory exists.
        """
        actual_directories = set(list_directories("examples"))
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

        for folder in set(list_directories("examples")):
            if not is_sdk_dependency_beyond_core_features(
                f"examples/{folder}/Cargo.toml"
            ):
                print(
                    f"{Style.BRIGHT}{Fore.BLUE}{folder}{Style.RESET_ALL} is detected core-only example"
                )

                build_command = f"cd examples && cargo build --release --bin {folder}"
                print(f"Testing build: {Fore.BLUE}{build_command}{Style.RESET_ALL}")
                self.assertEqual(os.system(build_command), 0)

                if folder in prove_and_verify_exceptions:
                    print(
                        f"{Fore.RED}ZK prove and verify skipping for {Style.BRIGHT}{folder}{Style.NORMAL} as it is marked as an exception{Style.RESET_ALL}"
                    )
                else:
                    prove_and_verify_command = f"""cargo run --bin mozak-cli -- prove-and-verify \
                        examples/target/riscv32im-mozak-mozakvm-elf/release/{folder} \
                        --self-prog-id {dummy_prog_id}"""
                    print(
                        f"ZK prove and verify: {Fore.BLUE}{prove_and_verify_command}{Style.RESET_ALL}"
                    )
                    self.assertEqual(os.system(prove_and_verify_command), 0)

                print("\n")

    def test_full_featured_examples(self):
        """This test runs examples that depend on more than just the core
        capabilities of the `sdk` i.e. make use of types, traits, system
        tape etc
        """
        prove_and_verify_exceptions = {}

        for folder in set(list_directories("examples")):
            if is_sdk_dependency_beyond_core_features(f"examples/{folder}/Cargo.toml"):
                print(
                    f"{Style.BRIGHT}{Fore.BLUE}{folder}{Style.RESET_ALL} is detected fully-featured example, building",
                )

                build_command = (
                    f"cd examples && cargo build --release --bin {folder}bin"
                )
                print(
                    f"Testing build: {Fore.BLUE}{build_command}{Style.RESET_ALL}",
                )
                self.assertEqual(os.system(build_command), 0)
                print("\n")

        for folder in set(list_directories("examples")):
            if is_sdk_dependency_beyond_core_features(f"examples/{folder}/Cargo.toml"):
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
                    # `extrainfo.example_dependents` in respective `Cargo.toml` (the dependent's
                    # `Cargo.toml` is not read for recursive expansion).
                    extra_info = read_toml_file(f"examples/{folder}/Cargo.toml")[
                        "extrainfo"
                    ]
                    dependents = extra_info["example_dependents"]
                    prog_id = extra_info[
                        "example_program_id"
                    ]  # We assume this to be different from all dependents

                    system_tape_generation_command = f"""
                        ARCH_TRIPLE="$(rustc --verbose --version | grep host | awk '{{ print $2; }}')";
                        cd examples && cargo run --release --features="native" --bin {folder}-native --target $ARCH_TRIPLE
                        """
                    print(
                        f"System tape generation: {Fore.BLUE}{system_tape_generation_command}{Style.RESET_ALL}",
                    )
                    self.assertEqual(os.system(system_tape_generation_command), 0)
                    print()

                    system_tape = f"examples/{folder}.tape.json"

                    programs_to_run = [
                        (
                            f"examples/target/riscv32im-mozak-mozakvm-elf/release/{folder}bin",
                            prog_id,
                        )
                    ]

                    for dependent in dependents:
                        dependent_prog_id = read_toml_file(
                            f"examples/{dependent}/Cargo.toml"
                        )["extrainfo"]["example_program_id"]
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
                        execution_command = f"""cargo run --bin mozak-cli -- prove-and-verify -vvv {elf} --system-tape {system_tape} --self-prog-id {id_}"""
                        print(
                            f"ZK prove and verify (sub-proof): {Fore.BLUE}{execution_command}{Style.RESET_ALL}",
                        )
                        self.assertEqual(os.system(system_tape_generation_command), 0)

                print("\n")


if __name__ == "__main__":
    unittest.main()
