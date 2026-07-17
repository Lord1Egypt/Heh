//! Diagnostics (SPEC §15): stable code, position, source line, caret.
//! Codes are append-only forever — the registry lives in docs/DIAGNOSTICS.md.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diag {
    pub code: &'static str,
    pub msg: String,
    /// 1-based line and column (columns count Unicode scalar values).
    pub line: u32,
    pub col: u32,
}

impl Diag {
    pub fn render(&self, file: &str, source: &str) -> String {
        let line_txt = source
            .lines()
            .nth(self.line as usize - 1)
            .unwrap_or("")
            .trim_end_matches('\r');
        let n = self.line.to_string();
        let pad = " ".repeat(n.len());
        let caret_pad = " ".repeat(self.col.saturating_sub(1) as usize);
        format!(
            "error[{code}]: {msg}\n{pad}--> {file}:{line}:{col}\n{pad} |\n{n} | {src}\n{pad} | {caret_pad}^",
            code = self.code,
            msg = self.msg,
            line = self.line,
            col = self.col,
            src = line_txt,
        )
    }
}
