//! `laconic-check mcp`: an MCP server on standard input and output that exposes `lint` and
//! `preserve` as tools. It speaks newline-delimited JSON-RPC 2.0 through the small JSON
//! reader and writer below, so the crate keeps no dependencies.

use crate::{lint, preserve};

/// The longest text argument, in characters.
const MAX_INPUT: usize = 200_000;
/// The deepest nesting a request may use; deeper input is a parse error, not a stack overflow.
const MAX_DEPTH: usize = 64;

const LINT: &str = "Flag Australian English and house-style candidates in Markdown prose: em dashes, US spellings, 'while' and 'among', double quotation marks, possible serial commas, numbers 2 to 20 written as words, dates and times in non-Style Manual forms, weekdays that do not match their date, Latin abbreviations, chatbot closers, stock phrases and chat-tool residue. Code, URLs and quoted text are skipped. A finding is a reason to look, not proof of a defect.";
const PRESERVE: &str = "Compare an original and an edited text and report numbers, sizes and units, dated months, inline code, URLs and meaning words and bounds (not, never, must, may, will, if, before, likely, some, all, only, unless, up to, about 50% and similar) that the edit dropped, then, as advice, those it added (an added figure or obligation can be invented) and any names to check. A clean result rules out these losses only; it does not prove the meaning survived.";

/// A JSON value. A number keeps its text, so a request's `id` is echoed exactly.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    /// A member of an object; of repeated keys, the last, as JavaScript reads them.
    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(members) => members.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }
}

