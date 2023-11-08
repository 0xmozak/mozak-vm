from pathlib import Path
import shutil
import tempfile

BUILD_FOLDER = Path.cwd() / "build"
DATA_FOLDER = Path.cwd() / "data"
TMPFOLDERNAME = "Perftool_Repos_tmp"
TMPFOLDER = Path(tempfile.gettempdir()) / TMPFOLDERNAME
CONFIG_JSON = Path.cwd() / "config.json"
PLOT_FOLDER = Path.cwd() / "plots"


def get_config_json() -> Path:
    return CONFIG_JSON


def get_plot_svg_file(bench_function: str) -> Path:
    return PLOT_FOLDER / f"{bench_function}.svg"


def get_bench_folder(bench_function: str) -> Path:
    return BUILD_FOLDER / bench_function


def get_data_folder(bench_function: str) -> Path:
    return DATA_FOLDER / bench_function


def get_data_csv_file(bench_function: str, commit: str) -> Path:
    return get_data_folder(bench_function) / f"{commit}.csv"


def get_commit_symlink(bench_function: str, commit: str) -> Path:
    return BUILD_FOLDER / bench_function / commit


def get_tmp_folder() -> Path:
    TMPFOLDER.mkdir(exist_ok=True)
    return TMPFOLDER


def get_commit_folder(bench_function: str, commit: str) -> Path:
    try:
        return get_commit_symlink(bench_function, commit).resolve()
    except FileNotFoundError as e:
        raise e


def get_actual_commit_folder(commit: str) -> Path:
    return TMPFOLDER / commit


def get_cli_repo(bench_function: str, commit: str) -> Path:
    return get_commit_folder(bench_function, commit) / "cli"


def get_actual_cli_repo(commit: str) -> Path:
    return TMPFOLDER / commit / "cli"


def create_symlink_for_repo(bench_function: str, commit: str):
    commit_folder = get_actual_commit_folder(commit)
    commit_link = get_bench_folder(bench_function) / commit
    try:
        commit_link.symlink_to(commit_folder)
    except FileExistsError:
        # relink in case its a wrong symlink
        commit_link.unlink()
        commit_link.symlink_to(commit_folder)


def create_folders_if_not_exist(bench_function: str):
    """
    Create following folders if they don't exist:
    - TMPFOLDER
    - build
    - data
    - build/{bench_function}
    - data/{bench_function}
    """
    TMPFOLDER.mkdir(exist_ok=True)
    BUILD_FOLDER.mkdir(exist_ok=True)
    DATA_FOLDER.mkdir(exist_ok=True)
    get_bench_folder(bench_function).mkdir(exist_ok=True)
    get_data_folder(bench_function).mkdir(exist_ok=True)


def delete_folder_if_no_symlink(folder: Path):
    for bench_folder in BUILD_FOLDER.iterdir():
        for commit_symlink in bench_folder.iterdir():
            if commit_symlink.resolve() == folder:
                return
    shutil.rmtree(folder)
