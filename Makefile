.PHONY: init install clean-pyc clean-build build test publish docs-init docs milestone-pre milestone-post governance-check cargo-check report

M ?= M000

init:
	conda install --file requirements.txt

install:
	pip install -e .

test:
	nosetests

clean-pyc:
	find . -name '*.pyc' -exec rm --force {} +
	find . -name '*.pyo' -exec rm --force {} +
	find . -name '*~' -exec rm --force  {} +

clean-build:
	rm -rf build/
	rm -rf dist/

clean: clean-pyc clean-build

build: clean-build
	python setup.py sdist
	# python setup.py bdist_wheel

publish: build
	twine upload dist/*

publish-test:
	twine upload --repository-url https://test.pypi.org/legacy/ dist/*

milestone-pre:
	python3 scripts/milestone_gate.py pre --milestone $(M)

milestone-post:
	python3 scripts/milestone_gate.py post --milestone $(M)

governance-check:
	python3 -m py_compile scripts/*.py
	bash -n scripts/cargo_guard.sh
	python3 scripts/check_milestone_schema.py
	python3 scripts/check_no_runtime_pairtools.py --milestone $(M)
	python3 scripts/check_no_noop_flags.py --milestone $(M)
	python3 scripts/check_parse_lite_drift.py --milestone $(M)

cargo-check:
	scripts/cargo_guard.sh check

report:
	python3 scripts/codex_report.py --milestone $(M)

#docs-init:
#	conda install --file docs/requirements.txt
#
#docs:
#	cd docs && python make_cli_rst.py && make html