/// One JSON text (RFC 8259), or `None` when it is not valid JSON or nests too deeply. A lone
/// surrogate in a `\u` escape becomes U+FFFD, since a Rust string cannot hold one.
pub fn parse(text: &str) -> Option<Json> {
    let mut p = Parser {
        s: text.as_bytes(),
        i: 0,
    };
    let value = p.value(0)?;
    p.ws();
    (p.i == p.s.len()).then_some(value)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while matches!(self.s.get(self.i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    fn eat(&mut self, b: u8) -> bool {
        let found = self.s.get(self.i) == Some(&b);
        self.i += usize::from(found);
        found
    }

    fn word(&mut self, word: &str, value: Json) -> Option<Json> {
        self.s[self.i..].starts_with(word.as_bytes()).then(|| {
            self.i += word.len();
            value
        })
    }

    fn value(&mut self, depth: usize) -> Option<Json> {
        if depth > MAX_DEPTH {
            return None;
        }
        self.ws();
        match *self.s.get(self.i)? {
            b'n' => self.word("null", Json::Null),
            b't' => self.word("true", Json::Bool(true)),
            b'f' => self.word("false", Json::Bool(false)),
            b'"' => self.string().map(Json::String),
            b'-' | b'0'..=b'9' => self.number(),
            b'[' => {
                self.i += 1;
                let mut items = Vec::new();
                self.ws();
                if self.eat(b']') {
                    return Some(Json::Array(items));
                }
                loop {
                    items.push(self.value(depth + 1)?);
                    self.ws();
                    if self.eat(b']') {
                        return Some(Json::Array(items));
                    }
                    if !self.eat(b',') {
                        return None;
                    }
                }
            }
            b'{' => {
                self.i += 1;
                let mut members = Vec::new();
                self.ws();
                if self.eat(b'}') {
                    return Some(Json::Object(members));
                }
                loop {
                    self.ws();
                    if self.s.get(self.i) != Some(&b'"') {
                        return None;
                    }
                    let key = self.string()?;
                    self.ws();
                    if !self.eat(b':') {
                        return None;
                    }
                    members.push((key, self.value(depth + 1)?));
                    self.ws();
                    if self.eat(b'}') {
                        return Some(Json::Object(members));
                    }
                    if !self.eat(b',') {
                        return None;
                    }
                }
            }
            _ => None,
        }
    }

    fn digits(&mut self) -> bool {
        let start = self.i;
        while self.s.get(self.i).is_some_and(u8::is_ascii_digit) {
            self.i += 1;
        }
        self.i > start
    }

    fn number(&mut self) -> Option<Json> {
        let start = self.i;
        self.eat(b'-');
        if !self.eat(b'0') && !self.digits() {
            return None;
        }
        if self.eat(b'.') && !self.digits() {
            return None;
        }
        if self.eat(b'e') || self.eat(b'E') {
            if !self.eat(b'+') {
                self.eat(b'-');
            }
            if !self.digits() {
                return None;
            }
        }
        // Only ASCII bytes were consumed, so the slice is whole characters.
        Some(Json::Number(
            String::from_utf8_lossy(&self.s[start..self.i]).into_owned(),
        ))
    }

    /// A string; `self.i` is at its opening quote.
    fn string(&mut self) -> Option<String> {
        self.i += 1;
        let mut out = String::new();
        loop {
            let start = self.i;
            while self
                .s
                .get(self.i)
                .is_some_and(|&b| b != b'"' && b != b'\\' && b >= 0x20)
            {
                self.i += 1;
            }
            // The text came from a &str and the run stops only at ASCII bytes.
            out.push_str(std::str::from_utf8(&self.s[start..self.i]).ok()?);
            match *self.s.get(self.i)? {
                b'"' => {
                    self.i += 1;
                    return Some(out);
                }
                b'\\' => {
                    self.i += 1;
                    let c = match *self.s.get(self.i)? {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            self.i += 1;
                            let c = self.escaped_char()?;
                            out.push(c);
                            continue;
                        }
                        _ => return None,
                    };
                    self.i += 1;
                    out.push(c);
                }
                // An unescaped control character.
                _ => return None,
            }
        }
    }

    /// The character of a `\u` escape (`self.i` is after the `u`), joining a surrogate pair.
    fn escaped_char(&mut self) -> Option<char> {
        let high = self.hex4()?;
        if (0xD800..0xDC00).contains(&high) && self.s[self.i..].starts_with(b"\\u") {
            let after_high = self.i;
            self.i += 2;
            let low = self.hex4()?;
            if (0xDC00..0xE000).contains(&low) {
                return char::from_u32(0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00));
            }
            self.i = after_high;
        }
        Some(char::from_u32(high).unwrap_or('\u{FFFD}'))
    }

    fn hex4(&mut self) -> Option<u32> {
        let digits = self.s.get(self.i..self.i + 4)?;
        if !digits.iter().all(u8::is_ascii_hexdigit) {
            return None;
        }
        self.i += 4;
        u32::from_str_radix(std::str::from_utf8(digits).ok()?, 16).ok()
    }
}

/// `value` as compact JSON text.
pub fn to_string(value: &Json) -> String {
    let mut out = String::new();
    write(value, &mut out);
    out
}

fn write(value: &Json, out: &mut String) {
    match value {
        Json::Null => out.push_str("null"),
        Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Json::Number(n) => out.push_str(n),
        Json::String(s) => write_string(s, out),
        Json::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write(item, out);
            }
            out.push(']');
        }
        Json::Object(members) => {
            out.push('{');
            for (i, (key, item)) in members.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(key, out);
                out.push(':');
                write(item, out);
            }
            out.push('}');
        }
    }
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn object(members: Vec<(&str, Json)>) -> Json {
    Json::Object(
        members
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    )
}

fn text(s: &str) -> Json {
    Json::String(s.to_string())
}

fn tool(name: &str, description: &str, properties: &[(&str, &str)]) -> Json {
    let schema = |description: &str| {
        object(vec![
            ("type", text("string")),
            ("description", text(description)),
        ])
    };
    object(vec![
        ("name", text(name)),
        ("description", text(description)),
        (
            "inputSchema",
            object(vec![
                ("type", text("object")),
                (
                    "properties",
                    object(properties.iter().map(|(k, d)| (*k, schema(d))).collect()),
                ),
                (
                    "required",
                    Json::Array(properties.iter().map(|(k, _)| text(k)).collect()),
                ),
            ]),
        ),
    ])
}

