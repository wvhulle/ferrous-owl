#![feature(rustc_private)]

use ferrous_owl::test::{DecoKind, TestCase};
#[test]
fn call_string_new() {
    TestCase::new(
        "call_string_new",
        r#"
        fn test() {
            let s = String::new();
            drop(s);
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_call()
    .run();
}

#[test]
fn call_string_from() {
    TestCase::new(
        "call_string_from",
        r#"
        fn test() {
            let s = String::from("hello");
            drop(s);
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_call()
    .run();
}

#[test]
fn call_vec_new() {
    TestCase::new(
        "call_vec_new",
        r#"
        fn test() {
            let v = Vec::<i32>::new();
            drop(v);
        }
    "#,
    )
    .cursor_on("v = Vec")
    .expect_call()
    .run();
}

#[test]
fn call_vec_macro() {
    // vec![] macro expands to code that moves the vector, not a direct call
    // decoration
    TestCase::new(
        "call_vec_macro",
        r#"
        fn test() {
            let v = vec![1, 2, 3];
            drop(v);
        }
    "#,
    )
    .cursor_on("v = vec!")
    .expect_move_at("v") // The macro produces a move, not a call
    .run();
}

#[test]
fn call_box_new() {
    TestCase::new(
        "call_box_new",
        r#"
        fn test() {
            let b = Box::new(42);
            drop(b);
        }
    "#,
    )
    .cursor_on("b = Box")
    .expect_call()
    .run();
}

#[test]
fn call_option_some() {
    // Some is an enum variant constructor, not a function call
    // It doesn't produce a Call decoration - just verify no panics and no
    // unexpected decos
    TestCase::new(
        "call_option_some",
        r#"
        fn test() {
            let opt = Some(42);
            drop(opt);
        }
    "#,
    )
    .cursor_on("opt = Some")
    .forbid(DecoKind::Call) // Confirm no call decoration
    .run();
}

#[test]
fn call_result_ok() {
    // Ok is an enum variant constructor, not a function call
    TestCase::new(
        "call_result_ok",
        r#"
        fn test() {
            let res: Result<i32, ()> = Ok(42);
            drop(res);
        }
    "#,
    )
    .cursor_on("res:")
    .forbid(DecoKind::Call) // Confirm no call decoration
    .run();
}

#[test]
fn call_hashmap_new() {
    TestCase::new(
        "call_hashmap_new",
        r#"
        use std::collections::HashMap;

        fn test() {
            let m = HashMap::<String, i32>::new();
            drop(m);
        }
    "#,
    )
    .cursor_on("m = HashMap")
    .expect_call()
    .run();
}

#[test]
fn call_custom_function() {
    TestCase::new(
        "call_custom_function",
        r#"
        fn create_string() -> String {
            String::from("hello")
        }

        fn test() {
            let s = create_string();
            drop(s);
        }
    "#,
    )
    .cursor_on("s = create")
    .expect_call()
    .run();
}

#[test]
fn call_to_string() {
    TestCase::new(
        "call_to_string",
        r#"
        fn test() {
            let n = 42;
            let s = n.to_string();
            drop(s);
        }
    "#,
    )
    .cursor_on("s = n")
    .expect_call()
    .run();
}

#[test]
fn call_default() {
    TestCase::new(
        "call_default",
        r#"
        fn test() {
            let s: String = Default::default();
            drop(s);
        }
    "#,
    )
    .cursor_on("s:")
    .expect_call()
    .run();
}

#[test]
fn call_collect() {
    TestCase::new(
        "call_collect",
        r#"
        fn test() {
            let v: Vec<i32> = (0..5).collect();
            drop(v);
        }
    "#,
    )
    .cursor_on("v:")
    .expect_call()
    .run();
}
