#![allow(unused)]

use std::rc::Rc;
use std::ops::Range;
use std::path::PathBuf;
use std::fmt;
use std::io::Write;

use url::Url;
use codespan::{FileMap, FileName, ByteOffset, Span};
use codespan_reporting::{Diagnostic, Label, LabelStyle, Severity};
use codespan_reporting::termcolor::{ColorChoice, StandardStream};

use crate::resolve::Context;

pub struct Diagnostics<'a> {
    file_map: Rc<FileMap<&'a str>>,
    out: StandardStream,
}

impl<'a> Clone for Diagnostics<'a> {
    fn clone(&self) -> Self {
        Diagnostics {
            file_map: self.file_map.clone(),
            out: Diagnostics::out_stream(),
        }
    }
}

impl<'a> fmt::Debug for Diagnostics<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Diagnostics")
            .field("file_map", &self.file_map)
            .field("out", &"Rc(StandardStream)")
            .finish()
    }
}

pub enum Input {
    File(PathBuf),
    Stdin,
    Url(Url),
}

impl<'a> Diagnostics<'a> {
    fn out_stream() -> StandardStream {
        // TODO: make this configurable
        StandardStream::stderr(ColorChoice::Auto)
    }

    pub fn new(markdown: &'a str, input: Input) -> Diagnostics<'a> {
        let source = match input {
            Input::File(path) => FileName::real(path),
            Input::Stdin => FileName::Virtual("stdin".into()),
            Input::Url(url) => FileName::Virtual(url.as_str().to_owned().into()),
        };
        let file_map = Rc::new(FileMap::new(source, markdown));

        Diagnostics {
            file_map,
            out: Diagnostics::out_stream(),
        }
    }

    pub fn first_line(&self, range: &Range<usize>) -> Range<usize> {
        let start = Span::from_offset(self.file_map.span().start(), ByteOffset(range.start as i64)).end();
        let line = self.file_map.location(start).unwrap().0;
        let line_span = self.file_map.line_span(line).unwrap();
        // get rid of newline
        let len = self.file_map.src_slice(line_span).unwrap().trim_end().len();
        Range {
            start: range.start,
            end: range.start + len,
        }
    }

    fn diagnostic(&mut self, severity: Severity, message: String) -> DiagnosticBuilder<'a, '_> {
        DiagnosticBuilder {
            file_map: &self.file_map,
            out: &mut self.out,
            diagnostics: Vec::new(),
            severity,
            message,
            code: None,
            labels: Vec::new(),
        }
    }

    pub fn bug<S: Into<String>>(&mut self, message: S) -> DiagnosticBuilder<'a, '_> {
        self.diagnostic(Severity::Bug, message.into())
    }
    pub fn error<S: Into<String>>(&mut self, message: S) -> DiagnosticBuilder<'a, '_> {
        self.diagnostic(Severity::Error, message.into())
    }
    pub fn warning<S: Into<String>>(&mut self, message: S) -> DiagnosticBuilder<'a, '_> {
        self.diagnostic(Severity::Warning, message.into())
    }
    pub fn note<S: Into<String>>(&mut self, message: S) -> DiagnosticBuilder<'a, '_> {
        self.diagnostic(Severity::Note, message.into())
    }
    pub fn help<S: Into<String>>(&mut self, message: S) -> DiagnosticBuilder<'a, '_> {
        self.diagnostic(Severity::Help, message.into())
    }
}

#[must_use = "call `emit` to emit the diagnostic"]
pub struct DiagnosticBuilder<'a: 'b, 'b> {
    file_map: &'b FileMap<&'a str>,
    out: &'b mut StandardStream,
    diagnostics: Vec<Diagnostic>,

    severity: Severity,
    message: String,
    code: Option<String>,
    labels: Vec<Label>,
}

impl<'a: 'b, 'b> DiagnosticBuilder<'a, 'b> {
    pub fn emit(self) {
        let Self { file_map, out, mut diagnostics, severity, message, code, labels } = self;
        diagnostics.push(Diagnostic { severity, message, code, labels });

        // ignore output errors, because where would we log them anyway?!
        for diagnostic in diagnostics {
            let _ = codespan_reporting::emit_single(&mut *out, file_map, &diagnostic);
        }
        writeln!(out);
    }

    fn diagnostic(self, new_severity: Severity, new_message: String) -> Self {
        let Self { file_map, out, mut diagnostics, severity, message, code, labels } = self;
        diagnostics.push(Diagnostic { severity, message, code, labels });

        Self {
            file_map,
            out,
            diagnostics,
            severity: new_severity,
            message: new_message,
            code: None,
            labels: Vec::new(),
        }
    }

    pub fn bug<S: Into<String>>(self, message: S) -> Self {
        self.diagnostic(Severity::Bug, message.into())
    }
    pub fn error<S: Into<String>>(self, message: S) -> Self {
        self.diagnostic(Severity::Error, message.into())
    }
    pub fn warning<S: Into<String>>(self, message: S) -> Self {
        self.diagnostic(Severity::Warning, message.into())
    }
    pub fn note<S: Into<String>>(self, message: S) -> Self {
        self.diagnostic(Severity::Note, message.into())
    }
    pub fn help<S: Into<String>>(self, message: S) -> Self {
        self.diagnostic(Severity::Help, message.into())
    }

    pub fn with_error_code(mut self, code: String) -> Self {
        self.code = Some(code);
        self
    }

    /// message can be empty
    pub fn with_section<S: Into<String>>(mut self, range: &Range<usize>, message: S) -> Self {
        let style = if self.labels.len() == 0 {
            LabelStyle::Primary
        } else {
            LabelStyle::Secondary
        };
        let span = self.file_map.span().subspan(ByteOffset(range.start as i64), ByteOffset(range.end as i64));
        let message = message.into();
        let message = if message.len() > 0 {
            Some(message)
        } else {
            None
        };
        self.labels.push(Label { span, message, style });
        self
    }
}