fn tools() -> Json {
    Json::Array(vec![
        tool(
            "lint",
            LINT,
            &[("text", "The Markdown or plain prose to check.")],
        ),
        tool(
            "preserve",
            PRESERVE,
            &[
                ("original", "The text before editing."),
                ("edited", "The text after editing."),
            ],
        ),
    ])
}

/// A tool's report, or the message of a failed call.
fn call_tool(params: Option<&Json>) -> Result<String, String> {
    let name = params
        .and_then(|p| p.get("name"))
        .and_then(Json::as_str)
        .unwrap_or_default();
    let args: &[(String, Json)] = match params.and_then(|p| p.get("arguments")) {
        None | Some(Json::Null) => &[],
        Some(Json::Object(members)) => members,
        Some(_) => return Err("arguments must be an object".into()),
    };
    for (_, value) in args {
        let Json::String(s) = value else {
            return Err("every argument must be a string".into());
        };
        if s.chars().count() > MAX_INPUT {
            return Err(format!("each text must be at most {MAX_INPUT} characters"));
        }
    }
    let arg = |key: &str| {
        args.iter()
            .rev()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| v.as_str())
    };
    match name {
        "lint" => {
            let text = arg("text").ok_or("text is required")?;
            let lines: Vec<String> = lint::lint(text)
                .iter()
                .map(|f| format!("line {}:{}: {}: {}", f.line, f.column, f.rule, f.message))
                .collect();
            Ok(if lines.is_empty() {
                "No findings.".into()
            } else {
                lines.join("\n")
            })
        }
        "preserve" => {
            let (Some(original), Some(edited)) = (arg("original"), arg("edited")) else {
                return Err("original and edited are required".into());
            };
            Ok(preserve::report(original, edited).lines.join("\n"))
        }
        _ => Err(format!("unknown tool: {name}")),
    }
}

fn reply(id: Json, outcome: Result<Json, (i32, String)>) -> String {
    let body = match outcome {
        Ok(result) => ("result", result),
        Err((code, message)) => (
            "error",
            object(vec![
                ("code", Json::Number(code.to_string())),
                ("message", text(&message)),
            ]),
        ),
    };
    to_string(&object(vec![("jsonrpc", text("2.0")), ("id", id), body]))
}

/// The reply to one request line, or `None` for a notification (a request with no `id`).
pub fn handle_line(line: &str) -> Option<String> {
    let Some(request) = parse(line) else {
        return Some(reply(Json::Null, Err((-32700, "parse error".into()))));
    };
    if !matches!(request, Json::Object(_)) {
        return Some(reply(Json::Null, Err((-32600, "invalid request".into()))));
    }
    let id = request.get("id")?.clone();
    let params = request.get("params");
    let method = request
        .get("method")
        .and_then(Json::as_str)
        .unwrap_or_default();
    let outcome = match method {
        "initialize" => {
            let version = params
                .and_then(|p| p.get("protocolVersion"))
                .and_then(Json::as_str);
            Ok(object(vec![
                ("protocolVersion", text(version.unwrap_or("2025-06-18"))),
                ("capabilities", object(vec![("tools", object(vec![]))])),
                (
                    "serverInfo",
                    object(vec![
                        ("name", text("laconic-check")),
                        ("version", text(env!("CARGO_PKG_VERSION"))),
                    ]),
                ),
            ]))
        }
        "ping" => Ok(object(vec![])),
        "tools/list" => Ok(object(vec![("tools", tools())])),
        "tools/call" => Ok(match call_tool(params) {
            Ok(report) => object(vec![("content", content(&report))]),
            Err(message) => object(vec![
                ("content", content(&message)),
                ("isError", Json::Bool(true)),
            ]),
        }),
        _ => Err((-32601, format!("method not found: {method}"))),
    };
    Some(reply(id, outcome))
}

