#![feature(rustc_private)]

//! Tests for mutable borrow decoration detection.

use ferrous_owl::test::TestCase;
#[test]
fn mut_borrow_push() {
    TestCase::new(
        "mut_borrow_push",
        r#"
        fn test() {
            let mut v = Vec::new();
            v.push(1);
        }
    "#,
    )
    .cursor_on("v = Vec")
    .expect_mut_borrow()
    .run();
}

#[test]
fn mut_borrow_reference() {
    TestCase::new(
        "mut_borrow_reference",
        r#"
        fn test() {
            let mut s = String::from("hello");
            let _r = &mut s;
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_mut_borrow()
    .run();
}

#[test]
fn mut_borrow_clear() {
    TestCase::new(
        "mut_borrow_clear",
        r#"
        fn test() {
            let mut v = vec![1, 2, 3];
            v.clear();
        }
    "#,
    )
    .cursor_on("v = vec!")
    .expect_mut_borrow()
    .run();
}

#[test]
fn mut_borrow_pop() {
    TestCase::new(
        "mut_borrow_pop",
        r#"
        fn test() {
            let mut v = vec![1, 2, 3];
            let _ = v.pop();
        }
    "#,
    )
    .cursor_on("v = vec!")
    .expect_mut_borrow()
    .run();
}

#[test]
fn mut_borrow_push_str() {
    TestCase::new(
        "mut_borrow_push_str",
        r#"
        fn test() {
            let mut s = String::new();
            s.push_str("hello");
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_mut_borrow()
    .run();
}

#[test]
fn mut_borrow_extend() {
    TestCase::new(
        "mut_borrow_extend",
        r#"
        fn test() {
            let mut v = Vec::new();
            v.extend([1, 2, 3]);
        }
    "#,
    )
    .cursor_on("v = Vec")
    .expect_mut_borrow()
    .run();
}

#[test]
fn mut_borrow_insert() {
    TestCase::new(
        "mut_borrow_insert",
        r#"
        fn test() {
            let mut v = vec![1, 3];
            v.insert(1, 2);
        }
    "#,
    )
    .cursor_on("v = vec!")
    .expect_mut_borrow()
    .run();
}

#[test]
fn mut_borrow_remove() {
    TestCase::new(
        "mut_borrow_remove",
        r#"
        fn test() {
            let mut v = vec![1, 2, 3];
            let _ = v.remove(0);
        }
    "#,
    )
    .cursor_on("v = vec!")
    .expect_mut_borrow()
    .run();
}

#[test]
fn mut_borrow_sort() {
    TestCase::new(
        "mut_borrow_sort",
        r#"
        fn test() {
            let mut v = vec![3, 1, 2];
            v.sort();
        }
    "#,
    )
    .cursor_on("v = vec!")
    .expect_mut_borrow()
    .run();
}

#[test]
fn mut_borrow_truncate() {
    TestCase::new(
        "mut_borrow_truncate",
        r#"
        fn test() {
            let mut s = String::from("hello");
            s.truncate(3);
        }
    "#,
    )
    .cursor_on("s = String")
    .expect_mut_borrow()
    .run();
}

#[test]
fn mut_borrow_iter_mut() {
    TestCase::new(
        "mut_borrow_iter_mut",
        r#"
        fn test() {
            let mut v = vec![1, 2, 3];
            for x in &mut v {
                *x += 1;
            }
        }
    "#,
    )
    .cursor_on("v = vec!")
    .expect_mut_borrow()
    .run();
}

#[test]
fn mut_borrow_swap() {
    TestCase::new(
        "mut_borrow_swap",
        r#"
        fn test() {
            let mut v = vec![1, 2, 3];
            v.swap(0, 2);
        }
    "#,
    )
    .cursor_on("v = vec!")
    .expect_mut_borrow()
    .run();
}

#[test]
fn mut_borrow_reverse() {
    TestCase::new(
        "mut_borrow_reverse",
        r#"
        fn test() {
            let mut v = vec![1, 2, 3];
            v.reverse();
        }
    "#,
    )
    .cursor_on("v = vec!")
    .expect_mut_borrow()
    .run();
}

#[test]
fn mut_borrow_retain() {
    TestCase::new(
        "mut_borrow_retain",
        r#"
        fn test() {
            let mut v = vec![1, 2, 3, 4];
            v.retain(|x| x % 2 == 0);
        }
    "#,
    )
    .cursor_on("v = vec!")
    .expect_mut_borrow()
    .run();
}

#[test]
fn mut_borrow_dedup() {
    TestCase::new(
        "mut_borrow_dedup",
        r#"
        fn test() {
            let mut v = vec![1, 1, 2, 2, 3];
            v.dedup();
        }
    "#,
    )
    .cursor_on("v = vec!")
    .expect_mut_borrow()
    .run();
}
