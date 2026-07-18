use crate::ast::*;
use crate::diag::Diag;
use crate::lexer::{Kw, StrPart, Token, TokenKind};

pub struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn peek_kind(&self) -> Option<&TokenKind> {
        self.peek().map(|t| &t.kind)
    }

    fn advance(&mut self) -> Option<&Token> {
        if self.pos < self.tokens.len() {
            let t = &self.tokens[self.pos];
            self.pos += 1;
            Some(t)
        } else {
            None
        }
    }

    fn advance_if(&mut self, kind: &TokenKind) -> bool {
        if self.peek_kind() == Some(kind) {
            self.advance();
            true
        } else {
            false
        }
    }
    
    fn advance_if_kw(&mut self, kw: Kw) -> bool {
        if let Some(TokenKind::Kw(k)) = self.peek_kind() {
            if *k == kw {
                self.advance();
                return true;
            }
        }
        false
    }
    
    fn advance_if_op(&mut self, op: &str) -> bool {
        if let Some(TokenKind::Op(o)) = self.peek_kind() {
            if *o == op {
                self.advance();
                return true;
            }
        }
        false
    }

    fn expect(&mut self, kind: &TokenKind, expected_msg: &str) -> Result<&Token, Diag> {
        if self.peek_kind() == Some(kind) {
            Ok(self.advance().unwrap())
        } else {
            self.err(expected_msg)
        }
    }
    
    fn expect_kw(&mut self, kw: Kw, expected_msg: &str) -> Result<&Token, Diag> {
        if let Some(TokenKind::Kw(k)) = self.peek_kind() {
            if *k == kw {
                return Ok(self.advance().unwrap());
            }
        }
        self.err(expected_msg)
    }
    
    fn expect_op(&mut self, op: &str, expected_msg: &str) -> Result<&Token, Diag> {
        if let Some(TokenKind::Op(o)) = self.peek_kind() {
            if *o == op {
                return Ok(self.advance().unwrap());
            }
        }
        self.err(expected_msg)
    }

    fn expect_ident(&mut self, expected_msg: &str) -> Result<(String, Span), Diag> {
        if let Some(t) = self.peek() {
            if let TokenKind::Ident(id) = &t.kind {
                let s = id.clone();
                let span = Span { line: t.line, col: t.col };
                self.advance();
                return Ok((s, span));
            }
        }
        self.err(expected_msg)
    }

    fn skip_newlines(&mut self) {
        while self.advance_if(&TokenKind::Newline) {}
    }

    fn err<T>(&self, expected_msg: &str) -> Result<T, Diag> {
        let (line, col) = if let Some(t) = self.peek() {
            (t.line, t.col)
        } else if let Some(t) = self.tokens.last() {
            (t.line, t.col)
        } else {
            (1, 1)
        };
        
        let found = match self.peek_kind() {
            Some(TokenKind::Kw(k)) => format!("keyword '{}'", k.as_str()),
            Some(TokenKind::Ident(i)) => format!("identifier '{}'", i),
            Some(TokenKind::Int(i)) => format!("integer '{}'", i),
            Some(TokenKind::Float(f)) => format!("float '{}'", f),
            Some(TokenKind::Str(_)) => "string".to_string(),
            Some(TokenKind::Lit(l)) => format!("'{}'", l),
            Some(TokenKind::Op(o)) => format!("'{}'", o),
            Some(TokenKind::Newline) => "newline".to_string(),
            Some(TokenKind::Indent) => "indent".to_string(),
            Some(TokenKind::Dedent) => "dedent".to_string(),
            Some(TokenKind::Eof) => "EOF".to_string(),
            None => "EOF".to_string(),
        };

        Err(Diag {
            code: "E0100", // Parse error
            msg: format!("expected {}, found {}", expected_msg, found),
            line,
            col,
        })
    }

    pub fn parse_file(&mut self) -> Result<File, Diag> {
        let mut uses = Vec::new();
        self.skip_newlines();
        while self.advance_if_kw(Kw::Use) {
            let t = self.tokens[self.pos - 1].clone();
            let span = Span { line: t.line, col: t.col };
            
            let path = if let Some(TokenKind::Str(parts)) = self.peek_kind() {
                if parts.len() == 1 {
                    if let StrPart::Text(txt) = &parts[0] {
                        let p = txt.clone();
                        self.advance();
                        p
                    } else {
                        return self.err("plain string path in use declaration");
                    }
                } else {
                    return self.err("plain string path in use declaration (no interpolation)");
                }
            } else if let Some(TokenKind::Ident(id)) = self.peek_kind() {
                // simple path_ident
                let mut p = id.clone();
                self.advance();
                while self.advance_if_op("/") {
                    if let Ok((id, _)) = self.expect_ident("identifier in use path") {
                        p.push('/');
                        p.push_str(&id);
                    }
                }
                p
            } else {
                return self.err("path identifier or string in use declaration");
            };
            self.expect(&TokenKind::Newline, "newline after use declaration")?;
            uses.push(UseDecl { span, path });
            self.skip_newlines();
        }

        let mut items = Vec::new();
        while self.peek_kind() != Some(&TokenKind::Eof) && self.peek().is_some() {
            items.push(self.parse_top_item()?);
            self.skip_newlines();
        }

        Ok(File { uses, items })
    }

    fn parse_top_item(&mut self) -> Result<TopItem, Diag> {
        if self.peek_kind() == Some(&TokenKind::Kw(Kw::Fn)) {
            Ok(TopItem::Fn(self.parse_fn_decl()?))
        } else if self.peek_kind() == Some(&TokenKind::Kw(Kw::Type)) {
            Ok(TopItem::Type(self.parse_type_decl()?))
        } else if self.peek_kind() == Some(&TokenKind::Kw(Kw::Let)) || self.peek_kind() == Some(&TokenKind::Kw(Kw::Mut)) {
            Ok(TopItem::Let(self.parse_let_stmt()?))
        } else {
            Ok(TopItem::Stmt(self.parse_statement()?))
        }
    }

    fn parse_fn_decl(&mut self) -> Result<FnDecl, Diag> {
        let t = self.expect_kw(Kw::Fn, "fn")?.clone();
        let span = Span { line: t.line, col: t.col };
        
        // Receiver or just name
        let (mut name, mut _nspan) = self.expect_ident("function name")?;
        let mut receiver = None;
        if self.advance_if_op(".") {
            receiver = Some(name);
            let (n, sp) = self.expect_ident("method name")?;
            name = n;
            _nspan = sp;
        }
        
        self.expect_op("(", "'(' after function name")?;
        let mut params = Vec::new();
        if !self.advance_if_op(")") {
            loop {
                let (pname, pspan) = self.expect_ident("parameter name")?;
                let mut typ = None;
                if self.advance_if_op(":") {
                    typ = Some(self.parse_type_expr()?);
                }
                params.push(Param { span: pspan, name: pname, typ });
                if !self.advance_if_op(",") {
                    break;
                }
            }
            self.expect_op(")", "')' after parameters")?;
        }
        
        let mut ret_type = None;
        if self.advance_if_op("->") {
            ret_type = Some(self.parse_type_expr()?);
        }
        
        let body = self.parse_block()?;
        Ok(FnDecl { span, receiver, name, params, ret_type, body })
    }

    fn parse_type_decl(&mut self) -> Result<TypeDecl, Diag> {
        let t = self.expect_kw(Kw::Type, "type")?.clone();
        let span = Span { line: t.line, col: t.col };
        let (name, _) = self.expect_ident("type name")?;
        
        let kind = if self.advance_if_op("=") {
            // Enum
            let mut variants = Vec::new();
            loop {
                let (vname, vspan) = self.expect_ident("variant name")?;
                let mut fields = Vec::new();
                if self.advance_if_op("(") {
                    if !self.advance_if_op(")") {
                        loop {
                            let (fname, fspan) = self.expect_ident("field name")?;
                            self.expect_op(":", "':' after field name")?;
                            let typ = self.parse_type_expr()?;
                            fields.push(FieldDef { span: fspan, name: fname, typ });
                            if !self.advance_if_op(",") {
                                break;
                            }
                        }
                        self.expect_op(")", "')' after variant fields")?;
                    }
                }
                variants.push(VariantDef { span: vspan, name: vname, fields });
                if !self.advance_if_kw(Kw::Or) {
                    break;
                }
            }
            self.expect(&TokenKind::Newline, "newline after enum declaration")?;
            TypeDeclKind::Enum(variants)
        } else {
            // Record
            self.expect(&TokenKind::Newline, "newline before record fields")?;
            self.expect(&TokenKind::Indent, "indent for record fields")?;
            let mut fields = Vec::new();
            while self.peek_kind() != Some(&TokenKind::Dedent) {
                let (fname, fspan) = self.expect_ident("field name")?;
                self.expect_op(":", "':' after field name")?;
                let typ = self.parse_type_expr()?;
                self.expect(&TokenKind::Newline, "newline after field")?;
                fields.push(FieldDef { span: fspan, name: fname, typ });
            }
            self.expect(&TokenKind::Dedent, "dedent after record fields")?;
            TypeDeclKind::Record(fields)
        };
        
        Ok(TypeDecl { span, name, kind })
    }

    fn parse_type_expr(&mut self) -> Result<TypeExpr, Diag> {
        let t = self.peek().ok_or_else(|| self.err::<()>("type expression").unwrap_err())?.clone();
        let span = Span { line: t.line, col: t.col };
        
        let kind = if self.advance_if_kw(Kw::Fn) {
            self.expect_op("(", "'(' in fn type")?;
            let mut args = Vec::new();
            if !self.advance_if_op(")") {
                loop {
                    args.push(self.parse_type_expr()?);
                    if !self.advance_if_op(",") {
                        break;
                    }
                }
                self.expect_op(")", "')' in fn type")?;
            }
            let mut ret = None;
            if self.advance_if_op("->") {
                ret = Some(Box::new(self.parse_type_expr()?));
            }
            TypeExprKind::Fn(args, ret)
        } else {
            let (name, _) = self.expect_ident("type name")?;
            let mut args = Vec::new();
            if self.advance_if_op("[") {
                if !self.advance_if_op("]") {
                    loop {
                        args.push(self.parse_type_expr()?);
                        if !self.advance_if_op(",") {
                            break;
                        }
                    }
                    self.expect_op("]", "']' after type arguments")?;
                }
            }
            TypeExprKind::Named(name, args)
        };
        
        let optional = self.advance_if_op("?");
        let mut result = false;
        if self.advance_if_kw(Kw::Or) {
            let (name, _) = self.expect_ident("'error'")?;
            if name != "error" {
                return self.err("'error' after 'or'");
            }
            result = true;
        }
        
        Ok(TypeExpr { span, kind, optional, result })
    }

    fn parse_block(&mut self) -> Result<Block, Diag> {
        let t = self.expect(&TokenKind::Newline, "newline before block")?.clone();
        let span = Span { line: t.line, col: t.col };
        self.expect(&TokenKind::Indent, "indent for block")?;
        let mut stmts = Vec::new();
        while self.peek_kind() != Some(&TokenKind::Dedent) {
            stmts.push(self.parse_statement()?);
        }
        self.expect(&TokenKind::Dedent, "dedent after block")?;
        Ok(Block { span, stmts })
    }

    fn parse_let_stmt(&mut self) -> Result<LetStmt, Diag> {
        let mut is_mut = false;
        let t = if self.advance_if_kw(Kw::Mut) {
            is_mut = true;
            self.tokens[self.pos - 1].clone()
        } else {
            self.expect_kw(Kw::Let, "'let' or 'mut'")?.clone()
        };
        let span = Span { line: t.line, col: t.col };
        
        let (name, _) = self.expect_ident("variable name")?;
        self.expect_op("=", "'=' after variable name")?;
        let init = self.parse_expr()?;
        self.expect(&TokenKind::Newline, "newline after let statement")?;
        Ok(LetStmt { span, is_mut, name, init })
    }

    fn parse_statement(&mut self) -> Result<Statement, Diag> {
        if self.peek_kind() == Some(&TokenKind::Kw(Kw::Let)) || self.peek_kind() == Some(&TokenKind::Kw(Kw::Mut)) {
            return Ok(Statement::Let(self.parse_let_stmt()?));
        }
        if self.peek_kind() == Some(&TokenKind::Kw(Kw::If)) {
            return Ok(Statement::If(self.parse_if_stmt()?));
        }
        if self.peek_kind() == Some(&TokenKind::Kw(Kw::While)) {
            return Ok(Statement::While(self.parse_while_stmt()?));
        }
        if self.peek_kind() == Some(&TokenKind::Kw(Kw::For)) {
            return Ok(Statement::For(self.parse_for_stmt()?));
        }
        if self.peek_kind() == Some(&TokenKind::Kw(Kw::Match)) {
            return Ok(Statement::Match(self.parse_match_stmt()?));
        }
        if self.peek_kind() == Some(&TokenKind::Kw(Kw::Return)) {
            return Ok(Statement::Return(self.parse_return_stmt()?));
        }
        if self.peek_kind() == Some(&TokenKind::Kw(Kw::Break)) {
            let t = self.advance().unwrap().clone();
            self.expect(&TokenKind::Newline, "newline after break")?;
            return Ok(Statement::Break(Span { line: t.line, col: t.col }));
        }
        if self.peek_kind() == Some(&TokenKind::Kw(Kw::Continue)) {
            let t = self.advance().unwrap().clone();
            self.expect(&TokenKind::Newline, "newline after continue")?;
            return Ok(Statement::Continue(Span { line: t.line, col: t.col }));
        }
        
        // Either assign or expr
        // We'll parse an expression. If it is an lvalue and followed by assign op, it's assign.
        let expr = self.parse_expr()?;
        let assign_op = match self.peek_kind() {
            Some(TokenKind::Op("=")) => Some(AssignOp::Eq),
            Some(TokenKind::Op("+=")) => Some(AssignOp::AddEq),
            Some(TokenKind::Op("-=")) => Some(AssignOp::SubEq),
            Some(TokenKind::Op("*=")) => Some(AssignOp::MulEq),
            Some(TokenKind::Op("/=")) => Some(AssignOp::DivEq),
            _ => None,
        };
        
        if let Some(op) = assign_op {
            self.advance(); // consume op
            let lvalue = self.expr_to_lvalue(expr)?;
            let rhs = self.parse_expr()?;
            self.expect(&TokenKind::Newline, "newline after assignment")?;
            Ok(Statement::Assign(AssignStmt { span: lvalue.span.clone(), target: lvalue, op, rhs }))
        } else {
            self.expect(&TokenKind::Newline, "newline after expression statement")?;
            Ok(Statement::Expr(expr))
        }
    }
    
    fn expr_to_lvalue(&self, expr: Expr) -> Result<LValue, Diag> {
        // Unwind field and index accesses
        let mut tail = Vec::new();
        let mut current = expr;
        loop {
            match current.kind {
                ExprKind::Ident(id) => {
                    tail.reverse();
                    return Ok(LValue { span: current.span, name: id, tail });
                }
                ExprKind::Field(inner, f) => {
                    tail.push(LValueTail::Field(f));
                    current = *inner;
                }
                ExprKind::Index(inner, idx) => {
                    tail.push(LValueTail::Index(*idx));
                    current = *inner;
                }
                _ => {
                    return Err(Diag {
                        code: "E0101",
                        msg: "invalid left-hand side of assignment".to_string(),
                        line: current.span.line,
                        col: current.span.col,
                    });
                }
            }
        }
    }

    fn parse_if_stmt(&mut self) -> Result<IfStmt, Diag> {
        let t = self.expect_kw(Kw::If, "if")?.clone();
        let span = Span { line: t.line, col: t.col };
        let cond = self.parse_expr()?;
        let then_block = self.parse_block()?;
        
        let mut elifs = Vec::new();
        while self.advance_if_kw(Kw::Elif) {
            let elif_cond = self.parse_expr()?;
            let elif_block = self.parse_block()?;
            elifs.push((elif_cond, elif_block));
        }
        
        let mut else_block = None;
        if self.advance_if_kw(Kw::Else) {
            else_block = Some(self.parse_block()?);
        }
        
        Ok(IfStmt { span, cond, then_block, elifs, else_block })
    }

    fn parse_while_stmt(&mut self) -> Result<WhileStmt, Diag> {
        let t = self.expect_kw(Kw::While, "while")?.clone();
        let span = Span { line: t.line, col: t.col };
        let cond = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(WhileStmt { span, cond, body })
    }

    fn parse_for_stmt(&mut self) -> Result<ForStmt, Diag> {
        let t = self.expect_kw(Kw::For, "for")?.clone();
        let span = Span { line: t.line, col: t.col };
        let (name, _) = self.expect_ident("loop variable")?;
        self.expect_kw(Kw::In, "'in'")?;
        let iter = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(ForStmt { span, name, iter, body })
    }

    fn parse_match_stmt(&mut self) -> Result<MatchStmt, Diag> {
        let t = self.expect_kw(Kw::Match, "match")?.clone();
        let span = Span { line: t.line, col: t.col };
        let expr = self.parse_expr()?;
        self.expect(&TokenKind::Newline, "newline before match arms")?;
        self.expect(&TokenKind::Indent, "indent for match arms")?;
        
        let mut arms = Vec::new();
        while self.peek_kind() != Some(&TokenKind::Dedent) {
            let arm_span = Span { line: self.peek().unwrap().line, col: self.peek().unwrap().col };
            let pattern = self.parse_pattern()?;
            let body = self.parse_block()?;
            arms.push(MatchArm { span: arm_span, pattern, body });
        }
        self.expect(&TokenKind::Dedent, "dedent after match arms")?;
        
        Ok(MatchStmt { span, expr, arms })
    }

    fn parse_pattern(&mut self) -> Result<Pattern, Diag> {
        let t = self.peek().ok_or_else(|| self.err::<()>("pattern").unwrap_err())?.clone();
        let span = Span { line: t.line, col: t.col };
        
        if self.advance_if_ident("_") {
            return Ok(Pattern::Wildcard(span));
        }
        
        if let Some(lit) = self.try_parse_literal() {
            return Ok(Pattern::Literal(lit));
        }
        
        let (name, _) = self.expect_ident("pattern")?;
        if self.advance_if_op("(") {
            let mut binds = Vec::new();
            if !self.advance_if_op(")") {
                loop {
                    let (b, _) = self.expect_ident("binding name")?;
                    binds.push(b);
                    if !self.advance_if_op(",") {
                        break;
                    }
                }
                self.expect_op(")", "')' in pattern")?;
            }
            Ok(Pattern::Variant(span, name, binds))
        } else {
            // A pattern like `dot` is just a variant with no binds, but grammar:
            // pattern = "_" | literal | ident , [ "(" , ident , { "," , ident } , ")" ] ;
            Ok(Pattern::Variant(span, name, vec![]))
        }
    }

    fn advance_if_ident(&mut self, s: &str) -> bool {
        if let Some(TokenKind::Ident(i)) = self.peek_kind() {
            if i == s {
                self.advance();
                return true;
            }
        }
        false
    }

    fn try_parse_literal(&mut self) -> Option<Literal> {
        let kind = self.peek_kind()?.clone();
        match kind {
            TokenKind::Int(s) => { self.advance(); Some(Literal::Int(s)) }
            TokenKind::Float(s) => { self.advance(); Some(Literal::Float(s)) }
            TokenKind::Lit(s) => {
                self.advance();
                match s {
                    "true" => Some(Literal::Bool(true)),
                    "false" => Some(Literal::Bool(false)),
                    "none" => Some(Literal::None),
                    _ => None,
                }
            }
            // Strings are complex if they have interpolation, but a pattern literal string has no interp.
            TokenKind::Str(parts) => {
                if parts.len() == 1 {
                    if let StrPart::Text(t) = &parts[0] {
                        let txt = t.clone();
                        self.advance();
                        return Some(Literal::Str(txt));
                    }
                }
                None
            }
            _ => None
        }
    }

    fn parse_return_stmt(&mut self) -> Result<ReturnStmt, Diag> {
        let t = self.expect_kw(Kw::Return, "return")?.clone();
        let span = Span { line: t.line, col: t.col };
        let mut expr = None;
        if self.peek_kind() != Some(&TokenKind::Newline) {
            expr = Some(self.parse_expr()?);
        }
        self.expect(&TokenKind::Newline, "newline after return")?;
        Ok(ReturnStmt { span, expr })
    }

    // Expressions
    fn parse_expr(&mut self) -> Result<Expr, Diag> {
        self.parse_or_expr()
    }

    fn parse_or_expr(&mut self) -> Result<Expr, Diag> {
        let mut left = self.parse_and_expr()?;
        while self.advance_if_kw(Kw::Or) {
            let right = self.parse_and_expr()?;
            left = Expr { span: left.span.clone(), kind: ExprKind::Binary(BinOp::Or, Box::new(left), Box::new(right)) };
        }
        Ok(left)
    }

    fn parse_and_expr(&mut self) -> Result<Expr, Diag> {
        let mut left = self.parse_cmp_expr()?;
        while self.advance_if_kw(Kw::And) {
            let right = self.parse_cmp_expr()?;
            left = Expr { span: left.span.clone(), kind: ExprKind::Binary(BinOp::And, Box::new(left), Box::new(right)) };
        }
        Ok(left)
    }

    fn parse_cmp_expr(&mut self) -> Result<Expr, Diag> {
        let left = self.parse_range_expr()?;
        
        let op = match self.peek_kind() {
            Some(TokenKind::Op("==")) => Some(BinOp::Eq),
            Some(TokenKind::Op("!=")) => Some(BinOp::Neq),
            Some(TokenKind::Op("<")) => Some(BinOp::Lt),
            Some(TokenKind::Op("<=")) => Some(BinOp::Leq),
            Some(TokenKind::Op(">")) => Some(BinOp::Gt),
            Some(TokenKind::Op(">=")) => Some(BinOp::Geq),
            _ => None,
        };
        
        if let Some(bin_op) = op {
            self.advance();
            let right = self.parse_range_expr()?;
            Ok(Expr { span: left.span.clone(), kind: ExprKind::Binary(bin_op, Box::new(left), Box::new(right)) })
        } else {
            Ok(left)
        }
    }

    fn parse_range_expr(&mut self) -> Result<Expr, Diag> {
        let left = self.parse_add_expr()?;
        let op = match self.peek_kind() {
            Some(TokenKind::Op("..")) => Some(BinOp::Range),
            Some(TokenKind::Op("..=")) => Some(BinOp::RangeInc),
            _ => None,
        };
        
        if let Some(bin_op) = op {
            self.advance();
            // Can be unbound: `0..`
            let right = if let Some(TokenKind::Op(o)) = self.peek_kind() {
                if *o == ")" || *o == "]" || *o == "}" || *o == "," {
                    None
                } else {
                    Some(self.parse_add_expr()?)
                }
            } else if let Some(TokenKind::Newline | TokenKind::Dedent) = self.peek_kind() {
                None
            } else if let Some(TokenKind::Kw(Kw::If | Kw::While | Kw::For | Kw::In | Kw::Return | Kw::Else | Kw::Elif)) = self.peek_kind() {
                // End of expression
                None
            } else {
                Some(self.parse_add_expr()?)
            };
            
            if let Some(r) = right {
                Ok(Expr { span: left.span.clone(), kind: ExprKind::Binary(bin_op, Box::new(left), Box::new(r)) })
            } else {
                // Unbounded right. AST doesn't explicitly have Unary Range, so we encode as Range(left, Dummy)
                // Oh wait, grammar says `range_expr = add_expr , [ ( ".." | "..=" ) , [ add_expr ] ] ;`
                // Let's just use a dummy None literal for unbounded.
                let dummy_span = left.span.clone(); // Use same span
                let none_expr = Expr { span: dummy_span, kind: ExprKind::Literal(Literal::None) };
                Ok(Expr { span: left.span.clone(), kind: ExprKind::Binary(bin_op, Box::new(left), Box::new(none_expr)) })
            }
        } else {
            Ok(left)
        }
    }

    fn parse_add_expr(&mut self) -> Result<Expr, Diag> {
        let mut left = self.parse_mul_expr()?;
        loop {
            let op = match self.peek_kind() {
                Some(TokenKind::Op("+")) => Some(BinOp::Add),
                Some(TokenKind::Op("-")) => Some(BinOp::Sub),
                _ => None,
            };
            if let Some(bin_op) = op {
                self.advance();
                let right = self.parse_mul_expr()?;
                left = Expr { span: left.span.clone(), kind: ExprKind::Binary(bin_op, Box::new(left), Box::new(right)) };
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_mul_expr(&mut self) -> Result<Expr, Diag> {
        let mut left = self.parse_pow_expr()?;
        loop {
            let op = match self.peek_kind() {
                Some(TokenKind::Op("*")) => Some(BinOp::Mul),
                Some(TokenKind::Op("/")) => Some(BinOp::Div),
                Some(TokenKind::Op("//")) => Some(BinOp::FloorDiv),
                Some(TokenKind::Op("%")) => Some(BinOp::Mod),
                _ => None,
            };
            if let Some(bin_op) = op {
                self.advance();
                let right = self.parse_pow_expr()?;
                left = Expr { span: left.span.clone(), kind: ExprKind::Binary(bin_op, Box::new(left), Box::new(right)) };
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_pow_expr(&mut self) -> Result<Expr, Diag> {
        let mut left = self.parse_unary_expr()?;
        if self.advance_if_op("**") {
            let right = self.parse_pow_expr()?; // Right-associative
            left = Expr { span: left.span.clone(), kind: ExprKind::Binary(BinOp::Pow, Box::new(left), Box::new(right)) };
        }
        Ok(left)
    }

    fn parse_unary_expr(&mut self) -> Result<Expr, Diag> {
        let t = self.peek().ok_or_else(|| self.err::<()>("expression").unwrap_err())?.clone();
        let span = Span { line: t.line, col: t.col };
        
        let op = if self.advance_if_op("-") {
            Some(UnOp::Neg)
        } else if self.advance_if_kw(Kw::Not) {
            Some(UnOp::Not)
        } else {
            None
        };
        
        let inner = self.parse_postfix()?;
        if let Some(un_op) = op {
            Ok(Expr { span, kind: ExprKind::Unary(un_op, Box::new(inner)) })
        } else {
            Ok(inner)
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, Diag> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.advance_if_op(".") {
                let (field, _) = self.expect_ident("field or method name")?;
                if self.peek_kind() == Some(&TokenKind::Op("(")) {
                    let args = self.parse_call_args()?;
                    // Desugar x.m(args) to m(x, args) or keep as method call?
                    // Let's keep as Field call for now or Field + Call?
                    // Primary produces Field. Then Call args.
                    let field_expr = Expr { span: expr.span.clone(), kind: ExprKind::Field(Box::new(expr), field) };
                    expr = Expr { span: field_expr.span.clone(), kind: ExprKind::Call(Box::new(field_expr), args) };
                } else {
                    expr = Expr { span: expr.span.clone(), kind: ExprKind::Field(Box::new(expr), field) };
                }
            } else if self.peek_kind() == Some(&TokenKind::Op("(")) {
                let args = self.parse_call_args()?;
                expr = Expr { span: expr.span.clone(), kind: ExprKind::Call(Box::new(expr), args) };
            } else if self.advance_if_op("[") {
                let idx = self.parse_expr()?;
                self.expect_op("]", "']' after index")?;
                expr = Expr { span: expr.span.clone(), kind: ExprKind::Index(Box::new(expr), Box::new(idx)) };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_call_args(&mut self) -> Result<Vec<CallArg>, Diag> {
        self.expect_op("(", "'('")?;
        let mut args = Vec::new();
        if !self.advance_if_op(")") {
            loop {
                // could be named `ident: expr`
                // We must peek ahead or parse as expr
                // Since `ident:` is distinct, we can peek two tokens ahead.
                // Or try parse ident and check if next is `:`
                let is_named = if let Some(TokenKind::Ident(_)) = self.peek_kind() {
                    if let Some(next) = self.tokens.get(self.pos + 1) {
                        next.kind == TokenKind::Op(":")
                    } else { false }
                } else { false };
                
                if is_named {
                    let (name, _) = self.expect_ident("argument name")?;
                    self.advance_if_op(":");
                    args.push(CallArg::Named(name, self.parse_expr()?));
                } else {
                    args.push(CallArg::Positional(self.parse_expr()?));
                }
                
                if !self.advance_if_op(",") {
                    break;
                }
            }
            self.expect_op(")", "')' after arguments")?;
        }
        Ok(args)
    }

    fn parse_primary(&mut self) -> Result<Expr, Diag> {
        let t = self.peek().ok_or_else(|| self.err::<()>("expression").unwrap_err())?.clone();
        let span = Span { line: t.line, col: t.col };
        
        if let Some(lit) = self.try_parse_literal() {
            return Ok(Expr { span, kind: ExprKind::Literal(lit) });
        }
        
        if let Some(TokenKind::Str(parts)) = self.peek_kind() {
            // Complex string (with interpolation)
            let mut interp = Vec::new();
            for p in parts.clone() {
                match p {
                    StrPart::Text(t) => interp.push(InterpPart::Text(t)),
                    StrPart::Interp(toks) => {
                        let mut sub_parser = Parser::new(&toks);
                        interp.push(InterpPart::Expr(sub_parser.parse_expr()?));
                        if sub_parser.pos < toks.len() {
                            return Err(Diag {
                                code: "E0100",
                                msg: "unexpected tokens in string interpolation".to_string(),
                                line: sub_parser.peek().unwrap().line,
                                col: sub_parser.peek().unwrap().col,
                            });
                        }
                    }
                }
            }
            self.advance();
            return Ok(Expr { span, kind: ExprKind::InterpStr(interp) });
        }
        
        if let Some(TokenKind::Ident(i)) = self.peek_kind() {
            let id = i.clone();
            self.advance();
            // Check if it's a record instantiation: `Ident {`
            if self.advance_if_op("{") {
                let mut fields = Vec::new();
                if !self.advance_if_op("}") {
                    loop {
                        let (fname, _) = self.expect_ident("field name")?;
                        self.expect_op(":", "':' after field name")?;
                        let fval = self.parse_expr()?;
                        fields.push((fname, fval));
                        if !self.advance_if_op(",") {
                            break;
                        }
                    }
                    self.expect_op("}", "'}' after record fields")?;
                }
                return Ok(Expr { span, kind: ExprKind::Record(id, fields) });
            }
            return Ok(Expr { span, kind: ExprKind::Ident(id) });
        }
        
        if self.advance_if_op("(") {
            let expr = self.parse_expr()?;
            self.expect_op(")", "')'")?;
            // A parenthesized expr AST representation just uses the inner expr, maybe we can keep parens but usually inner is enough.
            // Wait, we need to return the inner.
            return Ok(expr);
        }
        
        if self.advance_if_op("[") {
            let mut items = Vec::new();
            if !self.advance_if_op("]") {
                loop {
                    items.push(self.parse_expr()?);
                    if !self.advance_if_op(",") {
                        break;
                    }
                }
                self.expect_op("]", "']' after list")?;
            }
            return Ok(Expr { span, kind: ExprKind::List(items) });
        }
        
        if self.advance_if_op("{") {
            let mut items = Vec::new();
            if !self.advance_if_op("}") {
                loop {
                    let k = self.parse_expr()?;
                    self.expect_op(":", "':' in map literal")?;
                    let v = self.parse_expr()?;
                    items.push((k, v));
                    if !self.advance_if_op(",") {
                        break;
                    }
                }
                self.expect_op("}", "'}' after map")?;
            }
            return Ok(Expr { span, kind: ExprKind::Map(items) });
        }
        
        if self.advance_if_kw(Kw::Fn) {
            self.expect_op("(", "'(' in anonymous fn")?;
            let mut params = Vec::new();
            if !self.advance_if_op(")") {
                loop {
                    let (pname, pspan) = self.expect_ident("parameter name")?;
                    let mut typ = None;
                    if self.advance_if_op(":") {
                        typ = Some(self.parse_type_expr()?);
                    }
                    params.push(Param { span: pspan, name: pname, typ });
                    if !self.advance_if_op(",") {
                        break;
                    }
                }
                self.expect_op(")", "')' after anonymous fn parameters")?;
            }
            let mut ret_type = None;
            if self.advance_if_op("->") {
                ret_type = Some(Box::new(self.parse_type_expr()?));
            }
            let body = self.parse_block()?;
            return Ok(Expr { span, kind: ExprKind::Closure(params, ret_type, body) });
        }
        
        if self.advance_if_kw(Kw::Try) {
            let expr = self.parse_expr()?;
            let mut else_exit = false;
            if self.advance_if_kw(Kw::Else) {
                let (id, _) = self.expect_ident("'exit'")?;
                if id != "exit" {
                    return self.err("'exit' after 'else'");
                }
                else_exit = true;
            }
            return Ok(Expr { span, kind: ExprKind::Try(Box::new(expr), else_exit) });
        }

        self.err("expression")
    }
}
