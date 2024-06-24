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
            if os.path.exists(os.path.join(directory, dir, "mozakvm", "Cargo.toml"))
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


def has_no_native_target(example_dir: str) -> bool:
    """Checks if the example directory doesn't have native directory inside.
    So we also require that the crate shouldn't have sdk dependency beyond core features
    """
    return (
        "native" not in os.listdir(example_dir) and
        not has_sdk_dependency_beyond_core_features(f"{example_dir}/mozakvm/Cargo.toml")
    )



class ExamplesTester(unittest.TestCase):
    """Test class for running examples"""

    def test_core_only_examples(self):
        """This test runs examples that depend on just the core
        capabilities of the `sdk` i.e. alloc, heap, panic etc
        """
        prove_and_verify_exceptions = {"panic"}  # TODO: check why `panic` doesn't work

        for example in set(list_cargo_projects("examples")):
            if has_no_native_target(
                f"examples/{example}"
            ):
                print(
                    f"{Style.BRIGHT}{Fore.BLUE}{example}{Style.RESET_ALL} is detected core-only example"
                )

                build_command = "cargo mozakvm-build"
                print(f"Testing build: {Fore.BLUE}{build_command}{Style.RESET_ALL}")

                subprocess.run(
                    args=shlex.split(build_command),
                    cwd=os.path.join("examples", example, "mozakvm"),
                    capture_output=capture_output,
                    timeout=timeout,
                    env=os_environ,
                    check=True,
                )

                if example in prove_and_verify_exceptions:
                    print(
                        f"{Fore.RED}ZK prove and verify skipping for {Style.BRIGHT}{example}{Style.NORMAL} as it is marked as an exception{Style.RESET_ALL}"
                    )
                else:
                    prove_and_verify_command = f"""cargo run --features=parallel --bin mozak-cli -- prove-and-verify -vvv \
                        examples/{example}/mozakvm/target/riscv32im-mozak-mozakvm-elf/mozak-release/{example}-mozakvm \
                        """
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

        for example in set(list_cargo_projects("examples")):
            if not has_no_native_target(f"examples/{example}"):
                print(
                    f"{Style.BRIGHT}{Fore.BLUE}{example}{Style.RESET_ALL} is detected fully-featured example, building",
                )

                build_command = "cargo mozakvm-build --features=\"std\""
                print(
                    f"Testing build: {Fore.BLUE}{build_command}{Style.RESET_ALL}",
                )

                subprocess.run(
                    args=shlex.split(build_command),
                    cwd=os.path.join("examples", example, "mozakvm"),
                    capture_output=capture_output,
                    timeout=timeout,
                    env=os_environ,
                    check=True,
                )
                print()

        for example in set(list_cargo_projects("examples")):
            if not has_no_native_target(f"examples/{example}"):
                print(
                    f"{Style.BRIGHT}{Fore.BLUE}{example}{Style.RESET_ALL} is detected fully-featured example, ZK prove and verify",
                )

                if example in prove_and_verify_exceptions:
                    print(
                        f"{Fore.RED}ZK prove and verify skipping for {Style.BRIGHT}{example}{Style.NORMAL} as it is marked as an exception{Style.RESET_ALL}",
                    )
                else:
                    # Unlike core-only examples, fully featured examples make use of
                    # cross-program-calls and system-tapes. Hence, they don't only need
                    # to test their own prove-and-verify, but need to ensure that other
                    # programs also prove-and-verify for system-tape they generated. All
                    # dependent programs to be tested are supposed to be listed in
                    # `package.metadata.mozak.example_dependents` in respective `Cargo.toml` (the dependent's
                    # `Cargo.toml` is not read for recursive expansion).
                    extra_info = read_toml_file(f"examples/{example}/mozakvm/Cargo.toml")[
                        "package"
                    ]["metadata"]["mozak"]
                    dependents = []
                    if "example_dependents" in extra_info.keys():
                        dependents = extra_info["example_dependents"]

                    system_tape_generation_command = """cargo run --release"""
                    print(
                        f"System tape generation: {Fore.BLUE}{system_tape_generation_command}{Style.RESET_ALL}",
                    )

                    subprocess.run(
                        args=shlex.split(system_tape_generation_command),
                        cwd=f"examples/{example}/native",
                        capture_output=capture_output,
                        timeout=timeout,
                        env=os_environ,
                        check=True,
                    )

                    print()

                    system_tape = f"examples/{example}/native/out/tape.json"

                    programs_to_run = [    
                            f"examples/{example}/mozakvm/target/riscv32im-mozak-mozakvm-elf/mozak-release/{example}-mozakvm",  
                    ]

                    for dependent in dependents:
                        programs_to_run.append(    
                                f"examples/{dependent}/mozakvm/target/riscv32im-mozak-mozakvm-elf/mozak-release/{dependent}-mozakvm"
                        )

                    for elf in programs_to_run:
                        print(
                            f"ZK prove and verify for {Style.BRIGHT}{Fore.BLUE}{example}{Style.RESET_ALL} requires execution of {elf} ",
                        )
                        execution_command = f"""cargo run --features=parallel --bin mozak-cli -- prove-and-verify -vvv {elf} --system-tape {system_tape} """
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
