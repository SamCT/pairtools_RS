use crate::cli::SelectArgs;
use rust_htslib::htslib;
use std::ffi::CString;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::os::raw::c_void;
use std::path::Path;

pub fn cmd_select(args: SelectArgs) -> Result<(), Box<dyn std::error::Error>> {
    reject_unsupported_select_options(&args)?;
    let mut reader = open_input(args.input.as_deref())?;
    let (headers, first_body_line) = read_header(reader.as_mut())?;
    let columns = Columns::from_header(&headers)?;
    let predicate = Expr::parse(&args.condition)?;
    predicate.validate_columns(&columns)?;
    let command_line = std::env::args().collect::<Vec<_>>().join(" ");
    let headers = append_select_pg(&headers, &command_line);
    let headers = canonical_select_headers(&headers);

    let mut out = open_output(args.output.as_deref())?;
    let mut rest = match args.output_rest.as_deref() {
        Some(path) => Some(open_output(Some(path))?),
        None => None,
    };
    for header in &headers {
        writeln!(out, "{header}")?;
        if let Some(rest) = rest.as_mut() {
            writeln!(rest, "{header}")?;
        }
    }
    if let Some(line) = first_body_line {
        write_selected_line(&mut out, rest.as_mut(), &line, &columns, &predicate)?;
    }
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let trimmed = trim_line_end(&line).to_string();
        write_selected_line(&mut out, rest.as_mut(), &trimmed, &columns, &predicate)?;
    }
    out.flush()?;
    if let Some(rest) = rest.as_mut() {
        rest.flush()?;
    }
    Ok(())
}

fn reject_unsupported_select_options(
    args: &SelectArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    if args.chrom_subset.is_some() {
        return Err("not implemented: pairtools select --chrom-subset".into());
    }
    if args.startup_code.is_some() {
        return Err("not implemented: pairtools select --startup-code".into());
    }
    if !args.type_cast.is_empty() {
        return Err("not implemented: pairtools select --type-cast".into());
    }
    if args.remove_columns.is_some() {
        return Err("not implemented: pairtools select --remove-columns".into());
    }
    if args.nproc_in.is_some() {
        return Err("not implemented: pairtools select --nproc-in".into());
    }
    if args.nproc_out.is_some() {
        return Err("not implemented: pairtools select --nproc-out".into());
    }
    if args.cmd_in.is_some() {
        return Err("not implemented: pairtools select --cmd-in".into());
    }
    if args.cmd_out.is_some() {
        return Err("not implemented: pairtools select --cmd-out".into());
    }
    Ok(())
}

fn open_input(path: Option<&Path>) -> Result<Box<dyn BufRead>, Box<dyn std::error::Error>> {
    match path {
        Some(path) if path == Path::new("-") => Ok(Box::new(BufReader::new(io::stdin()))),
        Some(path) if has_suffix(path, ".gz") => {
            Ok(Box::new(BufReader::new(BgzfReader::open(path)?)))
        }
        Some(path) if has_suffix(path, ".lz4") => {
            Err("not implemented: compressed select input .lz4".into())
        }
        Some(path) => Ok(Box::new(BufReader::new(File::open(path)?))),
        None => Ok(Box::new(BufReader::new(io::stdin()))),
    }
}

fn open_output(path: Option<&Path>) -> Result<Box<dyn Write>, Box<dyn std::error::Error>> {
    match path {
        Some(path) if path == Path::new("-") => Ok(Box::new(BufWriter::new(io::stdout()))),
        Some(path) if has_suffix(path, ".gz") => {
            Ok(Box::new(BufWriter::new(BgzfWriter::create(path)?)))
        }
        Some(path) if has_suffix(path, ".lz4") => {
            Err("not implemented: compressed select output .lz4".into())
        }
        Some(path) => Ok(Box::new(BufWriter::new(File::create(path)?))),
        None => Ok(Box::new(BufWriter::new(io::stdout()))),
    }
}

fn has_suffix(path: &Path, suffix: &str) -> bool {
    path.to_string_lossy().ends_with(suffix)
}

fn read_header(
    reader: &mut dyn BufRead,
) -> Result<(Vec<String>, Option<String>), Box<dyn std::error::Error>> {
    let mut headers = Vec::new();
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Ok((headers, None));
        }
        let trimmed = trim_line_end(&line).to_string();
        if trimmed.starts_with('#') {
            headers.push(trimmed);
        } else {
            return Ok((headers, Some(trimmed)));
        }
    }
}

