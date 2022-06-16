// -Zvalidate-mir -Zverify-llvm-ir=yes -Zincremental-verify-ich=yes -Zmir-opt-level=0 -Zmir-opt-level=1 -Zmir-opt-level=2 -Zmir-opt-level=3 -Zdump-mir=all --emit=mir -Zsave-analysis -Zprint-mono-items=full
//&q["-Zcrate-attr=feature(generic_associated_types)"],
// git grep -o  "unstable(feature = \"[A-Za-z_-]*"   | grep -o "\ .*$" | grep -o "\".*" | sed s/\"// | sort -n | uniq | grep "...."
pub(crate) static RUSTC_FLAGS: &[&[&str]] = &[
    // all allow-by-default lints, split into two because otherwise the get_flag_combinations would eat all ram
    // I might fix this at some point by making it work lists of &str instead of String
    &[
        // must_not_suspend and non_exhaustive_omitted_patterns are unstable :(
        "-Wabsolute-paths-not-starting-with-crate",
        "-Wbox-pointers",
        "-Wdeprecated-in-future",
        "-Welided-lifetimes-in-paths",
        "-Wexplicit-outlives-requirements",
        "-Wfuzzy-provenance-casts",
        "-Wlossy-provenance-casts",
        "-Wkeyword-idents",
        "-Wmacro-use-extern-crate",
        "-Wmeta-variable-misuse",
        "-Wmissing-abi",
        "-Wmissing-copy-implementations",
        "-Wmissing-debug-implementations",
        "-Wmissing-docs",
        // "-Wmust-not-suspend",
        "-Wnon-ascii-idents",
        // "-Wnon-exhaustive-omitted-patterns",
        "-Wnoop-method-call",
        "-Wpointer-structural-match",
        "-Wrust-2021-incompatible-closure-captures",
    ],
    &[
        "-Wrust-2021-incompatible-or-patterns",
        "-Wrust-2021-prefixes-incompatible-syntax",
        "-Wrust-2021-prelude-collisions",
        "-Wsingle-use-lifetimes",
        "-Wtrivial-casts",
        "-Wtrivial-numeric-casts",
        "-Wunreachable-pub",
        "-Wunsafe-code",
        "-Wunsafe-op-in-unsafe-fn",
        "-Wunstable-features",
        "-Wunused-crate-dependencies",
        "-Wunused-extern-crates",
        "-Wunused-import-braces",
        "-Wunused-lifetimes",
        "-Wunused-macro-rules",
        "-Wunused-qualifications",
        "-Wunused-results",
        "-Wvariant-size-differences",
    ],
    /*  &[
        "-Zvalidate-mir",
        "-Zverify-llvm-ir=yes",
        "-Zincremental-verify-ich=yes",
        "-Zmir-opt-level=0",
        "-Zmir-opt-level=1",
        "-Zmir-opt-level=2",
        "-Zmir-opt-level=3",
        "-Zmir-opt-level=4",
        //  "-Zunsound-mir-opts",
        "-Zdump-mir=all",
        "--emit=mir",
        "-Zsave-analysis",
        "-Zprint-mono-items=full",
        "-Zpolymorphize=on",
        "-Zalways-encode-mir",
    ], */
    &[
        "-Zvalidate-mir",
        "-Zverify-llvm-ir=yes",
        "-Zincremental-verify-ich=yes",
        "-Zmir-opt-level=0",
        "-Zmir-opt-level=1",
        "-Zmir-opt-level=2",
        "-Zmir-opt-level=3",
        "-Zmir-opt-level=4",
        //  "-Zunsound-mir-opts",
        "-Zdump-mir=all",
        "--emit=mir",
        "-Zsave-analysis",
        "-Zprint-mono-items=full",
        "-Zpolymorphize=on",
        "-Zalways-encode-mir",
        "-Cpasses=lint",
        "--edition=2015",
    ],
    &[
        "-Zvalidate-mir",
        "-Zverify-llvm-ir=yes",
        "-Zincremental-verify-ich=yes",
        "-Zmir-opt-level=0",
        "-Zmir-opt-level=1",
        "-Zmir-opt-level=2",
        "-Zmir-opt-level=3",
        "-Zmir-opt-level=4",
        //  "-Zunsound-mir-opts",
        "-Zdump-mir=all",
        "--emit=mir",
        "-Zsave-analysis",
        "-Zprint-mono-items=full",
        "-Zpolymorphize=on",
        "-Zalways-encode-mir",
        "-Cpasses=lint",
        "--edition=2018",
    ],
    &[
        "-Zvalidate-mir",
        "-Zverify-llvm-ir=yes",
        "-Zincremental-verify-ich=yes",
        "-Zmir-opt-level=0",
        "-Zmir-opt-level=1",
        "-Zmir-opt-level=2",
        "-Zmir-opt-level=3",
        "-Zmir-opt-level=4",
        //  "-Zunsound-mir-opts",
        "-Zdump-mir=all",
        "--emit=mir",
        "-Zsave-analysis",
        "-Zprint-mono-items=full",
        "-Zpolymorphize=on",
        "-Zalways-encode-mir",
        "-Cpasses=lint",
        "--edition=2021",
    ],
    &["INCR_COMP"],
    // &["-Zborrowck=mir", "-Zcrate-attr=feature(nll)"],
    // temporary disable these for more throughput... haven't found new bugs with these in a long time
    /*
    &["-Cinstrument-coverage"],
    &["-Cprofile-generate=/tmp/icemaker_pgo/"],
    &["-Copt-level=z"],
    &["-Zsanitizer=address"],
    &["-Zsanitizer=memory"],
    &["-Zunpretty=normal"],
    &["-Zunpretty=identified"],
    &["-Zunpretty=expanded"],
    &["-Zunpretty=expanded,identified"],
    &["-Zunpretty=ast-tree"],
    &["-Zunpretty=ast-tree,expanded"],
    &["-Zunpretty=hir"],
    &["-Zunpretty=hir,identified"],
    &["-Zunpretty=hir-tree"],
    &["-Zunpretty=thir-tree"],
    &["-Zunpretty=hir,typed"],
    &["-Zunpretty=expanded,hygiene"],
    &["-Zunpretty=mir"],
    &["-Zunpretty=mir-cfg"],
    &["-Zunpretty=ast,expanded"],
    &["-Zthir-unsafeck=yes"],
    &["-Zdump-mir=all", "-Zdump-mir-dataflow"],
    */ /*
    &[
        "-Zmir-opt-level=3",
        "-Cdebuginfo=2",
        "-Copt-level=3",
        "-Zsanitizer=address",
        "-Clto",
    ],
    &[
        "-Zmir-opt-level=3",
        "-Cdebuginfo=2",
        "-Copt-level=3",
        "-Zsanitizer=cfi",
        "-Clto",
    ],
    &[
        "-Zmir-opt-level=3",
        "-Cdebuginfo=2",
        "-Copt-level=3",
        "-Zsanitizer=leak",
        "-Clto",
    ],
    &[
        "-Zmir-opt-level=3",
        "-Cdebuginfo=2",
        "-Copt-level=3",
        "-Zsanitizer=memory",
        "-Clto",
    ],
    &[
        "-Zmir-opt-level=3",
        "-Cdebuginfo=2",
        "-Copt-level=3",
        "-Zsanitizer=thread",
        "-Clto",
    ],
    &["-Zpolonius"],
    */
];

