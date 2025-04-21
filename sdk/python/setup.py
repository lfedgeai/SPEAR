import re
import subprocess

from setuptools import find_packages, setup


def convert_to_pep440(version_str):
    '''
    Convert a version string from git describe format to PEP 440 format.
    '''
    version_str = version_str.lstrip('v')

    pattern = r'^(\d+\.\d+\.\d+)-(\d+)-g([0-9a-f]+)(-dirty)?$'
    match = re.match(pattern, version_str)

    if match:
        major_version = match.group(1)
        commit_count = match.group(2)
        commit_hash = match.group(3)
        is_dirty = match.group(4)

        pep440_version = major_version
        if commit_count:
            pep440_version += f"+{commit_count}.g{commit_hash}"
        if is_dirty:
            pep440_version += is_dirty.replace('-', '.')

        return pep440_version
    else:
        return version_str


def get_version():
    '''
    Get the version from the git tag.
    '''
    try:
        result = subprocess.check_output(['git', 'describe', '--tags', '--match',
                                          '*', '--always', '--dirty'],
                                         stderr=subprocess.STDOUT)
        result = result.decode('utf-8').strip()
        return convert_to_pep440(result)
    except subprocess.CalledProcessError as e:
        print(f"Git command failed with error: {e.output.decode('utf-8')}")
    except FileNotFoundError:
        print("Git executable not found.")
    return '0.0.0'


setup(
    name="spear",
    version=get_version(),
    description="SPEAR Python SDK",
    author="Wilson Wang",
    license="Apache-2.0",
    python_requires=">=3.6",
    packages=find_packages(include=["spear", "spear.*"]),
    include_package_data=True,
    # dependencies
    install_requires=[
        "dataclasses-json",
        "flatbuffers",
        "numpy",
    ],
    # packages for building
    setup_requires=[
        "setuptools",
        "wheel",
        "pytest",
    ],
)