struct Columns {
    names: Vec<String>,
}

impl Columns {
    fn from_header(headers: &[String]) -> Result<Self, Box<dyn std::error::Error>> {
        let columns_line = headers
            .iter()
            .find(|line| line.starts_with("#columns:"))
            .ok_or("Input .pairs/.pairsam header is missing #columns")?;
        let names: Vec<String> = columns_line
            .split_once(':')
            .map(|(_, rest)| rest)
            .unwrap_or("")
            .split_whitespace()
            .map(str::to_string)
            .collect();
        if names.is_empty() {
            return Err("Input .pairs/.pairsam header has empty #columns".into());
        }
        Ok(Self { names })
    }

    fn index(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|column| column == name)
    }

    fn is_numeric(&self, name: &str) -> bool {
        matches!(
            name,
            "pos1" | "pos2" | "mapq1" | "mapq2" | "read_len1" | "read_len2"
        ) || name.starts_with("pos5")
            || name.starts_with("pos3")
            || name.starts_with("frag")
    }
}

fn write_selected_line(
    out: &mut Box<dyn Write>,
    rest: Option<&mut Box<dyn Write>>,
    line: &str,
    columns: &Columns,
    predicate: &Expr,
) -> Result<(), Box<dyn std::error::Error>> {
    if line.is_empty() {
        return Ok(());
    }
    let fields: Vec<&str> = line.split('\t').collect();
    if predicate.eval(&fields, columns)? {
        writeln!(out, "{line}")?;
    } else if let Some(rest) = rest {
        writeln!(rest, "{line}")?;
    }
    Ok(())
}

#[derive(Clone, Debug)]
enum Expr {
    Compare(Operand, CmpOp, Operand),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
}

impl Expr {
    fn parse(condition: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let tokens = tokenize(condition)?;
        let mut parser = ParserState { tokens, pos: 0, condition };
        let expr = parser.parse_or()?;
        if !matches!(parser.peek(), Token::End) {
            return Err(parser.error("unexpected trailing tokens").into());
        }
        Ok(expr)
    }

    fn validate_columns(&self, columns: &Columns) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Expr::Compare(left, _, right) => {
                left.validate_columns(columns)?;
                right.validate_columns(columns)?;
            }
            Expr::And(left, right) | Expr::Or(left, right) => {
                left.validate_columns(columns)?;
                right.validate_columns(columns)?;
            }
            Expr::Not(inner) => inner.validate_columns(columns)?,
        }
        Ok(())
    }

    fn eval(
        &self,
        fields: &[&str],
        columns: &Columns,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        match self {
            Expr::Compare(left, op, right) => compare_values(
                left.eval(fields, columns)?,
                *op,
                right.eval(fields, columns)?,
            ),
            Expr::And(left, right) => Ok(left.eval(fields, columns)? && right.eval(fields, columns)?),
            Expr::Or(left, right) => Ok(left.eval(fields, columns)? || right.eval(fields, columns)?),
            Expr::Not(inner) => Ok(!inner.eval(fields, columns)?),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Clone, Debug)]
enum Operand {
    Column(String),
    String(String),
    Number(f64),
}

impl Operand {
    fn validate_columns(&self, columns: &Columns) -> Result<(), Box<dyn std::error::Error>> {
        if let Operand::Column(name) = self {
            if columns.index(name).is_none() {
                return Err(format!("not implemented: pairtools select unknown column {name}").into());
            }
        }
        Ok(())
    }

    fn eval(
        &self,
        fields: &[&str],
        columns: &Columns,
    ) -> Result<EvalValue, Box<dyn std::error::Error>> {
        match self {
            Operand::Column(name) => {
                let Some(index) = columns.index(name) else {
                    return Err(format!("not implemented: pairtools select unknown column {name}").into());
                };
                let value = fields.get(index).copied().unwrap_or("");
                Ok(EvalValue {
                    text: value.to_string(),
                    number: value.parse::<f64>().ok(),
                    prefer_numeric: columns.is_numeric(name),
                })
            }
            Operand::String(value) => Ok(EvalValue {
                text: value.clone(),
                number: None,
                prefer_numeric: false,
            }),
            Operand::Number(value) => Ok(EvalValue {
                text: format_numeric(*value),
                number: Some(*value),
                prefer_numeric: true,
            }),
        }
    }
}