pub(crate) static EXCEPTIONS: &[&str] = &[
    // runtime
    "./src/test/ui/closures/issue-72408-nested-closures-exponential.rs",
    "./src/test/ui/issues/issue-74564-if-expr-stack-overflow.rs",
    "./library/stdarch/crates/core_arch/src/mod.rs", //10+ mins
    // memory
    "./src/test/ui/issues/issue-50811.rs",
    "./src/test/ui/issues/issue-29466.rs",
    "./src/tools/miri/tests/run-pass/float.rs",
    "./src/test/ui/numbers-arithmetic/saturating-float-casts-wasm.rs",
    "./src/test/ui/numbers-arithmetic/saturating-float-casts-impl.rs",
    "./src/test/ui/numbers-arithmetic/saturating-float-casts.rs",
    "./src/test/ui/wrapping-int-combinations.rs",
    // glacier/memory/time:
    "./fixed/23600.rs",
    "./23600.rs",
    "./fixed/71699.rs",
    "./71699.rs",
    // runtime
    "./library/stdarch/crates/core_arch/src/x86/avx512bw.rs",
    "./library/stdarch/crates/core_arch/src/x86/mod.rs",
    // 3.5 hours when reporting errors :(
    "./library/stdarch/crates/core_arch/src/lib.rs",
    // memory 2.0
    "./src/test/run-make-fulldeps/issue-47551/eh_frame-terminator.rs",
];

