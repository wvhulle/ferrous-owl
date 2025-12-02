//! Negative tests: verify Copy types don't generate move decorations.

use owl_test::TestCase;

#[test]
fn copy_integer() {
    TestCase::new("copy_integer", r#"
        fn test() {
            let x = 42;
            let _y = x;
            let _z = x;
        }
    "#)
    .cursor_on("x = 42")
    .forbid_move()
    .run();
}

#[test]
fn copy_float() {
    TestCase::new("copy_float", r#"
        fn test() {
            let x = 3.14;
            let _y = x;
            let _z = x;
        }
    "#)
    .cursor_on("x = 3.14")
    .forbid_move()
    .run();
}

#[test]
fn copy_bool() {
    TestCase::new("copy_bool", r#"
        fn test() {
            let b = true;
            let _c = b;
            let _d = b;
        }
    "#)
    .cursor_on("b = true")
    .forbid_move()
    .run();
}

#[test]
fn copy_char() {
    TestCase::new("copy_char", r#"
        fn test() {
            let c = 'a';
            let _d = c;
            let _e = c;
        }
    "#)
    .cursor_on("c = 'a'")
    .forbid_move()
    .run();
}

#[test]
fn copy_tuple_of_primitives() {
    TestCase::new("copy_tuple_of_primitives", r#"
        fn test() {
            let t = (1, 2, 3);
            let _u = t;
            let _v = t;
        }
    "#)
    .cursor_on("t = (1,")
    .forbid_move()
    .run();
}

#[test]
fn copy_array_of_primitives() {
    TestCase::new("copy_array_of_primitives", r#"
        fn test() {
            let arr = [1, 2, 3];
            let _brr = arr;
            let _crr = arr;
        }
    "#)
    .cursor_on("arr = [1,")
    .forbid_move()
    .run();
}

#[test]
fn copy_reference() {
    TestCase::new("copy_reference", r#"
        fn test() {
            let s = String::from("hello");
            let r = &s;
            let _r2 = r;
            let _r3 = r;
        }
    "#)
    .cursor_on("r = &s")
    .forbid_move()
    .run();
}

#[test]
fn copy_unit() {
    TestCase::new("copy_unit", r#"
        fn test() {
            let u = ();
            let _v = u;
            let _w = u;
        }
    "#)
    .cursor_on("u = ()")
    .forbid_move()
    .run();
}

#[test]
fn copy_option_primitive() {
    TestCase::new("copy_option_primitive", r#"
        fn test() {
            let opt = Some(42);
            let _copy1 = opt;
            let _copy2 = opt;
        }
    "#)
    .cursor_on("opt = Some")
    .forbid_move()
    .run();
}

#[test]
fn copy_result_primitives() {
    TestCase::new("copy_result_primitives", r#"
        fn test() {
            let res: Result<i32, i32> = Ok(42);
            let _copy1 = res;
            let _copy2 = res;
        }
    "#)
    .cursor_on("res:")
    .forbid_move()
    .run();
}

#[test]
fn copy_derived_struct() {
    TestCase::new("copy_derived_struct", r#"
        #[derive(Clone, Copy)]
        struct Point { x: i32, y: i32 }

        fn test() {
            let p = Point { x: 1, y: 2 };
            let _q = p;
            let _r = p;
        }
    "#)
    .cursor_on("p = Point")
    .forbid_move()
    .run();
}

#[test]
fn copy_function_pointer() {
    TestCase::new("copy_function_pointer", r#"
        fn add(a: i32, b: i32) -> i32 { a + b }

        fn test() {
            let f: fn(i32, i32) -> i32 = add;
            let _g = f;
            let _h = f;
        }
    "#)
    .cursor_on("f:")
    .forbid_move()
    .run();
}
