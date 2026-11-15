import postmortem
from postmortem.cli import main


def test_core_version():
    assert postmortem.core_version() == postmortem.__version__


def test_cli_check():
    assert main(["check"]) == 0