pub(crate) static MIRI_EXCEPTIONS: &[&str] = &[
    // all of clippy as well..?
    // most of these have infinite loops in runtime
    "./library/alloc/benches/vec_deque.rs",
    "./library/alloc/benches/vec_deque_append.rs",
    "./library/alloc/tests/vec_deque.rs",
    "./src/test/ui/consts/const-eval/infinite_loop.rs",
    "./src/test/ui/consts/promote_evaluation_unused_result.rs",
    "./src/test/ui/issues/issue-25579.rs",
    "./src/test/ui/iterators/iter-count-overflow-debug.rs",
    "./src/test/ui/iterators/iter-count-overflow-ndebug.rs",
    "./src/test/ui/iterators/iter-position-overflow-debug.rs",
    "./src/test/ui/iterators/iter-position-overflow-ndebug.rs",
    "./src/test/ui/iterators/skip-count-overflow.rs",
    "./src/test/ui/lint/lint-impl-fn.rs",
    "./src/test/ui/lint/lint-unnecessary-parens.rs",
    "./src/test/ui/reachable/expr_again.rs",
    "./src/test/ui/reachable/unreachable-code.rs",
    "./src/test/ui/rfc-2497-if-let-chains/irrefutable-lets.rs",
    "./src/test/ui/try-block/try-block-unreachable-code-lint.rs",
    "./src/test/ui/unreachable-code-1.rs",
    "./src/test/ui/unreachable-code.rs",
    "./src/test/ui/lint/rfc-2383-lint-reason/catch_multiple_lint_triggers.rs",
    "./src/test/ui/lint/suggestions.rs",
    "./src/test/ui/const-generics/infer_arr_len_from_pat.rs",
    "./src/test/ui/lint/suggestions.rs",
    "./src/test/ui/lint/lint-change-warnings.rs",
    "./src/tools/rust-analyzer/crates/parser/test_data/parser/ok/0059_loops_in_parens.rs",
    "./src/test/ui/rfc-2497-if-let-chains/no-double-assigments.rs",
    "./src/test/ui/lint/unused_labels.rs",
    "./src/test/ui/polymorphization/predicates.rs",
    "./src/test/ui/lint/rfc-2383-lint-reason/expect_multiple_lints.rs",
    "./src/test/ui/impl-trait/issues/issue-55608-captures-empty-region.rs",
    "./src/test/ui/lint/rfc-2383-lint-reason/expect_multiple_lints.rs",
    "./src/test/ui/codegen/issue-88043-bb-does-not-have-terminator.rs",
    "./src/test/ui/pattern/usefulness/top-level-alternation.rs",
    "./src/test/ui/issues/issue-12860.rs",
    "./src/test/ui/lint/rfc-2383-lint-reason/catch_multiple_lint_triggers.rs",
    "./src/test/ui/threads-sendsync/issue-8827.rs",
    "./src/test/mir-opt/inline/inline-cycle-generic.rs",
    "./src/test/ui/issues/issue-73229.rs",
    "./src/test/ui/consts/huge-values.rs",
    "./src/test/ui/threads-sendsync/issue-9396.rs",
    "./src/tools/rust-analyzer/crates/parser/test_data/parser/ok/0057_loop_in_call.rs",
    "./src/test/ui/panics/panic-set-handler.rs",
    "./src/doc/book/listings/ch03-common-programming-concepts/no-listing-32-loop/src/main.rs",
    "./src/doc/book/listings/ch19-advanced-features/no-listing-10-loop-returns-never/src/main.rs",
    "./src/test/ui/issues/issue-75704.rs",
    "./src/test/ui/panics/panic-set-handler.rs",
    "./src/test/ui/codegen/issue-88043-bb-does-not-have-terminator.rs",
    "./src/test/ui/issue-25579.rs",
    "./src/test/compile-fail/issue-25579.rs",
    "./src/test/ui/issue-25579.rs",
    "./src/test/ui/issues/issue-25579.rs",
    "./src/tools/clippy/tests/ui/while_let_on_iterator.rs",
    "./src/test/compile-fail/borrowck/borrowck-mut-borrow-linear-errors.rs",
    "./src/test/compile-fail/E0165.rs",
    "./src/test/ui/error-codes/E0165.rs",
];

