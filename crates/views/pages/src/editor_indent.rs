/// A line's leading whitespace as nesting steps: two spaces or one tab per
/// step, and a leftover odd space belongs to the TEXT — discarding it would
/// eat a byte of pasted prose on every save.
pub fn split_indent(raw: &str) -> (usize, &str) {
    let mut steps = 0;
    let mut pending = 0;
    let mut consumed = 0;
    for byte in raw.bytes() {
        match byte {
            b' ' => {
                pending += 1;
                if pending == 2 {
                    steps += 1;
                    pending = 0;
                }
            }
            b'\t' => {
                steps += 1;
                pending = 0;
            }
            _ => break,
        }
        consumed += 1;
    }
    (steps, &raw[consumed - pending..])
}