struct EvalValue {
    text: String,
    number: Option<f64>,
    prefer_numeric: bool,
}

fn compare_values(
    left: EvalValue,
    op: CmpOp,
    right: EvalValue,
) -> Result<bool, Box<dyn std::error::Error>> {
    let numeric = left.prefer_numeric || right.prefer_numeric;
    if numeric {
        let Some(left) = left.number else {
            return Ok(matches!(op, CmpOp::Ne));
        };
        let Some(right) = right.number else {
            return Ok(matches!(op, CmpOp::Ne));
        };
        return Ok(match op {
            CmpOp::Eq => left == right,
            CmpOp::Ne => left != right,
            CmpOp::Lt => left < right,
            CmpOp::Le => left <= right,
            CmpOp::Gt => left > right,
            CmpOp::Ge => left >= right,
        });
    }

    Ok(match op {
        CmpOp::Eq => left.text == right.text,
        CmpOp::Ne => left.text != right.text,
        CmpOp::Lt => left.text < right.text,
        CmpOp::Le => left.text <= right.text,
        CmpOp::Gt => left.text > right.text,
        CmpOp::Ge => left.text >= right.text,
    })
}

fn format_numeric(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}

#[derive(Clone, Debug)]
enum Token {
    Ident(String),
    String(String),
    Number(f64),
    And,
    Or,
    Not,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    LParen,
    RParen,
    End,
}

fn tokenize(condition: &str) -> Result<Vec<Token>, Box<dyn std::error::Error>> {
    let bytes = condition.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() {
            i += 1;
        } else if b == b'(' {
            tokens.push(Token::LParen);
            i += 1;
        } else if b == b')' {
            tokens.push(Token::RParen);
            i += 1;
        } else if b == b'=' && bytes.get(i + 1) == Some(&b'=') {
            tokens.push(Token::Eq);
            i += 2;
        } else if b == b'!' && bytes.get(i + 1) == Some(&b'=') {
            tokens.push(Token::Ne);
            i += 2;
        } else if b == b'<' && bytes.get(i + 1) == Some(&b'=') {
            tokens.push(Token::Le);
            i += 2;
        } else if b == b'>' && bytes.get(i + 1) == Some(&b'=') {
            tokens.push(Token::Ge);
            i += 2;
        } else if b == b'<' {
            tokens.push(Token::Lt);
            i += 1;
        } else if b == b'>' {
            tokens.push(Token::Gt);
            i += 1;
        } else if b == b'\'' || b == b'"' {
            let quote = b;
            i += 1;
            let start = i;
            while i < bytes.len() && bytes[i] != quote {
                if bytes[i] == b'\\' {
                    return Err(condition_error(condition, "escaped string literals are not supported").into());
                }
                i += 1;
            }
            if i == bytes.len() {
                return Err(condition_error(condition, "unterminated string literal").into());
            }
            let value = std::str::from_utf8(&bytes[start..i])?.to_string();
            tokens.push(Token::String(value));
            i += 1;
        } else if b.is_ascii_digit()
            || (b == b'-' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit))
        {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            let raw = std::str::from_utf8(&bytes[start..i])?;
            let value = raw
                .parse::<f64>()
                .map_err(|_| condition_error(condition, "invalid numeric literal"))?;
            tokens.push(Token::Number(value));
        } else if b.is_ascii_alphabetic() || b == b'_' {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let ident = std::str::from_utf8(&bytes[start..i])?;
            match ident {
                "and" => tokens.push(Token::And),
                "or" => tokens.push(Token::Or),
                "not" => tokens.push(Token::Not),
                _ => tokens.push(Token::Ident(ident.to_string())),
            }
        } else {
            return Err(condition_error(condition, "unsupported expression syntax").into());
        }
    }
    tokens.push(Token::End);
    Ok(tokens)
}

struct ParserState<'a> {
    tokens: Vec<Token>,
    pos: usize,
    condition: &'a str,
}

