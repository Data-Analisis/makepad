use makepad_widget::*;

pub struct TokenParserItem {
    pub chunk: Vec<char>,
    pub token_type: TokenType,
}

pub struct TokenParser<'a> {
    pub tokens: &'a Vec<TokenChunk>,
    pub flat_text: &'a Vec<char>,
    pub index: usize,
    pub next_index: usize
}

impl <'a>TokenParser<'a> {
    pub fn new(flat_text: &'a Vec<char>, token_chunks: &'a Vec<TokenChunk>) -> TokenParser<'a> {
        TokenParser {
            tokens: token_chunks,
            flat_text: flat_text,
            index: 0,
            next_index: 0
        }
    }
    
    pub fn advance(&mut self) -> bool {
        if self.next_index >= self.tokens.len() {
            return false
        }
        self.index = self.next_index;
        self.next_index += 1;
        return true;
    }
    
    pub fn eat_should_ignore(&mut self) -> bool {
        while self.cur_type().should_ignore() {
            if !self.advance() {
                return false
            }
        }
        return true
    }
    
    pub fn eat(&mut self, what: &str) -> bool {
        // eat as many ignorable tokens
        if !self.eat_should_ignore() {
            return false
        }
        // then match what in our token
        let chunk = &self.tokens[self.index];
        let mut off = chunk.offset;
        for c in what.chars() {
            if off - chunk.offset > chunk.len {
                return false
            }
            if self.flat_text[off] != c {
                return false;
            }
            off += 1;
        }
        if off - chunk.offset != chunk.len {
            return false
        }
        if !self.eat_should_ignore() {
            return false
        }
        self.advance();
        true
    }
    
    pub fn prev_type(&self) -> TokenType {
        if self.index > 0 {
            self.tokens[self.index - 1].token_type
        }
        else {
            TokenType::Unexpected
        }
    }
    
    pub fn cur_pair_as_string(&self) -> Option<String> {
        let pair_token = self.tokens[self.index].pair_token;
        if pair_token < self.index || pair_token >= self.tokens.len() {
            return None
        }
        let mut out_str = String::new();
        for i in self.cur_offset() + 1..self.cur_pair_offset() {
            out_str.push(self.flat_text[i]);
        }
        Some(out_str)
    }
    
    pub fn cur_as_string(&self) -> String {
        let mut out_str = String::new();
        let tok = &self.tokens[self.index];
        for i in tok.offset..tok.offset + tok.len {
            out_str.push(self.flat_text[i]);
        }
        return out_str
    }
    
    pub fn cur_type(&self) -> TokenType {
        self.tokens[self.index].token_type
    }
    
    pub fn cur_line_col(&self) -> (usize, usize) {
        let off = self.cur_offset();
        let mut line = 0;
        let mut lc = 0;
        for i in 0..off {
            if self.flat_text[i] == '\n' {
                line = line + 1;
                lc = i;
            }
        }
        return (line, off - lc);
    }
    
    pub fn cur_offset(&self) -> usize {
        self.tokens[self.index].offset
    }
    
    pub fn jump_to_pair(&mut self) {
        let pair_token = self.tokens[self.index].pair_token;
        if pair_token > self.index && pair_token < self.tokens.len() {
            self.index = pair_token;
        }
    }
    
    pub fn cur_pair_offset(&self) -> usize {
        self.tokens[self.tokens[self.index].pair_token].offset
    }
    
    pub fn cur_pair_range(&self) -> (usize, usize) {
        (
            self.tokens[self.index].offset,
            self.tokens[self.tokens[self.index].pair_token].offset
        )
    }
    
    pub fn cur_range(&self) -> (usize, usize) {
        (
            self.tokens[self.index].offset,
            self.tokens[self.index].offset + self.tokens[self.index].len
        )
    }
    
    pub fn next_type(&self) -> TokenType {
        if self.index < self.tokens.len() - 1 {
            self.tokens[self.index + 1].token_type
        }
        else {
            TokenType::Unexpected
        }
    }
    
    pub fn prev_char(&self) -> char {
        if self.index > 0 {
            let len = self.tokens[self.index - 1].len;
            let ch = self.flat_text[self.tokens[self.index - 1].offset];
            if len == 1 || ch == ' ' {
                return ch
            }
        }
        '\0'
    }
    
    pub fn cur_char(&self) -> char {
        let len = self.tokens[self.index].len;
        let ch = self.flat_text[self.tokens[self.index].offset];
        if len == 1 || ch == ' ' {
            return ch
        }
        '\0'
    }
    
    pub fn cur_chunk(&self) -> &[char] {
        let offset = self.tokens[self.index].offset;
        let len = self.tokens[self.index].len;
        &self.flat_text[offset..(offset + len)]
    }
    
    pub fn next_char(&self) -> char {
        if self.index < self.tokens.len() - 1 {
            let len = self.tokens[self.index + 1].len;
            let ch = self.flat_text[self.tokens[self.index + 1].offset];
            if len == 1 || ch == ' ' {
                return ch
            }
        }
        '\0'
    }
}

