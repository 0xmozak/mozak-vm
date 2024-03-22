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

class ExamplesTester(unittest.TestCase):
    def test_workspace_members(self):
        actual_directories = set(list_directories("examples"))
        listed_workspace_members = set(read_toml_file("examples/Cargo.toml")['workspace']['members'])
        self.assertEqual(actual_directories, listed_workspace_members)
    
    def test_core_only_examples(self):
        actual_directories = set(list_directories("examples"))
        for dir in actual_directories:
            if is_sdk_dependency_only_on_core_features(f"examples/{dir}/Cargo.toml"):
                print(f"{Style.BRIGHT}{Fore.GREEN}{dir}{Style.RESET_ALL} is detected core-only example; testing build")
                command = f"cd examples && cargo +nightly build --release --bin {dir}"
                print(f"Running: {Fore.BLUE}{command}{Style.RESET_ALL}")
                self.assertEqual(os.system(f"cd examples && cargo +nightly build --release --bin {dir}"), 0)
                print("\n")

if __name__ == "__main__":
    unittest.main()
