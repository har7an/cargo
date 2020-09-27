//! Tests for supporting older versions of the Cargo.lock file format.

use cargo_test_support::git;
use cargo_test_support::registry::Package;
use cargo_test_support::{basic_lib_manifest, basic_manifest, lines_match, project};

#[cargo_test]
fn oldest_lockfile_still_works() {
    let cargo_commands = vec!["build", "update"];
    for cargo_command in cargo_commands {
        oldest_lockfile_still_works_with_command(cargo_command);
    }
}

fn assert_lockfiles_eq(expected: &str, actual: &str) {
    for (l, r) in expected.lines().zip(actual.lines()) {
        assert!(lines_match(l, r), "Lines differ:\n{}\n\n{}", l, r);
    }

    assert_eq!(expected.lines().count(), actual.lines().count());
}

fn oldest_lockfile_still_works_with_command(cargo_command: &str) {
    Package::new("bar", "0.1.0").publish();

    let expected_lockfile = r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
[[package]]
name = "bar"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "[..]"

[[package]]
name = "foo"
version = "0.0.1"
dependencies = [
 "bar",
]
"#;

    let old_lockfile = r#"
[root]
name = "foo"
version = "0.0.1"
dependencies = [
 "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]

[[package]]
name = "bar"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#;

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file("Cargo.lock", old_lockfile)
        .build();

    p.cargo(cargo_command).run();

    let lock = p.read_lockfile();
    assert_lockfiles_eq(expected_lockfile, &lock);
}

#[cargo_test]
fn frozen_flag_preserves_old_lockfile() {
    let cksum = Package::new("bar", "0.1.0").publish();

    let old_lockfile = format!(
        r#"[root]
name = "foo"
version = "0.0.1"
dependencies = [
 "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]

[[package]]
name = "bar"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[metadata]
"checksum bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)" = "{}"
"#,
        cksum,
    );

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file("Cargo.lock", &old_lockfile)
        .build();

    p.cargo("build --locked").run();

    let lock = p.read_lockfile();
    assert_lockfiles_eq(&old_lockfile, &lock);
}

#[cargo_test]
fn totally_wild_checksums_works() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "Cargo.lock",
            r#"
[[package]]
name = "foo"
version = "0.0.1"
dependencies = [
 "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]

[[package]]
name = "bar"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[metadata]
"checksum baz 0.1.2 (registry+https://github.com/rust-lang/crates.io-index)" = "checksum"
"checksum bar 0.1.2 (registry+https://github.com/rust-lang/crates.io-index)" = "checksum"
"#,
        );

    let p = p.build();

    p.cargo("build").run();

    let lock = p.read_lockfile();
    assert_lockfiles_eq(
        r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
[[package]]
name = "bar"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "[..]"

[[package]]
name = "foo"
version = "0.0.1"
dependencies = [
 "bar",
]
"#,
        &lock,
    );
}

#[cargo_test]
fn wrong_checksum_is_an_error() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "Cargo.lock",
            r#"
[[package]]
name = "foo"
version = "0.0.1"
dependencies = [
 "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]

[[package]]
name = "bar"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[metadata]
"checksum bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)" = "checksum"
"#,
        );

    let p = p.build();

    p.cargo("build")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] `[..]` index
error: checksum for `bar v0.1.0` changed between lock files

this could be indicative of a few possible errors:

    * the lock file is corrupt
    * a replacement source in use (e.g., a mirror) returned a different checksum
    * the source itself may be corrupt in one way or another

unable to verify that `bar v0.1.0` is the same as when the lockfile was generated

",
        )
        .run();
}

// If the checksum is unlisted in the lock file (e.g., <none>) yet we can
// calculate it (e.g., it's a registry dep), then we should in theory just fill
// it in.
#[cargo_test]
fn unlisted_checksum_is_bad_if_we_calculate() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "Cargo.lock",
            r#"
[[package]]
name = "foo"
version = "0.0.1"
dependencies = [
 "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]

[[package]]
name = "bar"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[metadata]
"checksum bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)" = "<none>"
"#,
        );
    let p = p.build();

    p.cargo("fetch")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] `[..]` index
error: checksum for `bar v0.1.0` was not previously calculated, but a checksum \
could now be calculated

this could be indicative of a few possible situations:

    * the source `[..]` did not previously support checksums,
      but was replaced with one that does
    * newer Cargo implementations know how to checksum this source, but this
      older implementation does not
    * the lock file is corrupt

",
        )
        .run();
}

// If the checksum is listed in the lock file yet we cannot calculate it (e.g.,
// Git dependencies as of today), then make sure we choke.
#[cargo_test]
fn listed_checksum_bad_if_we_cannot_compute() {
    let git = git::new("bar", |p| {
        p.file("Cargo.toml", &basic_manifest("bar", "0.1.0"))
            .file("src/lib.rs", "")
    });

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "foo"
                    version = "0.0.1"
                    authors = []

                    [dependencies]
                    bar = {{ git = '{}' }}
                "#,
                git.url()
            ),
        )
        .file("src/lib.rs", "")
        .file(
            "Cargo.lock",
            &format!(
                r#"
[[package]]
name = "foo"
version = "0.0.1"
dependencies = [
 "bar 0.1.0 (git+{0})"
]

[[package]]
name = "bar"
version = "0.1.0"
source = "git+{0}"

[metadata]
"checksum bar 0.1.0 (git+{0})" = "checksum"
"#,
                git.url()
            ),
        );

    let p = p.build();

    p.cargo("fetch")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] git repository `[..]`
error: checksum for `bar v0.1.0 ([..])` could not be calculated, but a \
checksum is listed in the existing lock file[..]

this could be indicative of a few possible situations:

    * the source `[..]` supports checksums,
      but was replaced with one that doesn't
    * the lock file is corrupt

