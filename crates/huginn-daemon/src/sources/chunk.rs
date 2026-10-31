//! The source chunker: line windows, no overlap. A chunk is the unit the index
//! holds and a search answers, so its line range is a citation.

/// Bumped whenever the windows or the embedded text would change for the same
/// blob: it is part of every chunk's identity and of the collection's label, so
/// a change rebuilds the collection instead of mixing two chunkings.
pub const CHUNKER_VERSION: u32 = 1;

/// The most lines a chunk holds.
pub const CHUNK_LINES_MAX: usize = 48;
/// The most characters a chunk's text holds.
pub const CHUNK_CHARS_MAX: usize = 1_400;

/// One window of a file: its 1-based inclusive line range and its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    pub line_start: u32,
    pub line_end: u32,
    pub text: String,
}

/// The file cut into windows of at most `CHUNK_LINES_MAX` lines or
/// `CHUNK_CHARS_MAX` characters. CRLF is read as LF and a trailing newline ends
/// the last line without starting another. A line longer than the character
/// bound is a chunk of its own, cut at the bound on a character boundary.
/// A window of blank lines holds nothing to find and is dropped.
pub fn chunk(text: &str) -> Vec<Chunk> {
    let normalised = text.replace("\r\n", "\n");
    let body = normalised.strip_suffix('\n').unwrap_or(&normalised);
    if body.is_empty() {
        return Vec::new();
    }
    let mut chunks = Vec::new();
    let mut window: Vec<&str> = Vec::new();
    let mut window_chars = 0_usize;
    let mut window_start = 1_u32;
    let flush = |window: &mut Vec<&str>, window_chars: &mut usize, window_start: u32, chunks: &mut Vec<Chunk>| {
        if !window.is_empty() {
            let joined = window.join("\n");
            if !joined.trim().is_empty() {
                let end = window_start + u32::try_from(window.len()).unwrap_or(u32::MAX) - 1;
                chunks.push(Chunk { line_start: window_start, line_end: end, text: joined });
            }
        }
        window.clear();
        *window_chars = 0;
    };
    for (index, line) in body.split('\n').enumerate() {
        let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
        let length = line.chars().count();
        if length > CHUNK_CHARS_MAX {
            flush(&mut window, &mut window_chars, window_start, &mut chunks);
            let cut: String = line.chars().take(CHUNK_CHARS_MAX).collect();
            chunks.push(Chunk { line_start: number, line_end: number, text: cut });
            continue;
        }
        let joined = window_chars + length + usize::from(!window.is_empty());
        if !window.is_empty() && (window.len() >= CHUNK_LINES_MAX || joined > CHUNK_CHARS_MAX) {
            flush(&mut window, &mut window_chars, window_start, &mut chunks);
        }
        if window.is_empty() {
            window_start = number;
            window_chars = length;
        } else {
            window_chars += length + 1;
        }
        window.push(line);
    }
    flush(&mut window, &mut window_chars, window_start, &mut chunks);
    chunks
}

/// What is embedded for a chunk: where it lives, then what it says. The path
/// is part of the meaning of a line of code.
pub fn embedded_text(repo: &str, path: &str, chunk: &Chunk) -> String {
    format!("{repo}/{path}\n{}", chunk.text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(count: usize) -> String {
        (1..=count).map(|n| format!("line {n}\n")).collect()
    }

    /// Every chunk's range is the lines it holds, counted from one, whatever
    /// the file's line endings or last newline.
    #[test]
    fn chunk_lines_are_exact() {
        for text in [lines(100), lines(100).replace('\n', "\r\n"), lines(100).trim_end().to_owned()] {
            let source: Vec<String> =
                text.replace("\r\n", "\n").trim_end_matches('\n').split('\n').map(str::to_owned).collect();
            let chunks = chunk(&text);
            assert_eq!(chunks.len(), 3, "100 lines at 48 per window");
            let mut next = 1;
            for held in &chunks {
                assert_eq!(held.line_start, next, "no gap and no overlap");
                let expected = source[held.line_start as usize - 1..held.line_end as usize].join("\n");
                assert_eq!(held.text, expected, "lines {}-{}", held.line_start, held.line_end);
                assert!(!held.text.contains('\r'));
                next = held.line_end + 1;
            }
            assert_eq!(next, 101, "every line is in a chunk");
        }
        assert_eq!(chunk("a\n"), vec![Chunk { line_start: 1, line_end: 1, text: "a".into() }]);
        assert!(chunk("").is_empty() && chunk("\n").is_empty() && chunk("\n\n  \n").is_empty());
    }

    /// A window closes at the character bound before it closes at the line
    /// bound, and a line past the bound stands alone, cut on a boundary.
    #[test]
    fn a_window_closes_at_the_character_bound_and_a_long_line_stands_alone() {
        let wide = "x".repeat(700);
        let chunks = chunk(&format!("{wide}\n{wide}\n{wide}\n"));
        let ranges: Vec<(u32, u32)> = chunks.iter().map(|held| (held.line_start, held.line_end)).collect();
        assert_eq!(ranges, [(1, 1), (2, 2), (3, 3)], "two 700-char lines would join to 1,401");
        let fits = chunk(&format!("{}\n{}\n", "y".repeat(699), "y".repeat(700)));
        assert_eq!(fits.len(), 1, "699 + newline + 700 is exactly 1,400");

        let long = "\u{e9}".repeat(CHUNK_CHARS_MAX + 50);
        let chunks = chunk(&format!("before\n{long}\nafter\n"));
        let shown: Vec<(u32, u32, usize)> =
            chunks.iter().map(|held| (held.line_start, held.line_end, held.text.chars().count())).collect();
        assert_eq!(shown, [(1, 1, 6), (2, 2, CHUNK_CHARS_MAX), (3, 3, 5)]);
        assert!(chunks[1].text.chars().all(|c| c == '\u{e9}'));
    }

    #[test]
    fn the_embedded_text_names_the_file_and_the_payload_text_does_not() {
        let held = Chunk { line_start: 3, line_end: 4, text: "fn a() {}".into() };
        assert_eq!(embedded_text("Huginn", "src/a.rs", &held), "Huginn/src/a.rs\nfn a() {}");
    }
}
