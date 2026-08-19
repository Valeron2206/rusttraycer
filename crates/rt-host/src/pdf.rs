//! Minimal PDF writer for `artifact.export format=pdf`. No third-party PDF crate.

fn pdf_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\\' => out.push_str("\\\\"),
            c if c.is_ascii() && !c.is_control() => out.push(c),
            c if c.is_ascii_whitespace() => out.push(' '),
            _ => out.push('?'),
        }
    }
    out
}

fn wrap_line(s: &str, max: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        if word.len() > max {
            if !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
            }
            for chunk in word.as_bytes().chunks(max) {
                lines.push(String::from_utf8_lossy(chunk).into_owned());
            }
            continue;
        }
        if cur.is_empty() {
            cur.push_str(word);
        } else if cur.len() + 1 + word.len() > max {
            lines.push(std::mem::take(&mut cur));
            cur.push_str(word);
        } else {
            cur.push(' ');
            cur.push_str(word);
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

fn content_lines(title: &str, body: &str) -> Vec<(f32, String)> {
    const MAX_CHARS: usize = 86;
    const MAX_LINES: usize = 48;
    let mut out = Vec::new();
    let title = title.trim();
    if !title.is_empty() {
        for line in wrap_line(title, MAX_CHARS) {
            out.push((16.0, line));
        }
    }
    for raw in body.lines() {
        let t = raw.trim_end();
        if t.is_empty() {
            out.push((12.0, String::new()));
            continue;
        }
        for line in wrap_line(t, MAX_CHARS) {
            out.push((12.0, line));
        }
    }
    if out.is_empty() {
        out.push((12.0, String::new()));
    }
    out.truncate(MAX_LINES);
    out
}

fn build_stream(lines: &[(f32, String)]) -> String {
    let mut s = String::from("BT\n");
    let mut y = 720.0_f32;
    let mut first = true;
    let mut last_size = 12.0_f32;
    for (size, text) in lines {
        if first {
            s.push_str(&format!("/F1 {size:.0} Tf\n72 {y:.0} Td\n"));
            first = false;
            last_size = *size;
        } else {
            if (*size - last_size).abs() > f32::EPSILON {
                s.push_str(&format!("/F1 {size:.0} Tf\n"));
                last_size = *size;
            }
            let step = if text.is_empty() { 10.0 } else { size + 4.0 };
            y -= step;
            s.push_str(&format!("0 -{step:.0} Td\n"));
        }
        if !text.is_empty() {
            s.push_str(&format!("({}) Tj\n", pdf_escape(text)));
        }
        if y < 48.0 {
            break;
        }
    }
    s.push_str("ET\n");
    s
}

fn xref_entry(offset: usize, gen: u16, used: bool) -> String {
    let flag = if used { 'n' } else { 'f' };
    format!("{offset:010} {gen:05} {flag} \n")
}

/// Build a one-page PDF whose bytes start with `%PDF`. Title/body from markdown.
pub fn render_markdown_pdf(title: &str, body: &str) -> Result<Vec<u8>, String> {
    let stream = build_stream(&content_lines(title, body));
    let stream_len = stream.len();

    let obj1 = "<< /Type /Catalog /Pages 2 0 R >>".to_string();
    let obj2 = "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string();
    let obj3 = "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
/Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>"
        .to_string();
    let obj4 = format!("<< /Length {stream_len} >>\nstream\n{stream}endstream");
    let obj5 = "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string();
    let objects = [obj1, obj2, obj3, obj4, obj5];

    let header = "%PDF-1.4\n";
    let mut body_bytes = header.as_bytes().to_vec();
    let mut offsets = [0usize; 6];
    for (i, obj) in objects.iter().enumerate() {
        let id = i + 1;
        offsets[id] = body_bytes.len();
        body_bytes.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
        body_bytes.extend_from_slice(obj.as_bytes());
        body_bytes.extend_from_slice(b"\nendobj\n");
    }
    let xref_at = body_bytes.len();
    let mut xref = format!("xref\n0 6\n{}", xref_entry(0, 65535, false));
    for offset in offsets.iter().skip(1) {
        xref.push_str(&xref_entry(*offset, 0, true));
    }
    body_bytes.extend_from_slice(xref.as_bytes());
    let trailer = format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n");
    body_bytes.extend_from_slice(trailer.as_bytes());
    if !body_bytes.starts_with(b"%PDF") {
        return Err("pdf writer produced a document without %PDF header".into());
    }
    Ok(body_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_starts_with_pdf_header_and_includes_title() {
        let pdf = render_markdown_pdf("Auth spec", "# Auth\nbody line").expect("pdf");
        assert!(pdf.starts_with(b"%PDF"), "header: {:?}", &pdf[..8]);
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("Auth spec"), "{text}");
        assert!(text.contains("endobj"));
        assert!(text.contains("%%EOF"));
    }

    #[test]
    fn escape_parens_and_non_ascii() {
        let pdf = render_markdown_pdf("A (B)", "café").expect("pdf");
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("A \\(B\\)"));
        assert!(text.contains("caf?"));
    }
}