pub struct FormatOutput {
    pub out_lines: Vec<Vec<char >>
}

impl FormatOutput {
    pub fn new() -> FormatOutput {
        FormatOutput {
            out_lines: Vec::new()
        }
    }
    
    pub fn indent(&mut self, indent_depth: usize) {
        let last_line = self.out_lines.last_mut().unwrap();
        for _ in 0..indent_depth {
            last_line.push(' ');
        }
    }
    
    pub fn strip_space(&mut self) {
        let last_line = self.out_lines.last_mut().unwrap();
        if last_line.len()>0 && *last_line.last().unwrap() == ' ' {
            last_line.pop();
        }
    }
    
    pub fn new_line(&mut self) {
        self.out_lines.push(Vec::new());
    }
    
    pub fn extend(&mut self, chunk: &[char]) {
        let last_line = self.out_lines.last_mut().unwrap();
        last_line.extend_from_slice(chunk);
    }
    
    pub fn add_space(&mut self) {
        let last_line = self.out_lines.last_mut().unwrap();
        if last_line.len()>0 {
            if *last_line.last().unwrap() != ' ' {
                last_line.push(' ');
            }
        }
        else {
            last_line.push(' ');
        }
    }
    
}


pub struct MprsTokenizer {
    pub comment_single: bool,
    pub comment_depth: usize,
    pub in_string_code: bool,
    pub in_string: bool
}

impl MprsTokenizer {
    
    pub fn new() -> MprsTokenizer {
        MprsTokenizer {
            comment_single: false,
            comment_depth: 0,
            in_string: false,
            in_string_code: false
        }
    }
    
