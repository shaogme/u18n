use u18n_ast::Span;

struct RelatedDiagnostic {
    path: String,
    source: String,
    start: usize,
    end: usize,
    label: String,
}

pub(super) struct Diagnostic {
    path: String,
    source: String,
    start: usize,
    end: usize,
    message: String,
    related: Vec<RelatedDiagnostic>,
}

impl Diagnostic {
    pub(super) fn at(path: &str, source: &str, start: usize, end: usize, message: String) -> Self {
        Self {
            path: path.to_string(),
            source: source.to_string(),
            start,
            end,
            message,
            related: Vec::new(),
        }
    }

    pub(super) fn from_span(path: &str, source: &str, span: Span, message: String) -> Self {
        Self::at(
            path,
            source,
            span.start as usize,
            span.end as usize,
            message,
        )
    }

    pub(super) fn render(&self) -> String {
        let mut rendered = Self::render_parts(
            &self.path,
            &self.source,
            self.start,
            self.end,
            &self.message,
        );
        for related in &self.related {
            rendered.push_str("\nrelated: ");
            rendered.push_str(&Self::render_parts(
                &related.path,
                &related.source,
                related.start,
                related.end,
                &related.label,
            ));
        }
        rendered
    }

    pub(super) fn render_owned(self) -> String {
        self.render()
    }

    pub(super) fn with_related_span(
        mut self,
        path: &str,
        source: &str,
        span: Span,
        label: &str,
    ) -> Self {
        self.related.push(RelatedDiagnostic {
            path: path.to_string(),
            source: source.to_string(),
            start: span.start as usize,
            end: span.end as usize,
            label: label.to_string(),
        });
        self
    }

    fn render_parts(path: &str, source: &str, start: usize, end: usize, message: &str) -> String {
        let bounded_start = start.min(source.len());
        let (line, line_start) = Self::line_position(source.as_bytes(), bounded_start);
        let column = source
            .get(line_start..bounded_start)
            .map_or(1, |prefix| prefix.chars().count() + 1);
        let line_end = Self::line_end(source.as_bytes(), bounded_start);
        let excerpt = source.get(line_start..line_end).unwrap_or("");
        let width = source
            .get(bounded_start..end.min(source.len()))
            .map_or(1, |selected| selected.chars().count().max(1));
        format!(
            "{path}:{line}:{column}: {message}\n  {excerpt}\n  {}{}",
            " ".repeat(column.saturating_sub(1)),
            "^".repeat(width)
        )
    }

    fn line_position(bytes: &[u8], position: usize) -> (usize, usize) {
        let mut line = 1usize;
        let mut line_start = 0usize;
        let mut cursor = 0usize;
        while cursor < position {
            match bytes.get(cursor).copied() {
                Some(b'\r') if bytes.get(cursor + 1) == Some(&b'\n') => {
                    cursor += 2;
                    line += 1;
                    line_start = cursor;
                }
                Some(b'\r' | b'\n') => {
                    cursor += 1;
                    line += 1;
                    line_start = cursor;
                }
                Some(_) => cursor += 1,
                None => break,
            }
        }
        (line, line_start)
    }

    fn line_end(bytes: &[u8], position: usize) -> usize {
        bytes
            .get(position..)
            .and_then(|suffix| {
                suffix
                    .iter()
                    .position(|byte| *byte == b'\r' || *byte == b'\n')
            })
            .map_or(bytes.len(), |offset| position + offset)
    }
}

#[cfg(test)]
mod tests {
    use super::Diagnostic;

    #[test]
    fn reports_the_same_line_and_column_for_all_source_line_endings() {
        for source in [
            "header\nmessage x",
            "header\r\nmessage x",
            "header\rmessage x",
        ] {
            let start = source.find('x').unwrap_or_default();
            let diagnostic = Diagnostic::at("locale.u18n", source, start, start + 1, "bad".into());

            assert!(diagnostic.render().starts_with("locale.u18n:2:9: bad"));
        }
    }
}
