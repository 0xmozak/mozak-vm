from typing import Dict
import json

from path import CONFIG_JSON

global config
with open(CONFIG_JSON) as f:
    config = json.load(f)
    f.close()


def get_benches_with_commit(bench_name: str) -> Dict[str, Dict[str, str]]:
    return config[bench_name]["benches"]


def get_elf(bench_name: str, bench_description: str) -> str | None:
    return config[bench_name]["benches"][bench_description].get("elf")


def get_parameter_name(bench_name: str) -> str:
    return config[bench_name]["parameter"]


def get_output_name(bench_name: str) -> str:
    return config[bench_name]["output"]


def get_description(bench_name: str) -> str:
    return config[bench_name]["description"]
