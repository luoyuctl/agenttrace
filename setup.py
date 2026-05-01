"""agenttrace - AI Agent Session Performance Analyzer."""
from setuptools import setup, find_packages

setup(
    name="agenttrace",
    version="0.1.0",
    description="AI Agent Session Performance Analyzer - find hanging, token waste, and quality regressions",
    py_modules=["agenttrace"],
    entry_points={
        "console_scripts": [
            "agenttrace=agenttrace:main",
        ],
    },
    python_requires=">=3.9",
)
