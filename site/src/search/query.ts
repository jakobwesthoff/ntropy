// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The query language's tokenizer and parser, mirroring `src/query/token.rs`
// and `src/query/parser.rs` (ADR 0012, ADR 0052). Precedence is
// `not > and > or`, parentheses override, and a keyword acts as an operator
// only in operator position and when no `:` follows it, so `or:x` is the
// field predicate `or`.

export type Query =
  | { kind: "and"; left: Query; right: Query }
  | { kind: "or"; left: Query; right: Query }
  | { kind: "not"; operand: Query }
  | { kind: "tag"; value: string }
  | { kind: "field"; name: string; value: string }
  | { kind: "text"; pattern: string };

/** A syntax error at a character position, or a pattern that does not compile. */
export class QueryError extends Error {
  constructor(
    message: string,
    /** The zero-based character position of a syntax error, or null for a pattern error. */
    public readonly position: number | null,
  ) {
    super(message);
    this.name = "QueryError";
  }

  static parse(position: number, message: string): QueryError {
    return new QueryError(
      `query syntax error at position ${position}: ${message}`,
      position,
    );
  }

  static pattern(pattern: string, message: string): QueryError {
    return new QueryError(
      `invalid search pattern \`${pattern}\`: ${message}`,
      null,
    );
  }
}

type Token =
  | { kind: "lparen"; pos: number }
  | { kind: "rparen"; pos: number }
  | { kind: "colon"; pos: number }
  | { kind: "word"; value: string; pos: number }
  | { kind: "string"; value: string; pos: number };

function isWordChar(ch: string): boolean {
  return /[\p{L}\p{N}/_-]/u.test(ch);
}

export function tokenize(input: string): Token[] {
  const chars = Array.from(input);
  const tokens: Token[] = [];
  let i = 0;
  while (i < chars.length) {
    const ch = chars[i] as string;
    if (/\s/.test(ch)) {
      i += 1;
    } else if (ch === "(") {
      tokens.push({ kind: "lparen", pos: i });
      i += 1;
    } else if (ch === ")") {
      tokens.push({ kind: "rparen", pos: i });
      i += 1;
    } else if (ch === ":") {
      tokens.push({ kind: "colon", pos: i });
      i += 1;
    } else if (ch === '"') {
      const [value, next] = lexString(chars, i);
      tokens.push({ kind: "string", value, pos: i });
      i = next;
    } else if (isWordChar(ch)) {
      const start = i;
      let word = "";
      while (i < chars.length && isWordChar(chars[i] as string)) {
        word += chars[i];
        i += 1;
      }
      tokens.push({ kind: "word", value: word, pos: start });
    } else {
      throw QueryError.parse(
        i,
        `unexpected character \`${ch}\` (quote it to search literally)`,
      );
    }
  }
  return tokens;
}

/** A double-quoted string from its opening quote; `\"` and `\\` are the
 * only escapes, any other backslash passes through with its character. */
function lexString(chars: string[], open: number): [string, number] {
  let value = "";
  let i = open + 1;
  while (i < chars.length) {
    const ch = chars[i] as string;
    if (ch === '"') return [value, i + 1];
    if (ch === "\\" && i + 1 < chars.length) {
      const next = chars[i + 1] as string;
      if (next === '"') value += '"';
      else if (next === "\\") value += "\\";
      else value += `\\${next}`;
      i += 2;
    } else {
      value += ch;
      i += 1;
    }
  }
  throw QueryError.parse(open, "unterminated quoted string");
}

export function parse(input: string): Query {
  const tokens = tokenize(input);
  const eof = Array.from(input).length;
  if (tokens.length === 0) throw QueryError.parse(0, "expected a query");
  const parser = new Parser(tokens, eof);
  const query = parser.parseOr();
  const trailing = parser.peek();
  if (trailing !== undefined) {
    throw QueryError.parse(trailing.pos, "unexpected trailing input");
  }
  return query;
}

class Parser {
  private index = 0;

  constructor(
    private readonly tokens: Token[],
    private readonly eof: number,
  ) {}

  peek(offset = 0): Token | undefined {
    return this.tokens[this.index + offset];
  }

  private advance(): Token | undefined {
    const token = this.tokens[this.index];
    if (token !== undefined) this.index += 1;
    return token;
  }

  private atOperator(keyword: string): boolean {
    const token = this.peek();
    if (token === undefined || token.kind !== "word") return false;
    if (token.value.toLowerCase() !== keyword) return false;
    return this.peek(1)?.kind !== "colon";
  }

  parseOr(): Query {
    let left = this.parseAnd();
    while (this.atOperator("or")) {
      this.advance();
      const right = this.parseAnd();
      left = { kind: "or", left, right };
    }
    return left;
  }

  private parseAnd(): Query {
    let left = this.parseUnary();
    while (this.atOperator("and")) {
      this.advance();
      const right = this.parseUnary();
      left = { kind: "and", left, right };
    }
    return left;
  }

  private parseUnary(): Query {
    if (this.atOperator("not")) {
      this.advance();
      return { kind: "not", operand: this.parseUnary() };
    }
    return this.parsePrimary();
  }

  private parsePrimary(): Query {
    if (this.peek()?.kind === "lparen") {
      this.advance();
      const inner = this.parseOr();
      const close = this.advance();
      if (close === undefined) throw QueryError.parse(this.eof, "expected `)`");
      if (close.kind !== "rparen")
        throw QueryError.parse(close.pos, "expected `)`");
      return inner;
    }
    return this.parsePredicate();
  }

  private parsePredicate(): Query {
    const token = this.advance();
    if (token === undefined)
      throw QueryError.parse(this.eof, "expected a predicate");
    if (token.kind === "string") return { kind: "text", pattern: token.value };
    if (token.kind !== "word") {
      throw QueryError.parse(
        token.pos,
        `expected a predicate, found \`${renderKind(token)}\``,
      );
    }
    const word = token.value;
    if (this.peek()?.kind !== "colon") return { kind: "text", pattern: word };
    this.advance();
    const valueToken = this.advance();
    if (valueToken === undefined)
      throw QueryError.parse(this.eof, "expected a value after `:`");
    if (valueToken.kind !== "word" && valueToken.kind !== "string") {
      throw QueryError.parse(valueToken.pos, "expected a value after `:`");
    }
    const value = valueToken.value;
    const key = word.toLowerCase();
    if (key === "tag") return { kind: "tag", value };
    if (key === "text") return { kind: "text", pattern: value };
    return { kind: "field", name: word, value };
  }
}

function renderKind(token: Token): string {
  switch (token.kind) {
    case "lparen":
      return "(";
    case "rparen":
      return ")";
    case "colon":
      return ":";
    case "word":
      return "word";
    case "string":
      return "string";
  }
}
