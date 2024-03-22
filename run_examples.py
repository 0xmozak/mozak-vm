from colorama import init as colorama_init
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
        dirs = [dir for dir in os.listdir(directory) if (dir not in skip_directories and os.path.isdir(os.path.join(directory, dir)))]
        return dirs
    except OSError as e:
        raise OSError(f"Error while listing directory: {e}") from e

# Reads a `Cargo.toml` file and analyses whether the dependency on
# `mozak-sdk` is only on "core" features.
def is_sdk_dependency_only_on_core_features(cargo_file: str) -> bool:
    sdk_dependency = read_toml_file(cargo_file)["dependencies"]["mozak-sdk"]
    if 'default-features' in sdk_dependency:
        return sdk_dependency['default-features'] == False
    elif 'features' in sdk_dependency:
        return len(sdk_dependency['features']) == 0


MOZAK_CLI_LOCATION = "target/release/mozak-cli"

def build_mozak_cli():
    if os.path.exists(MOZAK_CLI_LOCATION):
        print(f"Found {Style.BRIGHT}{Fore.BLUE}mozak-cli{Style.RESET_ALL}: skipping build")
        return
    
    mozak_cli_build_command = f"cargo build --release --bin mozak-cli"
    print(f"Building {Style.BRIGHT}{Fore.BLUE}mozak-cli{Style.RESET_ALL}: {Fore.BLUE}{mozak_cli_build_command}{Style.RESET_ALL}")
    os.system(mozak_cli_build_command)

    if not os.path.exists(MOZAK_CLI_LOCATION):
        raise Exception("cannot build mozak-cli")

class ExamplesTester(unittest.TestCase):
    def test_workspace_members(self):
        actual_directories = set(list_directories("examples"))
        listed_workspace_members = set(read_toml_file("examples/Cargo.toml")['workspace']['members'])
        self.assertEqual(actual_directories, listed_workspace_members)
    
    def test_core_only_examples(self):
        build_mozak_cli()
        prove_and_verify_exceptions = {"panic"}  # TODO: check why `panic` doesn't work

        for dir in set(list_directories("examples")):
            if is_sdk_dependency_only_on_core_features(f"examples/{dir}/Cargo.toml"):
                print(f"{Style.BRIGHT}{Fore.BLUE}{dir}{Style.RESET_ALL} is detected core-only example")
                
                build_command = f"cd examples && cargo build --release --bin {dir}"
                print(f"Testing build: {Fore.BLUE}{build_command}{Style.RESET_ALL}")
                self.assertEqual(os.system(build_command), 0)
                
                if dir in prove_and_verify_exceptions:
                    print(f"{Fore.RED}ZK prove and verify skipping for {Style.BRIGHT}{dir}{Style.NORMAL} as it is marked as an exception{Style.RESET_ALL}")
                else:
                    prove_and_verify_command = f'''{MOZAK_CLI_LOCATION} prove-and-verify \
                        examples/target/riscv32im-mozak-mozakvm-elf/release/{dir} \
                        --self-prog-id MZK-0000000000000000000000000000000000000000000000000000000000000001;'''
                    print(f"ZK prove and verify: {Fore.BLUE}{prove_and_verify_command}{Style.RESET_ALL}")
                    self.assertEqual(os.system(prove_and_verify_command), 0)

                print("\n")

if __name__ == "__main__":
    unittest.main()