impl ParserState<'_> {
    fn parse_or(&mut self) -> Result<Expr, Box<dyn std::error::Error>> {
        let mut expr = self.parse_and()?;
        while matches!(self.peek(), Token::Or) {
            self.bump();
            let rhs = self.parse_and()?;
            expr = Expr::Or(Box::new(expr), Box::new(rhs));
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<Expr, Box<dyn std::error::Error>> {
        let mut expr = self.parse_not()?;
        while matches!(self.peek(), Token::And) {
            self.bump();
            let rhs = self.parse_not()?;
            expr = Expr::And(Box::new(expr), Box::new(rhs));
        }
        Ok(expr)
    }

    fn parse_not(&mut self) -> Result<Expr, Box<dyn std::error::Error>> {
        if matches!(self.peek(), Token::Not) {
            self.bump();
            return Ok(Expr::Not(Box::new(self.parse_not()?)));
        }
        self.parse_atom_expr()
    }

    fn parse_atom_expr(&mut self) -> Result<Expr, Box<dyn std::error::Error>> {
        if matches!(self.peek(), Token::LParen) {
            self.bump();
            let expr = self.parse_or()?;
            if !matches!(self.peek(), Token::RParen) {
                return Err(self.error("missing closing parenthesis").into());
            }
            self.bump();
            return Ok(expr);
        }

        let left = self.parse_operand()?;
        let op = self.parse_cmp_op()?;
        let right = self.parse_operand()?;
        Ok(Expr::Compare(left, op, right))
    }

    fn parse_operand(&mut self) -> Result<Operand, Box<dyn std::error::Error>> {
        match self.bump() {
            Token::Ident(name) => Ok(Operand::Column(name)),
            Token::String(value) => Ok(Operand::String(value)),
            Token::Number(value) => Ok(Operand::Number(value)),
            _ => Err(self.error("expected column or literal").into()),
        }
    }

    fn parse_cmp_op(&mut self) -> Result<CmpOp, Box<dyn std::error::Error>> {
        match self.bump() {
            Token::Eq => Ok(CmpOp::Eq),
            Token::Ne => Ok(CmpOp::Ne),
            Token::Lt => Ok(CmpOp::Lt),
            Token::Le => Ok(CmpOp::Le),
            Token::Gt => Ok(CmpOp::Gt),
            Token::Ge => Ok(CmpOp::Ge),
            _ => Err(self.error("expected comparison operator").into()),
        }
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::End)
    }

    fn bump(&mut self) -> Token {
        let token = self.tokens.get(self.pos).cloned().unwrap_or(Token::End);
        self.pos += 1;
        token
    }

    fn error(&self, detail: &str) -> String {
        condition_error(self.condition, detail)
    }
}

fn condition_error(condition: &str, detail: &str) -> String {
    format!("not implemented: pairtools select condition {condition}: {detail}")
}

fn append_select_pg(headers: &[String], command_line: &str) -> Vec<String> {
    let pg_records = samheader_pg_records(headers);
    if pg_records.is_empty() {
        return headers.to_vec();
    }

    let pp_ids: Vec<&str> = pg_records
        .iter()
        .filter_map(|record| record.pp.as_deref())
        .collect();
    let mut terminals: Vec<&PgRecord> = pg_records
        .iter()
        .filter(|record| !pp_ids.contains(&record.id.as_str()))
        .collect();
    if terminals.is_empty() {
        terminals = pg_records.iter().collect();
    }

    let branch_count = terminals.len();
    let new_records: Vec<String> = terminals
        .iter()
        .enumerate()
        .map(|(idx, terminal)| {
            let id = if branch_count == 1 {
                "pairtools_select".to_string()
            } else {
                format!(
                    "pairtools_select-{}.{}",
                    idx + 1,
                    pg_chain_len(terminal, &pg_records) + 1
                )
            };
            format!(
                "#samheader: @PG\tID:{id}\tPN:pairtools_select\tCL:{command_line}\tPP:{}\tVN:1.1.3",
                terminal.id
            )
        })
        .collect();

    let insert_at = headers
        .iter()
        .rposition(|line| line.starts_with("#samheader:"))
        .map(|idx| idx + 1)
        .unwrap_or(headers.len());
    let mut out = Vec::with_capacity(headers.len() + new_records.len());
    out.extend_from_slice(&headers[..insert_at]);
    out.extend(new_records);
    out.extend_from_slice(&headers[insert_at..]);
    out
}

