#![feature(rustc_private)]

//! Tests for reference and borrow lifetime patterns.
//!
//! Note: The `Lifetime` decoration type exists but is filtered out from
//! diagnostics as "too verbose". References produce `imm-borrow` or
//! `mut-borrow` decorations instead.

use ferrous_owl::test::TestCase;
#[test]
fn lifetime_basic_reference() {
    // References show as imm-borrow decorations (Lifetime is filtered)
    TestCase::new(
        "lifetime_basic_reference",
        r#"
        fn test() {
            let s = String::from("hello");
            let r = &s;
            println!("{}", r);
        }
    "#,
    )
    .cursor_on("r = &s")
    .expect_imm_borrow()
    .run();
}

#[test]
fn lifetime_function_param() {
    // Function calls that take references produce Call decorations
    TestCase::new(
        "lifetime_function_param",
        r#"
        fn first<'a>(s: &'a str) -> &'a str {
            &s[..1]
        }

        fn test() {
            let s = String::from("hello");
            let _f = first(&s);
        }
    "#,
    )
    .cursor_on("_f = first")
    .expect_call()
    .run();
}

#[test]
fn lifetime_struct_field() {
    // Struct construction with reference field - no special decoration on the
    // struct itself The borrow is implicit in the field assignment
    TestCase::new(
        "lifetime_struct_field",
        r#"
        struct Wrapper<'a> {
            data: &'a str,
        }

        fn test() {
            let s = String::from("hello");
            let _w = Wrapper { data: &s };
        }
    "#,
    )
    .cursor_on("s = String") // Focus on the source String
    .expect_imm_borrow() // The &s borrows s
    .run();
}

#[test]
fn lifetime_return_reference() {
    // Function call that returns a reference - Call decoration
    TestCase::new(
        "lifetime_return_reference",
        r#"
        fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
            if a.len() > b.len() { a } else { b }
        }

        fn test() {
            let s1 = String::from("hello");
            let s2 = String::from("world!");
            let _result = longest(&s1, &s2);
        }
    "#,
    )
    .cursor_on("_result = longest")
    .expect_call()
    .run();
}

#[test]
fn lifetime_mut_reference() {
    // Mutable references produce mut-borrow decorations
    TestCase::new(
        "lifetime_mut_reference",
        r#"
        fn test() {
            let mut s = String::from("hello");
            let r = &mut s;
            r.push_str(" world");
        }
    "#,
    )
    .cursor_on("r = &mut")
    .expect_mut_borrow()
    .run();
}

#[test]
fn lifetime_slice() {
    TestCase::new(
        "lifetime_slice",
        r#"
        fn test() {
            let v = vec![1, 2, 3, 4, 5];
            let slice = &v[1..4];
            println!("{:?}", slice);
        }
    "#,
    )
    .cursor_on("slice = &v")
    .expect_imm_borrow()
    .run();
}

#[test]
fn lifetime_static() {
    // Static references are trivial borrows
    TestCase::new(
        "lifetime_static",
        r#"
        static GREETING: &str = "hello";

        fn test() {
            let r: &'static str = GREETING;
            println!("{}", r);
        }
    "#,
    )
    .cursor_on("r:")
    .expect_imm_borrow()
    .run();
}

#[test]
fn lifetime_nested_struct() {
    // The inner variable is moved into the outer struct
    TestCase::new(
        "lifetime_nested_struct",
        r#"
        struct Inner<'a> {
            data: &'a str,
        }

        struct Outer<'a> {
            inner: Inner<'a>,
        }

        fn test() {
            let s = String::from("hello");
            let inner = Inner { data: &s };
            let _outer = Outer { inner };
        }
    "#,
    )
    .cursor_on("inner = Inner")
    .expect_move() // inner is moved into _outer
    .run();
}