    pub fn next_token<'a>(&mut self, state: &mut TokenizerState<'a>, chunk: &mut Vec<char>, token_chunks: &Vec<TokenChunk>) -> TokenType {
        let start = chunk.len();
        //chunk.truncate(0);
        if self.in_string {
            if state.next == ' ' || state.next == '\t' {
                while state.next == ' ' || state.next == '\t' {
                    chunk.push(state.next);
                    state.advance_with_cur();
                }
                return TokenType::Whitespace;
            }
            loop {
                if state.eof {
                    self.in_string = false;
                    return TokenType::StringChunk
                }
                else if state.next == '\n' {
                    if (chunk.len() - start)>0 {
                        return TokenType::StringChunk
                    }
                    chunk.push(state.next);
                    state.advance_with_cur();
                    return TokenType::Newline
                }
                else if state.next == '"' && state.cur != '\\' {
                    if (chunk.len() - start)>0 {
                        return TokenType::StringChunk
                    }
                    chunk.push(state.next);
                    state.advance_with_cur();
                    self.in_string = false;
                    return TokenType::StringMultiEnd
                }
                else {
                    chunk.push(state.next);
                    state.advance_with_cur();
                }
            }
            
        }
        else if self.comment_depth >0 { // parse comments
            loop {
                if state.eof {
                    self.comment_depth = 0;
                    return TokenType::CommentChunk
                }
                if state.next == '/' {
                    chunk.push(state.next);
                    state.advance();
                    if state.next == '*' {
                        chunk.push(state.next);
                        state.advance();
                        self.comment_depth += 1;
                    }
                }
                else if state.next == '*' {
                    chunk.push(state.next);
                    state.advance();
                    if state.next == '/' {
                        self.comment_depth -= 1;
                        chunk.push(state.next);
                        state.advance();
                        if self.comment_depth == 0 {
                            return TokenType::CommentMultiEnd
                        }
                    }
                }
                else if state.next == '\n' {
                    if self.comment_single {
                        self.comment_depth = 0;
                    }
                    // output current line
                    if (chunk.len() - start)>0 {
                        return TokenType::CommentChunk
                    }
                    
                    chunk.push(state.next);
                    state.advance();
                    return TokenType::Newline
                }
                else if state.next == ' ' {
                    if (chunk.len() - start)>0 {
                        return TokenType::CommentChunk
                    }
                    while state.next == ' ' {
                        chunk.push(state.next);
                        state.advance();
                    }
                    return TokenType::Whitespace
                }
                else {
                    chunk.push(state.next);
                    state.advance();
                }
            }
        }
        else {
            if state.eof{
                return TokenType::Eof
            }
            state.advance_with_cur();
            match state.cur {
                '\0' => { // eof insert a terminating space and end
                    chunk.push('\0');
                    return TokenType::Whitespace
                },
                '\n' => {
                    chunk.push('\n');
                    return TokenType::Newline
                },
                ' ' | '\t' => { // eat as many spaces as possible
                    chunk.push(state.cur);
                    while state.next == ' ' || state.next == '\t' {
                        chunk.push(state.next);
                        state.advance();
                    }
                    return TokenType::Whitespace;
                },
                '/' => { // parse comment
                    chunk.push(state.cur);
                    if state.next == '/' {
                        chunk.push(state.next);
                        state.advance();
                        self.comment_depth = 1;
                        self.comment_single = true;
                        return TokenType::CommentLine;
                    }
                    if state.next == '*' { // start parsing a multiline comment
                        //let mut comment_depth = 1;
                        chunk.push(state.next);
                        state.advance();
                        self.comment_single = false;
                        self.comment_depth = 1;
                        return TokenType::CommentMultiBegin;
                    }
                    if state.next == '=' {
                        chunk.push(state.next);
                        state.advance();
                    }
                    return TokenType::Operator;
                },
                '\'' => { // parse char literal or lifetime annotation
                    chunk.push(state.cur);
                    
                    if Self::parse_rust_escape_char(state, chunk) { // escape char or unicode
                        if state.next == '\'' { // parsed to closing '
                            chunk.push(state.next);
                            state.advance();
                            return TokenType::String;
                        }
                        return TokenType::TypeName;
                    }
                    else { // parse a single char or lifetime
                        let offset = state.offset;
                        let (is_ident, _) = Self::parse_rust_ident_tail(state, chunk);
                        if is_ident && ((state.offset - offset) >1 || state.next != '\'') {
                            return TokenType::TypeName;
                        }
                        if state.next != '\n' {
                            if (state.offset - offset) == 0 { // not an identifier char
                                chunk.push(state.next);
                                state.advance();
                            }
                            if state.next == '\'' { // lifetime identifier
                                chunk.push(state.next);
                                state.advance();
                            }
                            return TokenType::String;
                        }
                        return TokenType::String;
                    }
                },
                '"' => { // parse string
                    // we have to scan back, skip all whitespacey things
                    // see if we find a shader!(
                    // we have to backparse.
                    
                    
                    chunk.push(state.cur);
                    
                    if chunk.len()>=2 && chunk[chunk.len() - 2] == '{' {
                        self.in_string_code = true;
                        return TokenType::ParenOpen;
                    }
                    if state.next == '}' && self.in_string_code {
                        self.in_string_code = false;
                        return TokenType::ParenClose;
                    }
                    
                    state.prev = '\0';
                    while !state.eof && state.next != '\n' {
                        if state.next != '"' || state.prev != '\\' && state.cur == '\\' && state.next == '"' {
                            chunk.push(state.next);
                            state.advance_with_prev();
                        }
                        else {
                            chunk.push(state.next);
                            state.advance();
                            return TokenType::String;
                        }
                    };
                    if state.next == '\n' {
                        self.in_string = true;
                        return TokenType::StringMultiBegin;
                    }
                    return TokenType::String;
                },
                '0'..='9' => { // try to parse numbers
                    chunk.push(state.cur);
                    Self::parse_rust_number_tail(state, chunk);
                    return TokenType::Number;
                },
                ':' => {
                    chunk.push(state.cur);
                    if state.next == ':' {
                        chunk.push(state.next);
                        state.advance();
                        return TokenType::Namespace;
                    }
                    return TokenType::Colon;
                },
                '*' => {
                    chunk.push(state.cur);
                    if state.next == '=' {
                        chunk.push(state.next);
                        state.advance();
                        return TokenType::Operator;
                    }
                    if state.next == '/' {
                        chunk.push(state.next);
                        state.advance();
                        return TokenType::Unexpected;
                    }
                    return TokenType::Operator;
                },
                '^' => {
                    chunk.push(state.cur);
                    if state.next == '=' {
                        chunk.push(state.next);
                        state.advance();
                    }
                    return TokenType::Operator;
                },
                '+' => {
                    chunk.push(state.cur);
                    if state.next == '=' {
                        chunk.push(state.next);
                        state.advance();
                    }
                    return TokenType::Operator;
                },
                '-' => {
                    chunk.push(state.cur);
                    if state.next == '>' || state.next == '=' {
                        chunk.push(state.next);
                        state.advance();
                    }
                    return TokenType::Operator;
                },
                '=' => {
                    chunk.push(state.cur);
                    if state.next == '>' || state.next == '=' {
                        chunk.push(state.next);
                        state.advance();
                    }
                    
                    return TokenType::Operator;
                },
                '.' => {
                    chunk.push(state.cur);
                    if state.next == '.' {
                        chunk.push(state.next);
                        state.advance();
                        if state.next == '=' {
                            chunk.push(state.next);
                            state.advance();
                            return TokenType::Splat;
                        }
                        return TokenType::Splat;
                    }
                    return TokenType::Operator;
                },
                ';' => {
                    chunk.push(state.cur);
                    if state.next == '.' {
                        chunk.push(state.next);
                        state.advance();
                    }
                    return TokenType::Delimiter;
                },
                '&' => {
                    chunk.push(state.cur);
                    if state.next == '&' || state.next == '=' {
                        chunk.push(state.next);
                        state.advance();
                    }
                    return TokenType::Operator;
                },
                '|' => {
                    chunk.push(state.cur);
                    if state.next == '|' || state.next == '=' {
                        chunk.push(state.next);
                        state.advance();
                    }
                    return TokenType::Operator;
                },
                '!' => {
                    chunk.push(state.cur);
                    if state.next == '=' {
                        chunk.push(state.next);
                        state.advance();
                    }
                    return TokenType::Operator;
                },
                '<' => {
                    chunk.push(state.cur);
                    if state.next == '=' {
                        chunk.push(state.next);
                        state.advance();
                    }
                    if state.next == '<' {
                        chunk.push(state.next);
                        state.advance();
                        if state.next == '=' {
                            chunk.push(state.next);
                            state.advance();
                        }
                    }
                    return TokenType::Operator;
                },
                '>' => {
                    chunk.push(state.cur);
                    if state.next == '=' {
                        chunk.push(state.next);
                        state.advance();
                    }
                    if state.next == '>' {
                        chunk.push(state.next);
                        state.advance();
                        if state.next == '=' {
                            chunk.push(state.next);
                            state.advance();
                        }
                    }
                    return TokenType::Operator;
                },
                ',' => {
                    chunk.push(state.cur);
                    return TokenType::Delimiter;
                },
                '(' | '{' | '[' => {
                    chunk.push(state.cur);
                    return TokenType::ParenOpen;
                },
                ')' | '}' | ']' => {
                    chunk.push(state.cur);
                    return TokenType::ParenClose;
                },
                '#' => {
                    chunk.push(state.cur);
                    // if followed by 0-9A-Fa-f parse untill not one of those
                    if state.next >= '0' && state.next <= '9' 
                    || state.next >= 'a' && state.next <= 'f'
                    || state.next >= 'A' && state.next <= 'F' { // parse a hex number
                        chunk.push(state.next);
                        state.advance();
                        while state.next_is_hex() {
                            chunk.push(state.next);
                            state.advance();
                        }
                        return TokenType::Color;
                    }
                    else{
                        return TokenType::Hash;
                    }
                },
                '_' => {
                    chunk.push(state.cur);
                    Self::parse_rust_ident_tail(state, chunk);
                    if state.next == '(' {
                        return TokenType::Call;
                    }
                    if state.next == '!' {
                        return TokenType::Macro;
                    }
                    return TokenType::Identifier;
                },
                'a'..='z' => { // try to parse keywords or identifiers
                    chunk.push(state.cur);
                    
                    let keyword_type = Self::parse_rust_lc_keyword(state, chunk, token_chunks);
                    let (is_ident, _) = Self::parse_rust_ident_tail(state, chunk);
                    if is_ident {
                        if state.next == '(' {
                            return TokenType::Call;
                        }
                        if state.next == '!' {
                            return TokenType::Macro;
                        }
                        return TokenType::Identifier;
                    }
                    else {
                        return keyword_type
                    }
                },
                'A'..='Z' => {
                    chunk.push(state.cur);
                    let mut is_keyword = false;
                    if state.cur == 'S' {
                        if state.keyword(chunk, "elf") {
                            is_keyword = true;
                        }
                    }
                    let (is_ident, has_underscores) = Self::parse_rust_ident_tail(state, chunk);
                    if is_ident {
                        is_keyword = false;
                    }
                    if has_underscores {
                        return TokenType::ThemeName;
                    }
                    if is_keyword {
                        return TokenType::Keyword;
                    }
                    return TokenType::TypeName;
                },
                _ => {
                    chunk.push(state.cur);
                    return TokenType::Operator;
                }
            }
        }
    }
    
    fn parse_rust_ident_tail<'a>(state: &mut TokenizerState<'a>, chunk: &mut Vec<char>) -> (bool, bool) {
        let mut ret = false;
        let mut has_underscores = false;
        while state.next_is_digit() || state.next_is_letter() || state.next == '_' || state.next == '$' {
            if state.next == '_' {
                has_underscores = true;
            }
            ret = true;
            chunk.push(state.next);
            state.advance();
        }
        (ret, has_underscores)
    }
    
    
    fn parse_rust_escape_char<'a>(state: &mut TokenizerState<'a>, chunk: &mut Vec<char>) -> bool {
        if state.next == '\\' {
            chunk.push(state.next);
            state.advance();
            if state.next == 'u' {
                chunk.push(state.next);
                state.advance();
                if state.next == '{' {
                    chunk.push(state.next);
                    state.advance();
                    while state.next_is_hex() {
                        chunk.push(state.next);
                        state.advance();
                    }
                    if state.next == '}' {
                        chunk.push(state.next);
                        state.advance();
                    }
                }
            }
            else if state.next != '\n' && state.next != '\0' {
                // its a single char escape TODO limit this to valid escape chars
                chunk.push(state.next);
                state.advance();
            }
            return true
        }
        return false
    }
    fn parse_rust_number_tail<'a>(state: &mut TokenizerState<'a>, chunk: &mut Vec<char>) {
        if state.next == 'x' { // parse a hex number
            chunk.push(state.next);
            state.advance();
            while state.next_is_hex() || state.next == '_' {
                chunk.push(state.next);
                state.advance();
            }
        }
        else if state.next == 'b' { // parse a binary
            chunk.push(state.next);
            state.advance();
            while state.next == '0' || state.next == '1' || state.next == '_' {
                chunk.push(state.next);
                state.advance();
            }
        }
        else if state.next == 'o' { // parse a octal
            chunk.push(state.next);
            state.advance();
            while state.next == '0' || state.next == '1' || state.next == '2'
                || state.next == '3' || state.next == '4' || state.next == '5'
                || state.next == '6' || state.next == '7' || state.next == '_' {
                chunk.push(state.next);
                state.advance();
            }
        }
        else {
            while state.next_is_digit() || state.next == '_' {
                chunk.push(state.next);
                state.advance();
            }
            if state.next == 'u' || state.next == 'i' {
                chunk.push(state.next);
                state.advance();
                if state.keyword(chunk, "8") {
                }
                else if state.keyword(chunk, "16") {
                }
                else if state.keyword(chunk, "32") {
                }
                else if state.keyword(chunk, "64") {
                }
            }
            else if state.next == '.' || state.next == 'f' || state.next == 'e' || state.next == 'E' {
                if state.next == '.' || state.next == 'f' {
                    chunk.push(state.next);
                    state.advance();
                    while state.next_is_digit() || state.next == '_' {
                        chunk.push(state.next);
                        state.advance();
                    }
                }
                if state.next == 'E' || state.next == 'e' {
                    chunk.push(state.next);
                    state.advance();
                    if state.next == '+' || state.next == '-' {
                        chunk.push(state.next);
                        state.advance();
                        while state.next_is_digit() || state.next == '_' {
                            chunk.push(state.next);
                            state.advance();
                        }
                    }
                    else {
                        return
                    }
                }
                if state.next == 'f' { // the f32, f64 postfix
                    chunk.push(state.next);
                    state.advance();
                    if state.keyword(chunk, "32") {
                    }
                    else if state.keyword(chunk, "64") {
                    }
                }
            }
        }
    }
    
    fn parse_rust_lc_keyword<'a>(state: &mut TokenizerState<'a>, chunk: &mut Vec<char>, token_chunks: &Vec<TokenChunk>) -> TokenType {
        match state.cur {
            'a' => {
                if state.keyword(chunk, "s") {
                    return TokenType::Keyword
                }
            },
            'b' => {
                if state.keyword(chunk, "reak") {
                    return TokenType::Flow
                }
                if state.keyword(chunk, "ool") {
                    return TokenType::BuiltinType
                }
            },
            'c' => {
                if state.keyword(chunk, "on") {
                    if state.keyword(chunk, "st") {
                        return TokenType::Keyword
                    }
                    if state.keyword(chunk, "tinue") {
                        return TokenType::Flow
                    }
                }
                if state.keyword(chunk, "rate") {
                    return TokenType::Keyword
                }
                if state.keyword(chunk, "har") {
                    return TokenType::BuiltinType
                }
            },
            'd' => {
                if state.keyword(chunk, "yn") {
                    return TokenType::Keyword
                }
            },
            'e' => {
                if state.keyword(chunk, "lse") {
                    return TokenType::Flow
                }
                if state.keyword(chunk, "num") {
                    return TokenType::TypeDef
                }
                if state.keyword(chunk, "xtern") {
                    return TokenType::Keyword
                }
            },
            'f' => {
                if state.keyword(chunk, "alse") {
                    return TokenType::Bool
                }
                if state.keyword(chunk, "n") {
                    return TokenType::Fn
                }
                if state.keyword(chunk, "or") {
                    // check if we are first on a line
                    if token_chunks.len() <2
                        || token_chunks[token_chunks.len() - 1].token_type == TokenType::Newline
                        || token_chunks[token_chunks.len() - 2].token_type == TokenType::Newline
                        && token_chunks[token_chunks.len() - 1].token_type == TokenType::Whitespace {
                        return TokenType::Looping;
                        //self.code_editor.set_indent_color(self.code_editor.colors.indent_line_looping);
                    }
                    
                    return TokenType::Keyword;
                    // self.code_editor.set_indent_color(self.code_editor.colors.indent_line_def);
                }
                
                if state.keyword(chunk, "32") {
                    return TokenType::BuiltinType
                }
                if state.keyword(chunk, "64") {
                    return TokenType::BuiltinType
                }
            },
            'i' => {
                if state.keyword(chunk, "f") {
                    return TokenType::Flow
                }
                if state.keyword(chunk, "mpl") {
                    return TokenType::Impl
                }
                if state.keyword(chunk, "size") {
                    return TokenType::BuiltinType
                }
                if state.keyword(chunk, "n") {
                    return TokenType::Keyword
                }
                if state.keyword(chunk, "8") {
                    return TokenType::BuiltinType
                }
                if state.keyword(chunk, "16") {
                    return TokenType::BuiltinType
                }
                if state.keyword(chunk, "32") {
                    return TokenType::BuiltinType
                }
                if state.keyword(chunk, "64") {
                    return TokenType::BuiltinType
                }
            },
            'l' => {
                if state.keyword(chunk, "et") {
                    return TokenType::Keyword
                }
                if state.keyword(chunk, "oop") {
                    return TokenType::Looping
                }
            },
            'm' => {
                if state.keyword(chunk, "atch") {
                    return TokenType::Flow
                }
                if state.keyword(chunk, "ut") {
                    return TokenType::Keyword
                }
                if state.keyword(chunk, "o") {
                    if state.keyword(chunk, "d") {
                        return TokenType::Keyword
                    }
                    if state.keyword(chunk, "ve") {
                        return TokenType::Keyword
                    }
                }
            },
            'p' => { // pub
                if state.keyword(chunk, "ub") {
                    return TokenType::Keyword
                }
            },
            'r' => {
                if state.keyword(chunk, "e") {
                    if state.keyword(chunk, "f") {
                        return TokenType::Keyword
                    }
                    if state.keyword(chunk, "turn") {
                        return TokenType::Flow
                    }
                }
            },
            's' => {
                if state.keyword(chunk, "elf") {
                    return TokenType::Keyword
                }
                if state.keyword(chunk, "uper") {
                    return TokenType::Keyword
                }
                if state.keyword(chunk, "t") {
                    if state.keyword(chunk, "atic") {
                        return TokenType::Keyword
                    }
                    if state.keyword(chunk, "r") {
                        if state.keyword(chunk, "uct") {
                            return TokenType::TypeDef
                        }
                        return TokenType::BuiltinType
                    }
                }
            },
            't' => {
                if state.keyword(chunk, "ype") {
                    return TokenType::Keyword
                }
                if state.keyword(chunk, "r") {
                    if state.keyword(chunk, "ait") {
                        return TokenType::TypeDef
                    }
                    if state.keyword(chunk, "ue") {
                        return TokenType::Bool
                    }
                }
            },
            'u' => { // use
                
                if state.keyword(chunk, "nsafe") {
                    return TokenType::Keyword
                }
                if state.keyword(chunk, "8") {
                    return TokenType::BuiltinType
                }
                if state.keyword(chunk, "16") {
                    return TokenType::BuiltinType
                }
                if state.keyword(chunk, "32") {
                    return TokenType::BuiltinType
                }
                if state.keyword(chunk, "64") {
                    return TokenType::BuiltinType
                }
                if state.keyword(chunk, "s") {
                    if state.keyword(chunk, "ize") {
                        return TokenType::BuiltinType
                    }
                    if state.keyword(chunk, "e") {
                        return TokenType::Keyword
                    }
                }
            },
            'w' => { // use
                if state.keyword(chunk, "h") {
                    if state.keyword(chunk, "ere") {
                        return TokenType::Keyword
                    }
                    if state.keyword(chunk, "ile") {
                        return TokenType::Looping
                    }
                }
            },
            
            _ => {}
        }
        if state.next == '(' {
            return TokenType::Call;
        }
        else {
            return TokenType::Identifier;
        }
    }
    
    // because rustfmt is such an insane shitpile to compile or use as a library, here is a stupid version.
    pub fn auto_format(flat_text: &Vec<char>, token_chunks: &Vec<TokenChunk>, force_newlines: bool) -> FormatOutput {
        
        // extra spacey setting that rustfmt seems to do, but i don't like
        let extra_spacey = false;
        let pre_spacey = true;
        
        let mut out = FormatOutput::new();
        let mut tp = TokenParser::new(flat_text, token_chunks);
        
        struct ParenStack {
            expecting_newlines: bool,
            expected_indent: usize,
            angle_counter: usize
        }
        
        let mut paren_stack: Vec<ParenStack> = Vec::new();
        
        paren_stack.push(ParenStack {
            expecting_newlines: true,
            expected_indent: 0,
            angle_counter: 0
        });
        out.new_line();
        
        let mut first_on_line = true;
        let mut first_after_open = false;
        let mut expected_indent = 0;
        let mut is_unary_operator = true;
        let mut in_multline_comment = false;
        let mut in_singleline_comment = false;
        let mut in_multiline_string = false;
        while tp.advance() {
            
            match tp.cur_type() {
                TokenType::Whitespace => {
                    if in_singleline_comment || in_multline_comment {
                        out.extend(tp.cur_chunk());
                    }
                    else if !first_on_line && tp.next_type() != TokenType::Newline
                        && tp.prev_type() != TokenType::ParenOpen
                        && tp.prev_type() != TokenType::Namespace
                        && tp.prev_type() != TokenType::Delimiter
                        && (tp.prev_type() != TokenType::Operator || (tp.prev_char() == '>' || tp.prev_char() == '<')) {
                        out.add_space();
                    }
                },
                TokenType::Newline => {
                    in_singleline_comment = false;
                    //paren_stack.last_mut().unwrap().angle_counter = 0;
                    if in_singleline_comment || in_multline_comment || in_multiline_string {
                        out.new_line();
                        first_on_line = true;
                    }
                    else {
                        if first_on_line {
                            out.indent(expected_indent);
                        }
                        else {
                            out.strip_space();
                        }
                        if first_after_open {
                            paren_stack.last_mut().unwrap().expecting_newlines = true;
                            expected_indent += 4;
                        }
                        if paren_stack.last_mut().unwrap().expecting_newlines { // only insert when expecting newlines
                            first_after_open = false;
                            out.new_line();
                            first_on_line = true;
                        }
                    }
                },
                TokenType::Eof => {break},
                TokenType::ParenOpen => {
                    if first_on_line {
                        out.indent(expected_indent);
                    }
                    
                    paren_stack.push(ParenStack {
                        expecting_newlines: force_newlines,
                        expected_indent: expected_indent,
                        angle_counter: 0
                    });
                    first_after_open = true;
                    is_unary_operator = true;
                    
                    let is_curly = tp.cur_char() == '{';
                    if tp.cur_char() == '(' && (
                        tp.prev_type() == TokenType::Flow || tp.prev_type() == TokenType::Looping || tp.prev_type() == TokenType::Keyword
                    ) {
                        out.add_space();
                    }
                    if pre_spacey && is_curly && !first_on_line && tp.prev_type() != TokenType::Namespace {
                        if tp.prev_char() != ' ' && tp.prev_char() != '{'
                            && tp.prev_char() != '[' && tp.prev_char() != '(' && tp.prev_char() != ':' && tp.prev_char() != '!' {
                            out.add_space();
                        }
                    }
                    else if !pre_spacey {
                        out.strip_space();
                    }
                    
                    out.extend(tp.cur_chunk());
                    
                    if extra_spacey && is_curly && tp.next_type() != TokenType::Newline {
                        out.add_space();
                    }
                    first_on_line = false;
                },
                TokenType::ParenClose => {
                    
                    out.strip_space();
                    
                    let expecting_newlines = paren_stack.last().unwrap().expecting_newlines;
                    
                    if extra_spacey && tp.cur_char() == '}' && !expecting_newlines {
                        out.add_space();
                    }
                    
                    first_after_open = false;
                    if !first_on_line && expecting_newlines { // we are expecting newlines!
                        out.new_line();
                        first_on_line = true;
                    }
                    
                    expected_indent = if paren_stack.len()>1 {
                        paren_stack.pop().unwrap().expected_indent
                    }
                    else {
                        0
                    };
                    if first_on_line {
                        first_on_line = false;
                        out.indent(expected_indent);
                    }
                    if tp.cur_char() == '}' {
                        is_unary_operator = true;
                    }
                    else {
                        is_unary_operator = false;
                    }
                    out.extend(tp.cur_chunk());
                },
                TokenType::CommentLine => {
                    in_singleline_comment = true;
                    if first_on_line {
                        first_on_line = false;
                        out.indent(expected_indent);
                    }
                    else {
                        out.add_space();
                    }
                    out.extend(tp.cur_chunk());
                },
                TokenType::CommentMultiBegin => {
                    in_multline_comment = true;
                    if first_on_line {
                        first_on_line = false;
                        out.indent(expected_indent);
                    }
                    out.extend(tp.cur_chunk());
                },
                TokenType::CommentChunk => {
                    if first_on_line {
                        first_on_line = false;
                    }
                    out.extend(tp.cur_chunk());
                },
                TokenType::CommentMultiEnd => {
                    in_multline_comment = false;
                    if first_on_line {
                        first_on_line = false;
                    }
                    out.extend(tp.cur_chunk());
                },
                TokenType::StringMultiBegin => {
                    in_multiline_string = true;
                    if first_on_line {
                        first_on_line = false;
                        out.indent(expected_indent);
                    }
                    expected_indent += 4;
                    out.extend(tp.cur_chunk());
                },
                TokenType::StringChunk => {
                    if first_on_line {
                        first_on_line = false;
                        out.indent(expected_indent);
                    }
                    out.extend(tp.cur_chunk());
                },
                TokenType::StringMultiEnd => {
                    expected_indent -= 4;
                    in_multiline_string = false;
                    if first_on_line {
                        first_on_line = false;
                        out.indent(expected_indent);
                    }
                    out.extend(tp.cur_chunk());
                },
                TokenType::Colon => {
                    is_unary_operator = true;
                    out.strip_space();
                    out.extend(tp.cur_chunk());
                    if tp.next_type() != TokenType::Whitespace && tp.next_type() != TokenType::Newline {
                        out.add_space();
                    }
                },
                TokenType::Delimiter => {
                    if first_on_line {
                        first_on_line = false;
                        out.indent(expected_indent);
                    }
                    else {
                        out.strip_space();
                    }
                    out.extend(tp.cur_chunk());
                    if paren_stack.last_mut().unwrap().angle_counter == 0 // otherwise our generics multiline
                        && paren_stack.last().unwrap().expecting_newlines == true
                        && tp.next_type() != TokenType::Newline { // we are expecting newlines!
                        // scan forward to see if we really need a newline.
                        for next in (tp.index + 1)..tp.tokens.len() {
                            if tp.tokens[next].token_type == TokenType::Newline {
                                break;
                            }
                            if !tp.tokens[next].token_type.should_ignore() {
                                out.new_line();
                                first_on_line = true;
                                break;
                            }
                        }
                    }
                    else if tp.next_type() != TokenType::Newline {
                        out.add_space();
                    }
                    is_unary_operator = true;
                },
                TokenType::Operator => {
                    
                    // detect ++ and -- and execute insert or delete macros
                    
                    let mut is_closing_angle = false;
                    if tp.cur_char() == '<' {
                        paren_stack.last_mut().unwrap().angle_counter += 1;
                    }
                    else if tp.cur_char() == '>' {
                        let last = paren_stack.last_mut().unwrap();
                        last.angle_counter = last.angle_counter.max(1) - 1;
                        is_closing_angle = true;
                    }
                    else if tp.cur_char() != '&' && tp.cur_char() != '*' { // anything else resets the angle counter
                        paren_stack.last_mut().unwrap().angle_counter = 0
                    }
                    else {
                        paren_stack.last_mut().unwrap().angle_counter = 0
                    }
                    
                    if first_on_line {
                        first_on_line = false;
                        let extra_indent = if is_closing_angle || is_unary_operator {0}else {4};
                        out.indent(expected_indent + extra_indent);
                    }
                    
                    if (is_unary_operator && (tp.cur_char() == '-' || tp.cur_char() == '*' || tp.cur_char() == '&'))
                        || tp.cur_char() == '!' || tp.cur_char() == '.' || tp.cur_char() == '<' || tp.cur_char() == '>' {
                        out.extend(tp.cur_chunk());
                    }
                    else {
                        out.add_space();
                        out.extend(tp.cur_chunk());
                        if tp.next_type() != TokenType::Newline {
                            out.add_space();
                        }
                    }
                    
                    is_unary_operator = true;
                },
                TokenType::Identifier | TokenType::BuiltinType | TokenType::TypeName | TokenType::ThemeName => { // these dont reset the angle counter
                    is_unary_operator = false;
                    
                    first_after_open = false;
                    if first_on_line {
                        first_on_line = false;
                        let extra_indent = if paren_stack.last_mut().unwrap().angle_counter >0 {4}else {0};
                        out.indent(expected_indent + extra_indent);
                    }
                    out.extend(tp.cur_chunk());
                },
                TokenType::Namespace => {
                    is_unary_operator = true;
                    
                    first_after_open = false;
                    if first_on_line {
                        first_on_line = false;
                        out.indent(expected_indent);
                    }
                    out.extend(tp.cur_chunk());
                },
                // these are followed by unary operators (some)
                TokenType::TypeDef | TokenType::Impl | TokenType::Fn | TokenType::Hash | TokenType::Splat |
                TokenType::Keyword | TokenType::Flow | TokenType::Looping => {
                    is_unary_operator = true;
                    paren_stack.last_mut().unwrap().angle_counter = 0;
                    
                    first_after_open = false;
                    if first_on_line {
                        first_on_line = false;
                        out.indent(expected_indent);
                    }
                    out.extend(tp.cur_chunk());
                },
                // these are followeable by non unary operators
                TokenType::Macro | TokenType::Call | TokenType::String | TokenType::Regex | TokenType::Number | TokenType::Color |
                TokenType::Bool | TokenType::Unexpected | TokenType::Error | TokenType::Warning | TokenType::Defocus => {
                    is_unary_operator = false;
                    paren_stack.last_mut().unwrap().angle_counter = 0;
                    
                    first_after_open = false;
                    if first_on_line {
                        first_on_line = false;
                        out.indent(expected_indent);
                    }
                    out.extend(tp.cur_chunk());
                    
                },
            }
        };
        out
    }
}