fn content(s: &str) -> Json {
    Json::Array(vec![object(vec![
        ("type", text("text")),
        ("text", text(s)),
    ])])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(line: &str) -> Json {
        let reply = parse(&handle_line(line).unwrap()).unwrap();
        assert!(reply.get("error").is_none(), "{reply:?}");
        reply.get("result").unwrap().clone()
    }

    fn call(arguments: &str) -> (String, bool) {
        let line =
            format!(r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{arguments}}}"#);
        let r = result(&line);
        let Some(Json::Array(content)) = r.get("content") else {
            panic!("{r:?}")
        };
        let report = content[0]
            .get("text")
            .and_then(Json::as_str)
            .unwrap()
            .to_string();
        (report, r.get("isError") == Some(&Json::Bool(true)))
    }

    #[test]
    fn json_reads_what_rfc_8259_allows_and_nothing_else() {
        let ok = [
            "null",
            " true ",
            "false",
            "0",
            "-0",
            "12",
            "1.5",
            "-2e10",
            "1E+2",
            "3e-7",
            r#""""#,
            r#""a\"\\\/\b\f\n\r\tz""#,
            r#""é😀""#,
            "[]",
            "[1,[2,{}]]",
            r#"{"a":1,"b":[null]}"#,
            "\t{\r\n\"k\" : \"v\" }\r",
        ];
        for text in ok {
            assert!(parse(text).is_some(), "{text}");
        }
        let bad = [
            "",
            "nul",
            "01",
            "1.",
            ".5",
            "-",
            "1e",
            "+1",
            r#""a"#,
            "\"a\u{1}b\"",
            r#""\x""#,
            r#""\u12""#,
            r#""\u+123""#,
            "[1,]",
            "[1 2]",
            "{\"a\"}",
            "{a:1}",
            r#"{"a":1,}"#,
            "1 2",
            "[",
            "NaN",
        ];
        for text in bad {
            assert!(parse(text).is_none(), "{text}");
        }
        assert_eq!(parse(r#""😀""#), Some(Json::String("\u{1F600}".into())));
        assert_eq!(
            parse(r#""\ud83d!""#),
            Some(Json::String("\u{FFFD}!".into()))
        );
        assert_eq!(
            parse(r#""\ude00A""#),
            Some(Json::String("\u{FFFD}A".into()))
        );
        assert_eq!(
            parse(r#"{"a":1,"a":2}"#).unwrap().get("a"),
            Some(&Json::Number("2".into()))
        );
    }

    #[test]
    fn deep_nesting_is_a_parse_error_not_a_stack_overflow() {
        let deep = "[".repeat(100_000) + &"]".repeat(100_000);
        assert!(parse(&deep).is_none());
        let fits = "[".repeat(MAX_DEPTH + 1) + &"]".repeat(MAX_DEPTH + 1);
        assert!(parse(&fits).is_some());
    }

    #[test]
    fn json_text_written_reads_back_the_same() {
        for text in [
            r#"{"s":"q\"b\\n\n\u0001é😀","n":-1.5e3,"a":[true,null,{}]}"#,
            r#""\ud83d""#,
        ] {
            let value = parse(text).unwrap();
            assert_eq!(parse(&to_string(&value)), Some(value));
        }
        assert_eq!(
            to_string(&Json::String("a\u{1f}\"".into())),
            r#""a\u001f\"""#
        );
    }

    #[test]
    fn initialise_echoes_the_protocol_and_reports_this_version() {
        let r = result(
            r#"{"jsonrpc":"2.0","id":"x","method":"initialize","params":{"protocolVersion":"2025-03-26"}}"#,
        );
        assert_eq!(r.get("protocolVersion"), Some(&text("2025-03-26")));
        let info = r.get("serverInfo").unwrap();
        assert_eq!(info.get("version"), Some(&text(env!("CARGO_PKG_VERSION"))));
        let r = result(r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#);
        assert_eq!(r.get("protocolVersion"), Some(&text("2025-06-18")));
    }

    #[test]
    fn replies_echo_the_id_and_notifications_get_none() {
        let reply = handle_line(r#"{"jsonrpc":"2.0","id":1.50,"method":"ping"}"#).unwrap();
        assert_eq!(reply, r#"{"jsonrpc":"2.0","id":1.50,"result":{}}"#);
        assert_eq!(
            handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#),
            None
        );
        let reply = handle_line(r#"{"jsonrpc":"2.0","id":null,"method":"nope"}"#).unwrap();
        assert_eq!(
            reply,
            r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32601,"message":"method not found: nope"}}"#
        );
    }

    #[test]
    fn malformed_lines_get_an_error_with_a_null_id() {
        let reply = handle_line("{not json").unwrap();
        assert_eq!(
            reply,
            r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"parse error"}}"#
        );
        let reply = handle_line("[1]").unwrap();
        assert_eq!(
            reply,
            r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"invalid request"}}"#
        );
    }

    #[test]
    fn tools_list_names_both_tools_and_their_required_arguments() {
        let r = result(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#);
        let Some(Json::Array(tools)) = r.get("tools") else {
            panic!("{r:?}")
        };
        let required: Vec<Json> = tools
            .iter()
            .map(|t| {
                t.get("inputSchema")
                    .and_then(|s| s.get("required"))
                    .unwrap()
                    .clone()
            })
            .collect();
        assert_eq!(tools[0].get("name"), Some(&text("lint")));
        assert_eq!(tools[1].get("name"), Some(&text("preserve")));
        assert_eq!(
            required,
            [
                Json::Array(vec![text("text")]),
                Json::Array(vec![text("original"), text("edited")])
            ]
        );
    }

    #[test]
    fn lint_and_preserve_report_as_the_cli_does() {
        let (report, failed) =
            call(r#"{"name":"lint","arguments":{"text":"Clean line.\n\nIt worked — mostly.\n"}}"#);
        assert!(!failed && report.starts_with("line 3:"), "{report}");
        assert_eq!(
            call(r#"{"name":"lint","arguments":{"text":"It worked.\n"}}"#),
            ("No findings.".into(), false)
        );
        let (report, failed) = call(
            r#"{"name":"preserve","arguments":{"original":"Keep 42 items.\n","edited":"Keep items.\n"}}"#,
        );
        assert!(
            !failed && report.contains("dropped") && report.contains("42"),
            "{report}"
        );
        assert_eq!(
            report,
            preserve::report("Keep 42 items.\n", "Keep items.\n")
                .lines
                .join("\n")
        );
    }

    #[test]
    fn bad_tool_calls_are_errors_in_the_result() {
        let long = "a".repeat(MAX_INPUT + 1);
        for (arguments, message) in [
            (
                r#"{"name":"lint","arguments":{"text":1}}"#.to_string(),
                "every argument must be a string",
            ),
            (
                format!(r#"{{"name":"lint","arguments":{{"text":"{long}"}}}}"#),
                "each text must be at most 200000 characters",
            ),
            (
                r#"{"name":"lint","arguments":{}}"#.into(),
                "text is required",
            ),
            (
                r#"{"name":"preserve","arguments":{"original":"a"}}"#.into(),
                "original and edited are required",
            ),
            (
                r#"{"name":"lint","arguments":"text"}"#.into(),
                "arguments must be an object",
            ),
            (r#"{"name":"other"}"#.into(), "unknown tool: other"),
        ] {
            assert_eq!(call(&arguments), (message.to_string(), true));
        }
        let fits = "a".repeat(MAX_INPUT);
        assert!(
            !call(&format!(
                r#"{{"name":"lint","arguments":{{"text":"{fits}"}}}}"#
            ))
            .1
        );
    }
}
