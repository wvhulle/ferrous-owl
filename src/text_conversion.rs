use crate::models::Loc;
#[must_use]
pub fn index_to_line_char(s: &str, idx: Loc) -> (u32, u32) {
    let mut line = 0;
    let mut col = 0;
    // it seems that the compiler is ignoring CR
    for (i, c) in s.replace('\r', "").chars().enumerate() {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "source files are typically less than 2^32 characters"
        )]
        if idx == Loc::from(u32::try_from(i).unwrap_or(u32::MAX)) {
            return (line, col);
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else if c != '\r' {
            col += 1;
        }
    }
    (0, 0)
}
#[must_use]
pub fn line_char_to_index(s: &str, mut line: u32, char: u32) -> u32 {
    let mut col = 0;
    // it seems that the compiler is ignoring CR
    for (i, c) in s.replace('\r', "").chars().enumerate() {
        if line == 0 && col == char {
            #[allow(
                clippy::cast_possible_truncation,
                reason = "source files are typically less than 2^32 characters"
            )]
            return u32::try_from(i).unwrap_or(0);
        }
        if c == '\n' && 0 < line {
            line -= 1;
            col = 0;
        } else if c != '\r' {
            col += 1;
        }
    }
    0
}
