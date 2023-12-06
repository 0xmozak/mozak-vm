import json
from typing import Dict, TypedDict
from path import CONFIG_JSON


class BenchWithCommit(TypedDict):
    commit: str
    bench_function: str


class Config:
    def __init__(self):
        config_file_path = CONFIG_JSON
        with open(config_file_path, "r") as f:
            config = json.load(f)
        self.config = config

    def get_benches_with_commit(self, bench_name: str) -> Dict[str, BenchWithCommit]:
        return self.config[bench_name]["benches"]

    def get_elf(self, bench_name: str, bench_description: str) -> str | None:
        return self.config[bench_name]["benches"][bench_description].get("elf")

    def get_parameter_name(self, bench_name: str) -> str:
        return self.config[bench_name]["parameter"]

    def get_output_name(self, bench_name: str) -> str:
        return self.config[bench_name]["output"]

    def get_description(self, bench_name: str) -> str:
        return self.config[bench_name]["description"]
