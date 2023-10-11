import os
import subprocess


def create_repo_from_commmit(commit: str, tmpfolder) -> None:
    subprocess.run(["git", "worktree", "add", f"{tmpfolder}", f"{commit}"], check=True)
    return


def build_release(cli_repo: str) -> None:
    subprocess.run(["cargo", "build", "--release"], cwd=cli_repo, check=True)


def run_bench(bench_function: str, parameter: int, cli_repo: str) -> None:
    subprocess.run(
        ["cargo", "run", "--release", "bench", bench_function, f"{parameter}"],
        cwd=cli_repo,
        check=True,
    )
