from __future__ import annotations

import argparse
import sys
import typer
from typing_extensions import Annotated
from typing import Optional

from postmortem import __version__, core_version, print_history
from typer import Typer


app = Typer(
    help="Catch AI coding agents cheating on your tests before you commit.",
    no_args_is_help=True,
    add_help_option=True,

)

def version_callback(value: bool):
    if value:
        typer.echo(f"postmortem {__version__}\ncore {core_version()}")
        raise typer.Exit()

@app.callback()
def main(
    version: Annotated[
        Optional[bool],
        typer.Option("--version", "-V", callback=version_callback, is_eager=True,
                     help="Show version and exit."),
    ] = None,
):
    pass

@app.command()
def check() -> None:
    """Check changes for test tampering."""
    print("No checks implemented yet.")


@app.command()
def gixh(path: str, limit: int = 10) -> None:
    """Print the last `limit` commits of the repository at `path`."""
    res = print_history(path, limit)
    print("\n".join(res))
    


if __name__ == "__main__":
    sys.exit(app())
