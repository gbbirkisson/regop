<h1>
  <p align="center">
    <a href="https://github.com/gbbirkisson/regop">
      <img src="logo.png" alt="Logo" height="128">
    </a>
    <br>regop
  </p>
</h1>

<p align="center">
  Easy file manipulation with <b>reg</b>ex and <b>op</b>erators
</p>

<!-- vim-markdown-toc GFM -->

* [Usage 📖](#usage-)
  * [TL;DR](#tldr)
  * [Regex](#regex)
  * [Operators](#operators)
    * [Table](#table)
* [Installation 💻](#installation-)
  * [Using cargo](#using-cargo)
  * [Using install script](#using-install-script)
  * [Download pre-built binaries](#download-pre-built-binaries)
* [Development 🚧](#development-)

<!-- vim-markdown-toc -->

## Usage 📖

### TL;DR

Use the `-r` to define regular expression capture groups and `-o` to define operators to
manipulate files:

```bash
# Increment edition in Cargo.toml by one
$ regop \
    -r 'edition = "(?<edition>[^"]+)' \
    -o '<edition>:inc' \
    Cargo.toml

┌───────────────────────────────────────────────────────────────────────────────
│ Cargo.toml
├───────────────────────────────────────────────────────────────────────────────
│ 3   3    │ version = "0.2.3"
│ 4        │-edition = "2024"
│     4    │+edition = "2025"
│ 5   5    │ repository = "https://github.com/gbbirkisson/regop"
└───────────────────────────────────────────────────────────────────────────────
```

```bash
# Swap anyhow major and patch version, increment minor by 3, decrement patch by 10
$ regop \
    -r 'anyhow = "(?<major>\d+)\.(?<minor>\d+)\.(?<patch>\d+)"' \
    -o '<major>:rep:<patch>' \
    -o '<minor>:inc:3' \
    -o '<patch>:dec:10' \
    Cargo.toml
┌───────────────────────────────────────────────────────────────────────────────
│ Cargo.toml
├───────────────────────────────────────────────────────────────────────────────
│ 20  20   │ [dependencies]
│ 21       │-anyhow = "1.0.95"
│     21   │+anyhow = "95.3.85"
│ 22  22   │ atty = "0.2.14"
└───────────────────────────────────────────────────────────────────────────────
```

```bash
# Update all major versions in all toml files
$ find -name '*.toml' | regop \
    -w \
    -r '"(?<major>\d+)\.(?<minor>\d+)\.(?<patch>\d+)"' \
    -o '<major>:inc'
```

```bash
# Read from stdin and write to stdout
$ cat Cargo.toml | regop -w \
    -r "version = \"(?<major>\d)\.(?<minor>\d)" \
    -o "<major>:rep:21" \
    -
```

### Regex

The first piece of the puzzle is that you use regular expressions with named capture groups to
extract some values in files. We use the rust
[regex](https://docs.rs/regex/latest/regex/#example-named-capture-groups) crate, so you can use
that documentation for reference.

Another excellent resource is [regex101](https://regex101.com/). The site fully supports the
rust [regex](https://docs.rs/regex/latest/regex/#example-named-capture-groups) crate and can
help you make sense of complicated expressions:

Here are some examples from [regex101](https://regex101.com/):
- Extract `major`, `minor` and `patch` version from file: [link](https://regex101.com/r/wR5BJ5/1)
- Extract `H2` subheadings from markdown: [link](https://regex101.com/r/ixUPEW/1)

### Operators

The second piece is that you can manipulate your capture groups with operators. Operators
take the form of:

```
<target>:operation:parameter
```

Where:

- `<target>` is the name of your capture group (include the `<` `>` signs).
- `operation` is the name of the desired operation (see [table](#table) below).
- `parameter` is the parameter to the operation (see [table](#table) below). Note that
`parameter` can reference another named capture.

#### Table

| Name  | Description | Default | Valid parameters | Examples |
| ----- | ---------------- | ------ | --------------------- | ------------------------------------- |
| `inc` | Increment number | `1`    | `int`, `<capture>`    | `<a>:inc`, `<a>:inc:5`, `<a>:inc:<b>` |
| `dec` | Decrement number | `1`    | `int`, `<capture>`    | `<a>:dec`, `<a>:dec:5`, `<a>:inc:<b>` |
| `rep` | Replace          | `None` | `string`, `<capture>` | `<a>:rep:mystring`, `<a>:rep:<b>`     |
| `del` | Delete           | `None` | `None`                | `<a>:del`                             |

## Installation 💻

### Using cargo

<!--x-release-please-start-version-->
```bash
$ cargo install --git https://github.com/gbbirkisson/regop.git --tag 0.4.1
```
<!--x-release-please-end-->

### Using install script

<!--x-release-please-start-version-->
```bash
$ curl --proto '=https' --tlsv1.2 -LsSf https://github.com/gbbirkisson/regop/releases/download/0.4.1/regop-installer.sh | sh
```
<!--x-release-please-end-->

### Download pre-built binaries

Go to the [latest release](https://github.com/gbbirkisson/regop/releases/latest) and download
the binary for your OS.

## Development 🚧

This is a regular rust project, so `cargo` will we enough. But if you want you can use
[just](https://github.com/casey/just):

```bash
$ just
Available recipes:
    build       # Build release
    ci          # Run CI pipeline
    default     # Show this help
    dist        # Recreate release.yml workflow
    install     # Install locally
    lint-clippy # Run clippy linter
    lint-fmt    # Run fmt linter
    run         # Little test runs
    test        # Run tests
```
