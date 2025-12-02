#![feature(rustc_private)]

//! Tests for immutable borrow decoration detection.

use ferrous_owl::test::TestCase;
#[test]
fn imm_borrow_println() {
    TestCase::new(
        "imm_borrow_println",
        r#"
        fn test() {
            let s = String::from("hello");
            println!("{}", s);
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_imm_borrow()
    .run();
}

#[test]
fn imm_borrow_method_call() {
    TestCase::new(
        "imm_borrow_method_call",
        r#"
        fn test() {
            let s = String::from("hello");
            let _len = s.len();
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_imm_borrow()
    .run();
}

#[test]
fn imm_borrow_reference() {
    TestCase::new(
        "imm_borrow_reference",
        r#"
        fn test() {
            let s = String::from("hello");
            let _r = &s;
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_imm_borrow()
    .run();
}

#[test]
fn imm_borrow_function_param() {
    TestCase::new(
        "imm_borrow_function_param",
        r#"
        fn print_str(s: &str) {
            println!("{}", s);
        }

        fn test() {
            let s = String::from("hello");
            print_str(&s);
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_imm_borrow()
    .run();
}

#[test]
fn imm_borrow_deref() {
    TestCase::new(
        "imm_borrow_deref",
        r#"
        fn test() {
            let s = String::from("hello");
            let _first = s.chars().next();
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_imm_borrow()
    .run();
}

#[test]
fn imm_borrow_slice() {
    TestCase::new(
        "imm_borrow_slice",
        r#"
        fn test() {
            let v = vec![1, 2, 3];
            let _slice = &v[..];
        }
    "#,
    )
    .cursor_on("v = vec!")
    .expect_imm_borrow()
    .run();
}

#[test]
fn imm_borrow_iter() {
    TestCase::new(
        "imm_borrow_iter",
        r#"
        fn test() {
            let v = vec![1, 2, 3];
            for x in &v {
                let _ = x;
            }
        }
    "#,
    )
    .cursor_on("v = vec!")
    .expect_imm_borrow()
    .run();
}

#[test]
fn imm_borrow_contains() {
    TestCase::new(
        "imm_borrow_contains",
        r#"
        fn test() {
            let s = String::from("hello world");
            let _has = s.contains("world");
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_imm_borrow()
    .run();
}

#[test]
fn imm_borrow_debug() {
    TestCase::new(
        "imm_borrow_debug",
        r#"
        fn test() {
            let v = vec![1, 2, 3];
            println!("{:?}", v);
        }
    "#,
    )
    .cursor_on("v = vec!")
    .expect_imm_borrow()
    .run();
}

#[test]
fn imm_borrow_comparison() {
    TestCase::new(
        "imm_borrow_comparison",
        r#"
        fn test() {
            let a = String::from("hello");
            let b = String::from("world");
            let _cmp = a == b;
        }
    "#,
    )
    .cursor_on("a = String")
    .expect_imm_borrow()
    .run();
}

#[test]
fn imm_borrow_is_empty() {
    TestCase::new(
        "imm_borrow_is_empty",
        r#"
        fn test() {
            let s = String::new();
            let _empty = s.is_empty();
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_imm_borrow()
    .run();
}

#[test]
fn imm_borrow_clone() {
    TestCase::new(
        "imm_borrow_clone",
        r#"
        fn test() {
            let s = String::from("hello");
            let _clone = s.clone();
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_imm_borrow()
    .run();
}