pub(crate) static MIRIFLAGS: &[&[&str]] = &[
    // with mir opt level
    /*  &[
        "-Zmir-opt-level=4",
        "-Zmiri-check-number-validity",
        "-Zmiri-strict-provenance",
        "-Zmiri-symbolic-alignment-check",
        "-Zmiri-tag-raw-pointers",
    ], */
    // and without
    &[
        "-Zmiri-check-number-validity",
        "-Zmiri-strict-provenance",
        "-Zmiri-symbolic-alignment-check",
        "-Zmiri-tag-raw-pointers",
        "-Zmiri-mute-stdout-stderr",
        //"-Zmir-opt-level=4",
        // "-Zrandomize-layout",
    ],
];

// TODO: tests
pub(crate) static MIRIRUSTFLAGS: &[&[&str]] = &[
    &["--edition=2015", "-Zvalidate-mir"],
    &["--edition=2018", "-Zvalidate-mir"],
    &["--edition=2021", "-Zvalidate-mir"],
];

#[cfg(test)]
mod tests {
    use super::{EXCEPTIONS, MIRIFLAGS, MIRI_EXCEPTIONS, RUSTC_FLAGS};
    use crate::ice::*;
    use std::fs::File;
    use std::io::Write;
    use tempdir::TempDir;

    const DUMMY_FILE_CONTENT: &str = "pub fn main() {}\n";

    #[test]
    fn rustc_flags_are_valid() {
        // make sure we don't have invalid rustc flags
        for (i, batch_of_flags) in RUSTC_FLAGS
            .iter()
            // skip incr comp here, needs to be special cased!
            .filter(|flags| flags != &&["INCR_COMP"])
            .enumerate()
        {
            let tempdir = TempDir::new(&i.to_string()).expect("failed to create tempdir!");
            let tempdir_path = tempdir.path();
            let rustfile_path = tempdir_path.join("file.rs");
            let mut rustfile = File::create(&rustfile_path).unwrap();
            writeln!(rustfile, "{}", DUMMY_FILE_CONTENT).unwrap();

            let output = &std::process::Command::new(&Executable::Rustc.path())
                .args(*batch_of_flags)
                .arg(&rustfile_path)
                .output()
                .unwrap();

            dbg!(output);
            assert!(output.status.success());
        }
    }

    #[test]
    fn filepaths_are_valid() {
        let paths_iter = EXCEPTIONS.iter().chain(MIRI_EXCEPTIONS.iter());

        paths_iter.for_each(|file| {
            assert!(file.starts_with("./"), "{}", file);
            assert!(file.ends_with(".rs"), "{}", file);
        });
    }

    #[test]
    fn test_miriflags_are_valid() {
        for (i, batch_of_flags) in MIRIFLAGS.iter().enumerate() {
            let tempdir = TempDir::new(&format!("icemaker_miri_tempdir_{}", i)).unwrap();
            let tempdir_path = tempdir.path();
            // create a new cargo project inside the tmpdir

            // dummy crate name
            let crate_name = &format!("_{}", i);

            let mut cmd = std::process::Command::new("cargo");
            cmd.arg("new")
                .args(["--vcs", "none"])
                .arg(crate_name)
                .current_dir(&tempdir_path);

            let status = cmd
                .output()
                .expect("failed to exec cargo new")
                .status
                .success();

            dbg!(&cmd);

            assert!(status, "failed to run cargo new");

            let source_path = {
                let mut sp = tempdir_path.to_owned();
                sp.push(crate_name);
                sp.push("src/");
                sp.push("main.rs");
                sp
            };

            // write the content of the file we want to check into tmpcrate/src/main.rs
            std::fs::write(source_path, DUMMY_FILE_CONTENT).expect("failed to write to file");

            // we should have everything prepared for the miri invocation now: execute "cargo miri run"

            let mut crate_path = tempdir_path.to_owned();
            crate_path.push(crate_name);

            let mut cmd = std::process::Command::new("cargo");

            assert!(
                cmd.arg("miri")
                    .arg("run")
                    .current_dir(crate_path)
                    .env("MIRIFLAGS", batch_of_flags.join(" "))
                    .env("RUSTFLAGS", "-Zvalidate-mir")
                    .output()
                    .unwrap()
                    .status
                    .success(),
                "miri flags bad: '{:?}'",
                batch_of_flags
            );
        }
    }
}
