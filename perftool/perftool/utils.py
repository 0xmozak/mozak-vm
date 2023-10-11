import os
import re
import subprocess


def create_repo_from_commmit(commit: str, tmpfolder) -> None:
    subprocess.run(
        ["git", "worktree", "add", "-f", f"{tmpfolder}", f"{commit}"], check=True
    )
    return


def build_release(cli_repo: str) -> None:
    subprocess.run(["cargo", "build", "--release"], cwd=cli_repo, check=True)


def bench(bench_function: str, parameter: int, cli_repo: str) -> float:
    stdout = subprocess.check_output(
        args=["cargo", "run", "--release", "bench", bench_function, f"{parameter}"],
        cwd=cli_repo,
        stderr=subprocess.DEVNULL,
    )
    pattern = r"\d+\.\d+"
    time_taken = re.findall(pattern, stdout.decode())[0]
    return float(time_taken)
