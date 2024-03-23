# TODO: set up formatting and linting for Python files in CI.
from colorama import Fore
from colorama import Style
import os
import unittest
import toml


# Reads a toml file and returns
def read_toml_file(file_path: str):
    try:
        with open(file_path, "r") as f:
            data = toml.load(f)
            return data
    except FileNotFoundError as e:
        raise FileNotFoundError(f"Error: File '{file_path}' not found.") from e
    except Exception as e:
        raise Exception(f"Error reading TOML file: {e}") from e


# Lists the different sub-directories (one level) at a given root directory
def list_directories(directory: str):
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


# Reads a `Cargo.toml` file and analyses whether the dependency on
# `mozak-sdk` is only on "core" features.
def is_sdk_dependency_beyond_core_features(cargo_file: str) -> bool:
    sdk_dependency = read_toml_file(cargo_file)["dependencies"]["mozak-sdk"]
    if "default-features" in sdk_dependency:
        return sdk_dependency["default-features"]
    elif "features" in sdk_dependency:
        return len(sdk_dependency["features"]) > 0
    else:
        return True


MOZAK_CLI_LOCATION = "target/release/mozak-cli"


def build_mozak_cli():
    if os.path.exists(MOZAK_CLI_LOCATION):
        print(
            f"Found {Style.BRIGHT}{Fore.BLUE}mozak-cli{Style.RESET_ALL}: skipping build",
            flush=True,
        )
        return

    mozak_cli_build_command = f"cargo build --release --bin mozak-cli"
    print(
        f"Building {Style.BRIGHT}{Fore.BLUE}mozak-cli{Style.RESET_ALL}: {Fore.BLUE}{mozak_cli_build_command}{Style.RESET_ALL}",
        flush=True,
    )
    os.system(mozak_cli_build_command)

    if not os.path.exists(MOZAK_CLI_LOCATION):
        raise Exception("cannot build mozak-cli")


class ExamplesTester(unittest.TestCase):
    """
    This test ensures that all the workspace members are accounted
    for and no dangling examples directory exists.
    """

    def test_workspace_members(self):
        actual_directories = set(list_directories("examples"))
        listed_workspace_members = set(
            read_toml_file("examples/Cargo.toml")["workspace"]["members"]
        )
        self.assertEqual(actual_directories, listed_workspace_members)

    """
    This test runs examples that depend on just the core
    capabilities of the `sdk` i.e. alloc, heap, panic etc
    """

    def test_core_only_examples(self):
        build_mozak_cli()
        prove_and_verify_exceptions = {"panic"}  # TODO: check why `panic` doesn't work
        dummy_prog_id = (
            "MZK-0000000000000000000000000000000000000000000000000000000000000001"
        )

        for dir in set(list_directories("examples")):
            if not is_sdk_dependency_beyond_core_features(f"examples/{dir}/Cargo.toml"):
                print(
                    f"{Style.BRIGHT}{Fore.BLUE}{dir}{Style.RESET_ALL} is detected core-only example"
                )

                build_command = f"cd examples && cargo build --release --bin {dir}"
                print(f"Testing build: {Fore.BLUE}{build_command}{Style.RESET_ALL}")
                self.assertEqual(os.system(build_command), 0)

                if dir in prove_and_verify_exceptions:
                    print(
                        f"{Fore.RED}ZK prove and verify skipping for {Style.BRIGHT}{dir}{Style.NORMAL} as it is marked as an exception{Style.RESET_ALL}"
                    )
                else:
                    prove_and_verify_command = f"""{MOZAK_CLI_LOCATION} prove-and-verify \
                        examples/target/riscv32im-mozak-mozakvm-elf/release/{dir} \
                        --self-prog-id {dummy_prog_id}"""
                    print(
                        f"ZK prove and verify: {Fore.BLUE}{prove_and_verify_command}{Style.RESET_ALL}"
                    )
                    self.assertEqual(os.system(prove_and_verify_command), 0)

                print("\n")

    """
    This test runs examples that depend on more than just the core
    capabilities of the `sdk` i.e. make use of types, traits, system
    tape etc
    """

    def test_full_featured_examples(self):
        build_mozak_cli()
        prove_and_verify_exceptions = {}

        for dir in set(list_directories("examples")):
            if is_sdk_dependency_beyond_core_features(
                f"examples/{dir}/Cargo.toml"
            ):
                print(
                    f"{Style.BRIGHT}{Fore.BLUE}{dir}{Style.RESET_ALL} is detected fully-featured example, building",
                    flush=True,
                )

                build_command = f"cd examples && cargo build --release --bin {dir}bin"
                print(
                    f"Testing build: {Fore.BLUE}{build_command}{Style.RESET_ALL}",
                    flush=True,
                )
                self.assertEqual(os.system(build_command), 0)
                print("\n")

        for dir in set(list_directories("examples")):
            if is_sdk_dependency_beyond_core_features(
                f"examples/{dir}/Cargo.toml"
            ):
                print(
                    f"{Style.BRIGHT}{Fore.BLUE}{dir}{Style.RESET_ALL} is detected fully-featured example, ZK prove and verify",
                    flush=True,
                )

                if dir in prove_and_verify_exceptions:
                    print(
                        f"{Fore.RED}ZK prove and verify skipping for {Style.BRIGHT}{dir}{Style.NORMAL} as it is marked as an exception{Style.RESET_ALL}",
                        flush=True,
                    )
                else:
                    """
                    Unlike core-only examples, fully featured examples make use of
                    cross-program-calls and system-tapes. Hence, they don't only need
                    to test their own prove-and-verify, but need to ensure that other
                    programs also prove-and-verify for system-tape they generated. All
                    dependent programs to be tested are supposed to be listed in
                    `extrainfo.example_dependents` in respective `Cargo.toml` (the dependent's
                    `Cargo.toml` is not read for recursive expansion).
                    """
                    extra_info = read_toml_file(f"examples/{dir}/Cargo.toml")[
                        "extrainfo"
                    ]
                    dependents = extra_info["example_dependents"]
                    prog_id = extra_info[
                        "example_program_id"
                    ]  # We assume this to be different from all dependents

                    system_tape_generation_command = f"""ARCH_TRIPLE="$(rustc --verbose --version | grep host | awk '{{ print $2; }}')"; cd examples && cargo run --release --features="native" --bin {dir}-native --target $ARCH_TRIPLE"""
                    print(
                        f"System tape generation: {Fore.BLUE}{system_tape_generation_command}{Style.RESET_ALL}",
                        flush=True,
                    )
                    self.assertEqual(os.system(system_tape_generation_command), 0)
                    print("\n")

                    system_tape = f"examples/{dir}.tape.json"

                    programs_to_run = [
                        (
                            f"examples/target/riscv32im-mozak-mozakvm-elf/release/{dir}bin",
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
                            f"ZK prove and verify for {Style.BRIGHT}{Fore.BLUE}{dir}{Style.RESET_ALL} requires execution of {elf} with ID: {id_}",
                            flush=True,
                        )
                        execution_command = f"""{MOZAK_CLI_LOCATION} prove-and-verify -vvv {elf} --system-tape {system_tape} --self-prog-id {id_}"""
                        print(
                            f"ZK prove and verify (sub-proof): {Fore.BLUE}{execution_command}{Style.RESET_ALL}",
                            flush=True,
                        )
                        self.assertEqual(os.system(system_tape_generation_command), 0)

                print("\n")


if __name__ == "__main__":
    unittest.main()