fn canonical_select_headers(headers: &[String]) -> Vec<String> {
    let mut primary = Vec::new();
    let mut chroms = Vec::new();
    let mut samheaders = Vec::new();
    let mut columns = Vec::new();
    for header in headers {
        if header.starts_with("#samheader:") {
            samheaders.push(header.clone());
        } else if header.starts_with("#chromosomes:") || header.starts_with("#chromsize:") {
            chroms.push(header.clone());
        } else if header.starts_with("#columns:") {
            columns.push(header.clone());
        } else {
            primary.push(header.clone());
        }
    }

    let mut out = Vec::with_capacity(headers.len());
    out.extend(primary);
    out.extend(chroms);
    out.extend(samheaders);
    out.extend(columns);
    out
}

struct PgRecord {
    id: String,
    pp: Option<String>,
}

fn samheader_pg_records(headers: &[String]) -> Vec<PgRecord> {
    let mut records = Vec::new();
    for line in headers {
        let Some(sam) = line.strip_prefix("#samheader: ") else {
            continue;
        };
        if !sam.starts_with("@PG\t") {
            continue;
        }
        let mut id = None;
        let mut pp = None;
        for field in sam.split('\t').skip(1) {
            if let Some(value) = field.strip_prefix("ID:") {
                id = Some(value.to_string());
            } else if let Some(value) = field.strip_prefix("PP:") {
                pp = Some(value.to_string());
            }
        }
        if let Some(id) = id {
            records.push(PgRecord { id, pp });
        }
    }
    records
}

fn pg_chain_len(terminal: &PgRecord, records: &[PgRecord]) -> usize {
    let mut len = 1;
    let mut parent = terminal.pp.as_deref();
    while let Some(parent_id) = parent {
        let Some(record) = records.iter().find(|record| record.id == parent_id) else {
            break;
        };
        len += 1;
        parent = record.pp.as_deref();
    }
    len
}

fn trim_line_end(line: &str) -> &str {
    let line = line.strip_suffix('\n').unwrap_or(line);
    line.strip_suffix('\r').unwrap_or(line)
}

struct BgzfReader {
    handle: *mut htslib::BGZF,
}

impl BgzfReader {
    fn open(path: &Path) -> io::Result<Self> {
        let path = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "input path contains NUL byte")
        })?;
        let mode = CString::new("r").expect("static BGZF mode has no NUL bytes");
        let handle = unsafe { htslib::bgzf_open(path.as_ptr(), mode.as_ptr()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { handle })
    }

    fn close(&mut self) -> io::Result<()> {
        if self.handle.is_null() {
            return Ok(());
        }
        let status = unsafe { htslib::bgzf_close(self.handle) };
        self.handle = std::ptr::null_mut();
        if status == 0 {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                format!("failed to close BGZF stream, HTSlib status {status}"),
            ))
        }
    }
}

impl Read for BgzfReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let read = unsafe {
            htslib::bgzf_read(
                self.handle,
                buf.as_mut_ptr() as *mut c_void,
                buf.len(),
            )
        };
        if read < 0 {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "failed to read BGZF stream",
            ))
        } else {
            Ok(read as usize)
        }
    }
}

impl Drop for BgzfReader {
    fn drop(&mut self) {
        drop(self.close());
    }
}

struct BgzfWriter {
    handle: *mut htslib::BGZF,
}

impl BgzfWriter {
    fn create(path: &Path) -> io::Result<Self> {
        let path = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "output path contains NUL byte")
        })?;
        let mode = CString::new("w").expect("static BGZF mode has no NUL bytes");
        let handle = unsafe { htslib::bgzf_open(path.as_ptr(), mode.as_ptr()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { handle })
    }

    fn close(&mut self) -> io::Result<()> {
        if self.handle.is_null() {
            return Ok(());
        }
        let status = unsafe { htslib::bgzf_close(self.handle) };
        self.handle = std::ptr::null_mut();
        if status == 0 {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                format!("failed to close BGZF stream, HTSlib status {status}"),
            ))
        }
    }
}

impl Write for BgzfWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let written =
            unsafe { htslib::bgzf_write(self.handle, buf.as_ptr() as *const c_void, buf.len()) };
        if written < 0 {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "failed to write BGZF stream",
            ))
        } else {
            Ok(written as usize)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        let status = unsafe { htslib::bgzf_flush(self.handle) };
        if status == 0 {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                format!("failed to flush BGZF stream, HTSlib status {status}"),
            ))
        }
    }
}

impl Drop for BgzfWriter {
    fn drop(&mut self) {
        drop(self.close());
    }
}