unable to verify that `bar v0.1.0 ([..])` is the same as when the lockfile was generated

",
        )
        .run();
}

#[cargo_test]
fn current_lockfile_format() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "");
    let p = p.build();

    p.cargo("build").run();

    let actual = p.read_lockfile();

    let expected = "\
# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.
[[package]]
name = \"bar\"
version = \"0.1.0\"
source = \"registry+https://github.com/rust-lang/crates.io-index\"
checksum = \"[..]\"

[[package]]
name = \"foo\"
version = \"0.0.1\"
dependencies = [
 \"bar\",
]
";
    assert_lockfiles_eq(expected, &actual);
}

#[cargo_test]
fn lockfile_without_root() {
    Package::new("bar", "0.1.0").publish();

    let lockfile = r#"
# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
[[package]]
name = "bar"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "foo"
version = "0.0.1"
dependencies = [
 "bar",
]
"#;

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file("Cargo.lock", lockfile);

    let p = p.build();

    p.cargo("build").run();

    let lock = p.read_lockfile();
    assert_lockfiles_eq(
        r#"# [..]
# [..]
[[package]]
name = "bar"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "[..]"

[[package]]
name = "foo"
version = "0.0.1"
dependencies = [
 "bar",
]
"#,
        &lock,
    );
}

#[cargo_test]
fn locked_correct_error() {
    Package::new("bar", "0.1.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "");
    let p = p.build();

    p.cargo("build --locked")
        .with_status(101)
        .with_stderr(
            "\
[UPDATING] `[..]` index
error: the lock file [CWD]/Cargo.lock needs to be updated but --locked was passed to prevent this
If you want to try to generate the lock file without accessing the network, use the --offline flag.
",
        )
        .run();
}

#[cargo_test]
fn v2_format_preserved() {
    let cksum = Package::new("bar", "0.1.0").publish();

    let lockfile = format!(
        r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
[[package]]
name = "bar"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "{}"

[[package]]
name = "foo"
version = "0.0.1"
dependencies = [
 "bar",
]
"#,
        cksum
    );

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file("Cargo.lock", &lockfile)
        .build();

    p.cargo("fetch").run();

    let lock = p.read_lockfile();
    assert_lockfiles_eq(&lockfile, &lock);
}

#[cargo_test]
fn v2_path_and_crates_io() {
    let cksum010 = Package::new("a", "0.1.0").publish();
    let cksum020 = Package::new("a", "0.2.0").publish();

    let lockfile = format!(
        r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
[[package]]
name = "a"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "{}"

[[package]]
name = "a"
version = "0.2.0"

[[package]]
name = "a"
version = "0.2.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "{}"

[[package]]
name = "foo"
version = "0.0.1"
dependencies = [
 "a 0.1.0",
 "a 0.2.0",
 "a 0.2.0 (registry+https://github.com/rust-lang/crates.io-index)",
]
"#,
        cksum010, cksum020,
    );

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                a = { path = 'a' }
                b = { version = "0.1", package = 'a' }
                c = { version = "0.2", package = 'a' }
            "#,
        )
        .file("src/lib.rs", "")
        .file(
            "a/Cargo.toml",
            r#"
                [project]
                name = "a"
                version = "0.2.0"
            "#,
        )
        .file("a/src/lib.rs", "")
        .file("Cargo.lock", &lockfile)
        .build();

    p.cargo("fetch").run();
    p.cargo("fetch").run();

    let lock = p.read_lockfile();
    assert_lockfiles_eq(&lockfile, &lock);
}

#[cargo_test]
fn v3_and_git() {
    let (git_project, repo) = git::new_repo("dep1", |project| {
        project
            .file("Cargo.toml", &basic_lib_manifest("dep1"))
            .file("src/lib.rs", "")
    });
    let head_id = repo.head().unwrap().target().unwrap();

    let lockfile = format!(
        r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "dep1"
version = "0.5.0"
source = "git+{}?branch=master#{}"

[[package]]
name = "foo"
version = "0.0.1"
dependencies = [
 "dep1",
]
"#,
        git_project.url(),
        head_id,
    );

    let p = project()
        .file(
            "Cargo.toml",
            &format!(
                r#"
                    [project]
                    name = "foo"
                    version = "0.0.1"
                    authors = []

                    [dependencies]
                    dep1 = {{ git = '{}', branch = 'master' }}
                "#,
                git_project.url(),
            ),
        )
        .file("src/lib.rs", "")
        .file("Cargo.lock", "version = 3")
        .build();

    p.cargo("fetch").run();

    let lock = p.read_lockfile();
    assert_lockfiles_eq(&lockfile, &lock);
}

#[cargo_test]
fn lock_from_the_future() {
    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []
            "#,
        )
        .file("src/lib.rs", "")
        .file("Cargo.lock", "version = 10000000")
        .build();

    p.cargo("fetch")
        .with_stderr(
            "\
error: failed to parse lock file at: [..]

Caused by:
  lock file version `10000000` was found, but this version of Cargo does not \
  understand this lock file, perhaps Cargo needs to be updated?
",
        )
        .with_status(101)
        .run();
}

#[cargo_test]
fn preserve_old_format_if_no_update_needed() {
    let cksum = Package::new("bar", "0.1.0").publish();
    let lockfile = format!(
        r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
[[package]]
name = "bar"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "foo"
version = "0.0.1"
dependencies = [
 "bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]

[metadata]
"checksum bar 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)" = "{}"
"#,
        cksum
    );

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [project]
                name = "foo"
                version = "0.0.1"
                authors = []

                [dependencies]
                bar = "0.1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file("Cargo.lock", &lockfile)
        .build();

    p.cargo("build --locked").run();
}
