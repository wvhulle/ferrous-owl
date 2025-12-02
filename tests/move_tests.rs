#![feature(rustc_private)]

//! Tests for move decoration detection.

use ferrous_owl::test::TestCase;
#[test]
fn move_to_drop() {
    TestCase::new(
        "move_to_drop",
        r#"
        fn test() {
            let s = String::new();
            drop(s);
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_move()
    .run();
}

#[test]
fn move_to_function() {
    TestCase::new(
        "move_to_function",
        r#"
        fn consume(_s: String) {}

        fn test() {
            let s = String::from("hello");
            consume(s);
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_move()
    .run();
}

#[test]
fn move_into_vec() {
    TestCase::new(
        "move_into_vec",
        r#"
        fn test() {
            let s = String::new();
            let mut v = Vec::new();
            v.push(s);
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_move()
    .run();
}

#[test]
fn move_into_option() {
    TestCase::new(
        "move_into_option",
        r#"
        fn test() {
            let s = String::new();
            let _opt = Some(s);
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_move()
    .run();
}

#[test]
fn move_into_result() {
    TestCase::new(
        "move_into_result",
        r#"
        fn test() {
            let s = String::new();
            let _res: Result<String, ()> = Ok(s);
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_move()
    .run();
}

#[test]
fn move_into_box() {
    TestCase::new(
        "move_into_box",
        r#"
        fn test() {
            let s = String::new();
            let _b = Box::new(s);
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_move()
    .run();
}

#[test]
fn move_return_value() {
    TestCase::new(
        "move_return_value",
        r#"
        fn test() -> String {
            let s = String::from("hello");
            s
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_move()
    .run();
}

#[test]
fn move_struct_field() {
    TestCase::new(
        "move_struct_field",
        r#"
        struct Wrapper { inner: String }

        fn test() {
            let s = String::new();
            let _w = Wrapper { inner: s };
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_move()
    .run();
}

#[test]
fn move_tuple() {
    TestCase::new(
        "move_tuple",
        r#"
        fn test() {
            let s = String::new();
            let _t = (1, s);
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_move()
    .run();
}

#[test]
fn move_closure_capture() {
    // With `move` keyword but only using s.len(), Rust may optimize to borrow
    // since len() only needs &self. The actual decoration is imm-borrow.
    TestCase::new(
        "move_closure_capture",
        r#"
        fn test() {
            let s = String::new();
            let f = move || s.len();
            let _ = f();
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_imm_borrow()
    .run();
}

#[test]
fn move_assignment() {
    TestCase::new(
        "move_assignment",
        r#"
        fn test() {
            let s = String::new();
            let _t = s;
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_move()
    .run();
}

#[test]
fn move_match_arm() {
    TestCase::new(
        "move_match_arm",
        r#"
        fn test() {
            let s = Some(String::new());
            match s {
                Some(inner) => drop(inner),
                None => {}
            }
        }
    "#,
    )
    .cursor_on("s = Some")
    .expect_move()
    .run();
}

#[test]
fn move_if_let() {
    TestCase::new(
        "move_if_let",
        r#"
        fn test() {
            let s = Some(String::new());
            if let Some(inner) = s {
                drop(inner);
            }
        }
    "#,
    )
    .cursor_on("s = Some")
    .expect_move()
    .run();
}

#[test]
fn move_for_loop() {
    TestCase::new(
        "move_for_loop",
        r#"
        fn test() {
            let v = vec![String::new(), String::new()];
            for s in v {
                drop(s);
            }
        }
    "#,
    )
    .cursor_on("v = vec!")
    .expect_move()
    .run();
}
